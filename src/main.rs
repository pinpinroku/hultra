use clap::Parser;
use indicatif::MultiProgress;
use reqwest::Url;
use tracing::info;

mod cli;
mod client;
mod constant;
mod download;
mod error;
mod fileutil;
mod installed_mods;
mod mod_registry;

use cli::{Cli, Commands};
use download::{ModDownloader, build_progress_bar};
use error::Error;
use fileutil::{find_installed_mod_archives, read_updater_blacklist};
use installed_mods::{check_updates, list_installed_mods, remove_blacklisted_mods};
use mod_registry::ModRegistry;

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
    // Initialize the tracing subscriber for logging.
    tracing_subscriber::fmt()
        .compact()
        .with_max_level(tracing::Level::ERROR)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(true)
        .init();

    info!("Application starts");

    // Parse CLI arguments.
    let cli = Cli::parse();
    info!("Command passed: {:#?}", &cli.command);

    // Determine the mods directory.
    let mods_directory = cli.mods_dir.unwrap_or(fileutil::get_mods_directory()?);

    // Gathering mod paths
    let archive_paths = find_installed_mod_archives(&mods_directory)?;

    // Handle commands based on user input.
    match &cli.command {
        Commands::List => {
            if archive_paths.is_empty() {
                println!("No mods are currently installed.");
                return Ok(());
            }

            // List all installed mods in the mods directory.
            let installed_mods = list_installed_mods(archive_paths)?;

            println!("\nInstalled mods ({} found):", installed_mods.len());
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
            let installed_mods = list_installed_mods(archive_paths)?;

            // HACK: If args.url_or_name is a name, check if already installed to prevent unnecessary fetching
            // If args.url_or_name is an URL, fetch mod registry to get actual download URL
            let downloader = ModDownloader::new(&mods_directory);
            let mod_registry_data = downloader.fetch_mod_registry().await?;
            let mod_registry = ModRegistry::from(mod_registry_data).await?;

            // Determine if the input is a URL or a mod name
            let name_or_url = &args.name_or_url;
            let mod_info = Url::parse(name_or_url)
                .is_ok_and(|url| {
                    url.host_str()
                        .is_some_and(|host| host.contains("gamebanana.com"))
                })
                // Valid GameBanana URL
                .then(|| mod_registry.get_mod_info_from_url(name_or_url))
                // Not a valid GameBanana URL, treat as mod name
                .unwrap_or_else(|| mod_registry.get_mod_info_by_name(name_or_url));

            match mod_info {
                Some(mod_info) => {
                    // Check if already installed
                    if installed_mods
                        .iter()
                        .any(|installed| installed.manifest.name == mod_info.name)
                    {
                        println!("You already have [{}] installed.", mod_info.name);
                        return Ok(());
                    }
                    let pb = build_progress_bar(&mod_info.name, Some(mod_info.file_size));

                    downloader.download_mod(mod_info, pb).await?;

                    info!("[{}] installation complete.", mod_info.name);
                }
                None => {
                    println!("Could not find a mod matching [{}].", name_or_url);
                }
            }
        }

        Commands::Update(args) => {
            // Update installed mods by checking for available updates in the mod registry.
            let downloader = ModDownloader::new(&mods_directory);
            let mod_registry_data = downloader.fetch_mod_registry().await?;
            let mod_registry = ModRegistry::from(mod_registry_data).await?;

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

                    for update in available_updates {
                        let downloader = downloader.clone();
                        let pb = multi_progress.add(build_progress_bar(&update.name, None));

                        let handle = tokio::spawn(async move {
                            let result = downloader.download_mod(&update, pb).await;

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
                    }

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
