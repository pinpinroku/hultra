use clap::Parser;
use indicatif::ProgressBar;
use reqwest::Client;
use std::collections::HashSet;
use tracing::{debug, info};

mod cli;
mod constant;
mod download;
mod error;
mod fileutil;
mod local;
mod mod_registry;

use cli::{Cli, Commands};
use download::{
    install::parse_mod_page_url,
    update::{self, check_updates},
};
use error::Error;
use fileutil::{find_installed_mod_archives, read_updater_blacklist, replace_home_dir_with_tilde};
use local::remove_blacklisted_mods;
use mod_registry::ModRegistryQuery;

fn setup_logging(verbose: bool) {
    use tracing_subscriber::{
        Layer, filter::LevelFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt,
    };

    // Create a layer for INFO level and above - no timestamp
    let info_layer = fmt::layer()
        .with_ansi(true)
        .with_level(false)
        .with_target(false)
        .without_time()
        .with_filter(LevelFilter::INFO);

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

async fn run() -> Result<(), Error> {
    debug!("Application starts");

    let cli = Cli::parse();

    // Initialize the tracing subscriber for logging based on user flags.
    setup_logging(cli.verbose);

    debug!("Command passed: {:#?}", &cli.command);

    // Determine the mods directory.
    let mods_directory = cli.mods_dir.unwrap_or(fileutil::get_mods_directory()?);
    debug!(
        "Determined mods directory: {}",
        replace_home_dir_with_tilde(&mods_directory)
    );

    // Gathering mod paths
    let archive_paths = find_installed_mod_archives(&mods_directory)?;
    debug!("Number of mod files found: {}", archive_paths.len());

    match &cli.command {
        // Show name and version of installed mods
        Commands::List => {
            if archive_paths.is_empty() {
                info!("No mods are currently installed.");
                return Ok(());
            }

            let local_mods = local::load_local_mods(archive_paths)?;

            for local_mod in local_mods.iter() {
                info!(
                    "- {} (version {})",
                    local_mod.manifest.name, local_mod.manifest.version
                );
            }

            debug!("{} mods installed.", &local_mods.len());
        }

        // Show details of a specific mod if it is installed.
        Commands::Show(args) => {
            debug!("Checking installed mod information...");

            let local_mods = local::load_local_mods(archive_paths)?;

            if let Some(local_mod) = local_mods.iter().find(|m| m.manifest.name == args.name) {
                info!(
                    "📂 {}\n",
                    fileutil::replace_home_dir_with_tilde(&local_mod.file_path)
                );
                info!("- Name: {}", local_mod.manifest.name);
                info!("  Version: {}", local_mod.manifest.version);
                if let Some(deps) = &local_mod.manifest.dependencies {
                    info!("  Dependencies:");
                    for dep in deps {
                        info!("    - Name: {}", dep.name);
                        info!("      Version: {}", dep.version);
                    }
                }
                if let Some(opt_deps) = &local_mod.manifest.optional_dependencies {
                    info!("  Optional Dependencies:");
                    for dep in opt_deps {
                        info!("    - Name: {}", dep.name);
                        info!("      Version: {}", dep.version);
                    }
                }
            } else {
                info!("The mod '{}' is not currently installed.", args.name);
            }
        }

        // Install a mod by fetching its information from the mod registry.
        Commands::Install(args) => {
            let mod_id = parse_mod_page_url(&args.mod_page_url)?;

            // Fetches mod information from URL
            let mod_registry = mod_registry::fetch_remote_mod_registry().await?;
            let mod_info = mod_registry.find_mod_registry_by_id(mod_id);

            // If the mod is found in the database, check if it is installed or not, if not, install it.
            match mod_info {
                Some((mod_name, manifest)) => {
                    debug!("Matched entry name: {}", mod_name);
                    debug!("Matched entry detail: {:#?}", manifest);

                    // Check if already installed
                    let installed_mods = local::load_local_mods(archive_paths)?;

                    // Create a vector of mod names.
                    let installed_names: HashSet<_> = installed_mods
                        .into_iter()
                        .map(|installed| installed.manifest.name)
                        .collect();

                    // Check if the target mod_name is in the vector.
                    if installed_names.contains(mod_name) {
                        info!("You already have [{}] installed.", mod_name);
                        return Ok(());
                    }

                    // Install the new mod
                    let client = Client::new();
                    let pb = ProgressBar::new(manifest.file_size);
                    let mod_registry = mod_registry.clone();
                    download::install::install(
                        &client,
                        (mod_name, manifest),
                        &mod_registry,
                        &mods_directory,
                        installed_names,
                        &pb,
                    )
                    .await?;
                }
                None => {
                    info!("Could not find a mod matching [{}].", &args.mod_page_url);
                }
            }
        }

        Commands::Update(args) => {
            // Update installed mods by checking for available updates in the mod registry.
            let mod_registry = mod_registry::fetch_remote_mod_registry().await?;

            // Filter installed mods by using the blacklist
            let mut installed_mods = local::load_local_mods(archive_paths)?;
            let blacklist = read_updater_blacklist(&mods_directory)?;
            remove_blacklisted_mods(&mut installed_mods, &blacklist)?;

            let available_updates = check_updates(installed_mods, mod_registry).await?;
            if available_updates.is_empty() {
                info!("All mods are up to date!");
            } else if args.install {
                info!("\nInstalling updates...");
                let client = Client::new();
                update::update_multiple_mods(&client, &mods_directory, available_updates).await?;
            } else {
                info!("\nRun with --install to install these updates");
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        tracing::error!("{}", err)
    }
}
