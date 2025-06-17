use std::sync::Arc;

use anyhow::Result;
use clap::Parser;

mod cli;
mod config;
mod constant;
mod dependency;
mod download;
mod error;
mod fetch;
mod fileutil;
mod local;
mod mod_registry;

use crate::{
    cli::{Cli, Commands},
    config::Config,
    dependency::ModDependencyQuery,
    local::LocalMod,
    mod_registry::{ModRegistryQuery, RemoteModRegistry},
};

fn setup_logging(verbose: bool) {
    use tracing_subscriber::{
        Layer, filter::LevelFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt,
    };

    // Create a layer for ERROR level and above - no timestamp
    let info_layer = fmt::layer()
        .with_ansi(true)
        .with_level(true)
        .with_target(false)
        .without_time()
        .with_filter(LevelFilter::ERROR);

    // Create a layer for DEBUG level - with module name, thread IDs, detailed file information
    let debug_layer = fmt::layer()
        .with_ansi(true)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .with_filter(LevelFilter::DEBUG);

    // Only register the debug layer if in verbose mode
    if verbose {
        tracing_subscriber::registry().with(debug_layer).init();
    } else {
        tracing_subscriber::registry().with(info_layer).init();
    }
}

async fn run() -> Result<()> {
    tracing::info!("Application starts");

    let cli = Cli::parse();

    // Initialize the tracing subscriber for logging based on user flags.
    setup_logging(cli.verbose);

    tracing::debug!("Command passed: {:?}", &cli.command);

    let config = Config::new(&cli)?;

    // Determine the mods directory.
    let mods_directory = config.directory();
    tracing::debug!(
        "Determined mods directory: {}",
        fileutil::replace_home_dir_with_tilde(mods_directory)
    );

    // Gathering mod paths
    let archive_paths = config.find_installed_mod_archives()?;

    match &cli.command {
        // Show mod name and file name of installed mods.
        Commands::List => {
            if archive_paths.is_empty() {
                println!("No mods are currently installed.");
                return Ok(());
            }

            let local_mods = LocalMod::load_local_mods(&archive_paths);

            local_mods.iter().for_each(|local_mod| {
                if let Some(os_str) = local_mod.file_path.file_name() {
                    println!(
                        "- {} ({})",
                        local_mod.manifest.name,
                        os_str.to_string_lossy()
                    );
                }
            });

            println!("\n✅ {} mods found.", &local_mods.len());
        }

        // Show details of a specific mod if it is installed.
        Commands::Show(args) => {
            tracing::info!("Checking installed mod information...");

            let local_mods = LocalMod::load_local_mods(&archive_paths);

            if let Some(local_mod) = local_mods.iter().find(|m| m.manifest.name == args.name) {
                println!(
                    "📂 {}",
                    fileutil::replace_home_dir_with_tilde(&local_mod.file_path)
                );
                println!("- Name: {}", local_mod.manifest.name);
                println!("  Version: {}", local_mod.manifest.version);
                if let Some(deps) = &local_mod.manifest.dependencies {
                    println!("  Dependencies:");
                    for dep in deps {
                        println!("    - Name: {}", dep.name);
                        if let Some(version) = &dep.version {
                            println!("      Version: {}", version);
                        }
                    }
                }
                if let Some(opt_deps) = &local_mod.manifest.optional_dependencies {
                    println!("  Optional Dependencies:");
                    for dep in opt_deps {
                        println!("    - Name: {}", dep.name);
                        if let Some(version) = &dep.version {
                            println!("      Version: {}", version);
                        }
                    }
                }
            } else {
                println!("The mod '{}' is not currently installed.", args.name);
            }
        }

        // Install a mod by fetching its information from the mod registry.
        Commands::Install(args) => {
            let mod_id = args.parse_mod_page_url()?;
            // Fetching online database
            let (mod_registry, dependency_graph) = fetch::fetch_online_database().await?;

            // Gets the mod name by using the ID from the Remote Mod Registry.
            let mod_name = match mod_registry.get_mod_name_by_id(mod_id) {
                Some(name) => name,
                None => {
                    println!("Could not find the mod matches [{}].", mod_id);
                    return Ok(());
                }
            };

            let installed_mod_names = LocalMod::names(&archive_paths);
            if installed_mod_names.contains(mod_name) {
                println!("You already have [{}] installed.", mod_name);
                return Ok(());
            }

            let downloadable_mods =
                dependency_graph.check_dependencies(mod_name, &mod_registry, &installed_mod_names);
            if downloadable_mods.is_empty() {
                println!(
                    "All dependencies for mod [{}] are already installed",
                    mod_name
                );
                return Ok(());
            }
            println!("Downloading mod [{}] and its dependencies...", mod_name);
            download::download_mods_concurrently(&downloadable_mods, config, 6).await?;
        }

        Commands::Update(args) => {
            // Filter installed mods according to the `updaterblacklist.txt`
            let mut local_mods = LocalMod::load_local_mods(&archive_paths);
            if let Some(blacklist) = config.read_updater_blacklist()? {
                local_mods.retain(|local_mod| !blacklist.contains(&local_mod.file_path));
            }

            // Update installed mods by checking for available updates in the mod registry.
            let spinner = download::pb_style::create_spinner();
            let client = reqwest::ClientBuilder::new()
                .http2_prior_knowledge()
                .gzip(true)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());
            let mod_registry = RemoteModRegistry::fetch(&client).await?;
            spinner.finish_and_clear();
            drop(spinner);

            let registry = Arc::new(mod_registry);

            let available_updates = registry.check_updates(&local_mods);

            if available_updates.is_empty() {
                println!("All mods are up to date!");
            } else if args.install {
                println!("\nInstalling updates...");
                download::download_mods_concurrently(&available_updates, config, 6).await?;
            } else {
                println!("\nRun with --install to install these updates");
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        tracing::error!("{:#?}", err);
        eprintln!("Failed to run the command.");
    }
    tracing::info!("Command completed successfully.");
}
