use clap::Parser;
use indicatif::MultiProgress;
use reqwest::Client;
use tracing::{debug, info, level_filters::LevelFilter};

mod cli;
mod client;
mod constant;
mod download;
mod error;
mod fileutil;
mod installed_mods;
mod mod_registry;

use cli::{Cli, Commands};
use download::build_progress_bar;
use error::Error;
use fileutil::{find_installed_mod_archives, read_updater_blacklist};
use installed_mods::{check_updates, list_installed_mods, remove_blacklisted_mods};

/// Initialize logging with a level based on CLI flags.
/// --verbose sets the level to DEBUG with extra details enabled.
/// --quiet disables logging.
/// Otherwise default level INFO is selected without extra details.
fn init_tracing(cli: &Cli) {
    use tracing_subscriber::fmt;

    let (max_level, extra_details) = match (cli.quiet, cli.verbose) {
        (true, _) => (LevelFilter::OFF, false),
        (_, true) => (LevelFilter::DEBUG, true),
        _ => (LevelFilter::INFO, false),
    };

    let builder = fmt().compact().with_max_level(max_level);

    let builder = if extra_details {
        builder
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_target(true)
    } else {
        builder
            .with_file(false)
            .with_line_number(false)
            .with_thread_ids(false)
            .with_target(false)
    };

    builder.init();
}

/// The main function initializes the application, sets up tracing for logging, and parses CLI arguments.
///
/// Based on the provided commands, it performs actions like listing mods, showing mod details,
/// installing mods, or updating mods.
///
/// # Errors
/// Returns an error if there are issues with parsing arguments, accessing the mods directory,
/// or performing mod-related operations.
#[tokio::main]
async fn main() -> Result<(), Error> {
    // Parse CLI arguments.
    let cli = Cli::parse();

    // Initialize the tracing subscriber for logging based on user flags.
    init_tracing(&cli);

    debug!("Application starts");
    debug!("Command passed: {:#?}", &cli.command);

    // Determine the mods directory.
    let mods_directory = cli.mods_dir.unwrap_or(fileutil::get_mods_directory()?);

    // Gathering mod paths
    let archive_paths = find_installed_mod_archives(&mods_directory)?;

    // Handle commands based on user input.
    match &cli.command {
        Commands::List => {
            if archive_paths.is_empty() {
                info!("No mods are currently installed.");
                return Ok(());
            }

            // List all installed mods in the mods directory.
            let installed_mods = list_installed_mods(archive_paths)?;

            info!("\nInstalled mods ({} found):", installed_mods.len());
            for mod_info in installed_mods {
                println!(
                    "- {} (version {})",
                    mod_info.manifest.name, mod_info.manifest.version
                );
            }
        }

        Commands::Show(args) => {
            // Show details of a specific mod if it is installed.
            println!("Checking installed mod information...");
            let installed_mods = list_installed_mods(archive_paths)?;
            if let Some(mod_info) = installed_mods.iter().find(|m| m.manifest.name == args.name) {
                println!("\nMod Information:");
                println!("- Name: {}", mod_info.manifest.name);
                println!("- Version: {}", mod_info.manifest.version);
                if let Some(deps) = &mod_info.manifest.dependencies {
                    println!("  Dependencies:");
                    for dep in deps {
                        if let Some(ver) = &dep.version {
                            println!("  - Name: {}", dep.name);
                            println!("  - Version: {}", ver);
                        } else {
                            println!("  - {}", dep.name);
                        }
                    }
                }
                if let Some(opt_deps) = &mod_info.manifest.optional_dependencies {
                    println!("  Optional Dependencies:");
                    for dep in opt_deps {
                        if let Some(ver) = &dep.version {
                            println!("  - Name: {}", dep.name);
                            println!("  - Version: {}", ver);
                        } else {
                            println!("  - {}", dep.name);
                        }
                    }
                }
            } else {
                println!("The mod '{}' is not currently installed.", args.name);
            }
        }

        // Install a mod by fetching its information from the mod registry.
        Commands::Install(args) => {
            // Fetching the mod information
            let mod_registry = mod_registry::fetch_remote_mod_registry().await?;
            let mod_info = mod_registry::get_mod_info_by_url(&mod_registry, &args.mod_page_url);
            debug!("Matched entry: {:#?}", mod_info);

            // Determine if the input is a URL or a mod name
            match mod_info {
                Some(mod_info) => {
                    // Check if already installed
                    let installed_mods = list_installed_mods(archive_paths)?;
                    if installed_mods
                        .into_iter()
                        .any(|installed| installed.manifest.name == *mod_info.0)
                    {
                        println!("You already have [{}] installed.", mod_info.0);
                        return Ok(());
                    }

                    // Setup components for the downloader
                    let client = Client::new();
                    let total_size =
                        crate::client::get_file_size(client.clone(), &mod_info.1.download_url)
                            .await?;
                    assert_eq!(total_size, mod_info.1.file_size, "File sizes must match!");

                    let pb = build_progress_bar(mod_info.0, Some(total_size));

                    crate::client::download_file(
                        client.clone(),
                        mod_info.0,
                        &mod_info.1.download_url,
                        &mod_info.1.checksums,
                        &mods_directory,
                        pb,
                    )
                    .await?;

                    info!("[{}] installation complete.", mod_info.0);
                }
                None => {
                    println!("Could not find a mod matching [{}].", &args.mod_page_url);
                }
            }
        }

        Commands::Update(args) => {
            // Update installed mods by checking for available updates in the mod registry.
            let mod_registry = mod_registry::fetch_remote_mod_registry().await?;

            // Filter installed mods by using the blacklist
            let mut installed_mods = list_installed_mods(archive_paths)?;
            let blacklist = read_updater_blacklist(&mods_directory)?;
            remove_blacklisted_mods(&mut installed_mods, &blacklist)?;

            println!("Checking mod updates...");
            let available_updates = check_updates(installed_mods, &mod_registry)?;
            if available_updates.is_empty() {
                println!("All mods are up to date!");
            } else {
                println!("Available updates:");
                for update_info in &available_updates {
                    println!("\n{}", update_info.name);
                    println!(" - Current version: {}", update_info.current_version);
                    println!(" - Available version: {}", update_info.available_version);
                }
                if args.install {
                    println!("\nInstalling updates...");
                    let mut handles = Vec::new();
                    let multi_progress = MultiProgress::new();
                    let client = Client::new();

                    available_updates.into_iter().for_each(|update| {
                        let client = client.clone();
                        let mods_directory = mods_directory.clone();
                        let pb = multi_progress.add(build_progress_bar(&update.name, None));

                        let handle = tokio::spawn(async move {
                            let result = crate::client::download_file(client, &update.name, &update.url, &update.hash, &mods_directory, pb).await;

                            match result {
                                Ok(_) => {
                                    println!(
                                        "[Success] Updated {} to version {}\n",
                                        update.name, update.available_version
                                    );
                                    if update.existing_path.exists() {
                                        if let Err(e) =
                                            tokio::fs::remove_file(&update.existing_path).await
                                        {
                                            eprintln!(
                                                "Failed to remove outdated file: {}.\nPlease remove it manually. File path: {}",
                                                e,
                                                update.existing_path.display()
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("[Error] Failed to update {}: {}", update.name, e);
                                }
                            }
                        });
                        handles.push(handle);
                    });

                    for handle in handles {
                        handle.await?;
                    }

                    println!("\nAll updates installed successfully!");
                } else {
                    println!("\nRun with --install to install these updates");
                }
            }
        }
    }

    Ok(())
}
