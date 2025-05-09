use clap::Parser;
use indicatif::ProgressBar;
use reqwest::Client;
use std::time::Duration;

mod cli;
mod constant;
mod download;
mod error;
mod fileutil;
mod local;
mod mod_registry;

use cli::{Cli, Commands};
use download::{install, update};
use error::Error;
use mod_registry::ModRegistryQuery;

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

async fn run() -> Result<(), Error> {
    tracing::info!("Application starts");

    let cli = Cli::parse();

    // Initialize the tracing subscriber for logging based on user flags.
    setup_logging(cli.verbose);

    tracing::debug!("Command passed: {:?}", &cli.command);

    // Determine the mods directory.
    let mods_directory = cli.mods_dir.unwrap_or(fileutil::get_mods_directory()?);
    tracing::debug!(
        "Determined mods directory: {}",
        fileutil::replace_home_dir_with_tilde(&mods_directory)
    );

    // Gathering mod paths
    let archive_paths = fileutil::find_installed_mod_archives(&mods_directory)?;

    match &cli.command {
        // Show mod name and file name of installed mods.
        Commands::List => {
            if archive_paths.is_empty() {
                println!("No mods are currently installed.");
                return Ok(());
            }

            let local_mods = local::load_local_mods(&archive_paths)?;

            local_mods.iter().for_each(|local_mod| {
                if let Some(name) = local_mod.file_path.file_name() {
                    println!("- {} ({})", local_mod.manifest.name, name.to_string_lossy());
                } else {
                    println!("- {}", local_mod.manifest.name);
                }
            });

            println!("\n✅ {} mods found.", &local_mods.len());
        }

        // Show details of a specific mod if it is installed.
        Commands::Show(args) => {
            tracing::info!("Checking installed mod information...");

            let local_mods = local::load_local_mods(&archive_paths)?;

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
                        println!("      Version: {}", dep.version);
                    }
                }
                if let Some(opt_deps) = &local_mod.manifest.optional_dependencies {
                    println!("  Optional Dependencies:");
                    for dep in opt_deps {
                        println!("    - Name: {}", dep.name);
                        println!("      Version: {}", dep.version);
                    }
                }
            } else {
                println!("The mod '{}' is not currently installed.", args.name);
            }
        }

        // Install a mod by fetching its information from the mod registry.
        Commands::Install(args) => {
            let mod_id = install::parse_mod_page_url(&args.mod_page_url)?;

            tracing::info!("Look up the mod entry matches the given URL by using its ID");
            let mod_registry = mod_registry::fetch_remote_mod_registry().await?;
            let mod_entry = mod_registry.find_mod_entry_by_id(mod_id);

            let (mod_name, remote_mod) = match mod_entry {
                Some(entry) => entry,
                None => {
                    println!("Could not find a mod matching [{}].", &args.mod_page_url);
                    return Ok(());
                }
            };

            tracing::debug!("Matched entry name: {}", mod_name);
            tracing::debug!("Matched entry detail: {:#?}", remote_mod);

            tracing::info!("Check if the mod is already installed or not");
            let local_mods = local::load_local_mods(&archive_paths)?;
            let installed_mod_names = local::collect_installed_mod_names(local_mods)?;
            if installed_mod_names.contains(mod_name) {
                println!("You already have [{}] installed.", mod_name);
                return Ok(());
            }

            tracing::info!("Start installing the new mod");
            let client = Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .build()?;
            let pb = ProgressBar::new(remote_mod.file_size);
            install::install(
                &client,
                (mod_name, remote_mod),
                &mod_registry,
                &mods_directory,
                installed_mod_names,
                &pb,
            )
            .await?;
        }

        Commands::Update(args) => {
            // Filter installed mods according to the `updaterblacklist.txt`
            let mut local_mods = local::load_local_mods(&archive_paths)?;
            let blacklist = fileutil::read_updater_blacklist(&mods_directory)?;
            local::remove_blacklisted_mods(&mut local_mods, &blacklist);

            // Update installed mods by checking for available updates in the mod registry.
            let mod_registry = mod_registry::fetch_remote_mod_registry().await?;

            let available_updates = update::check_updates(local_mods, mod_registry).await?;
            if available_updates.is_empty() {
                println!("All mods are up to date!");
            } else if args.install {
                println!("\nInstalling updates...");
                let client = Client::builder()
                    .connect_timeout(Duration::from_secs(5))
                    .build()?;
                update::update_multiple_mods(&client, &mods_directory, available_updates).await?;
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
