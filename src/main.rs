use std::{collections::HashSet, fmt};

use anyhow::Context;
use clap::Parser;
use tracing::{debug, info};

use crate::{
    cli::{Cli, Command},
    config::{AppConfig, CARGO_PKG_NAME, CARGO_PKG_VERSION},
    dependency::DependencyGraph,
    download::{DatabaseKind, Downloader},
    local_mods::LocalMod,
    registry::ModRegistry,
};

mod cache;
mod cli;
mod config;
mod dependency;
mod download;
mod local_mods;
mod log;
mod mirrorlist;
mod registry;
mod update;

/// Represents success case
#[derive(Debug)]
enum Success {
    UpToDate,
    AllModsBlacklisted,
    AlreadyInstalled,
    Completed,
}

/// Display message for success operation
impl fmt::Display for Success {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Success::UpToDate => write!(f, "All mods are up-to-date!"),
            Success::AllModsBlacklisted => {
                write!(f, "All of mods are blacklisted, skipping updates.")
            }
            Success::AlreadyInstalled => {
                write!(f, "All mods are already installed, exiting program.")
            }
            Success::Completed => write!(f, "Command completed successfully."),
        }
    }
}

async fn run() -> anyhow::Result<()> {
    let args = Cli::parse();

    log::set_up_logger(args.verbose);

    info!("{} version {}", CARGO_PKG_NAME, CARGO_PKG_VERSION);
    debug!("\n{:#?}", args);

    // Init app config
    let config = AppConfig::new(args.directory.as_deref())?;
    debug!("\n{:#?}", config);

    // Load already installed mods
    let installed_mods = LocalMod::load_local_mods(&config).with_context(|| {
        format!(
            "failed to read mods directory: {}",
            config.mods_dir().display()
        )
    })?;

    match args.commands {
        Command::List => {
            for installed in installed_mods {
                println!("{}", installed)
            }
        }

        Command::Install { urls, option } => {
            info!("starting install mods");

            // Parse mod page URLs to get mod IDs
            let ids: Vec<u32> = urls
                .iter()
                .filter_map(|url| url.extract_id().ok())
                .collect();

            // Fetch metadata
            let downloader = Downloader::new(60, option.jobs as usize);
            let spinner = download::create_spinner();
            let (reg_bytes, graph_bytes) = tokio::try_join!(
                downloader.fetch_database(DatabaseKind::Update, &option),
                downloader.fetch_database(DatabaseKind::DependencyGraph, &option)
            )?;
            spinner.finish_and_clear();
            let registry = ModRegistry::from_slice(&reg_bytes)?;
            let graph = DependencyGraph::from_slice(&graph_bytes)?;

            // Collect mod names found by ID in registry
            let mod_names = registry.get_names_by_ids(&ids);

            // Collect names of already installed mods
            let local_mod_names: HashSet<&str> = installed_mods
                .iter()
                .map(|local_mod| local_mod.name())
                .collect();

            // If all target mods are already installed, exit early
            if local_mod_names.is_superset(&mod_names) {
                println!("{}", Success::AlreadyInstalled)
            }

            // Traverses dependency graph to collect missing dependency names
            let deps = graph.bfs_traversal(mod_names);

            // Determine which dependencies are missing locally
            let missing_dep_names: HashSet<_> = deps
                .into_iter()
                .filter(|name| !local_mod_names.contains(name.as_str()))
                .collect();

            // Prepare download mods
            let targets = registry::extract_target_mods(registry.mods, &missing_dep_names);

            // Download missing mods
            downloader.download_files(targets, &config, &option).await
        }
        Command::Update(option) => {
            info!("starting update mods");

            let mut local_mods = installed_mods;

            let blacklist = config
                .read_updater_blacklist()
                .context("failed to read updater blacklist")?;
            local_mods.retain(|local_mod| !blacklist.contains(local_mod.get_file_name().as_ref()));

            if local_mods.is_empty() {
                eprintln!("{}", Success::AllModsBlacklisted)
            }

            let cache_db = cache::sync(&config).context("failed to sync file cache")?;

            // fetch metadata
            let downloader = Downloader::new(60, option.jobs as usize);
            let spinner = download::create_spinner();
            let bytes = downloader
                .fetch_database(DatabaseKind::Update, &option)
                .await?;
            spinner.finish_and_clear();
            let registry = ModRegistry::from_slice(&bytes)?;

            // check updates
            let (targets, update_info_list) = update::detect(cache_db, registry.mods, &local_mods);

            if targets.is_empty() {
                println!("{}", Success::UpToDate)
            } else {
                println!("Available updates:\n");
                for update_info in update_info_list {
                    println!("{}", update_info);
                }
                println!();
            }

            // Download updates
            downloader.download_files(targets, &config, &option).await
        }
    }

    info!("{}", Success::Completed);
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("{:?}", err)
    }
}
