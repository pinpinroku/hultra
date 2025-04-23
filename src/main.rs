use clap::Parser;

mod cli;
mod constant;
mod download;
mod error;
mod fileutil;
mod installed_mods;
mod mod_registry;

use cli::{Cli, Commands};
use download::ModDownloader;
use installed_mods::{check_updates, list_installed_mods};
use mod_registry::ModRegistry;
use tracing::info;

/// The main function initializes the application, sets up tracing for logging, and parses CLI arguments.
///
/// Based on the provided commands, it performs actions like listing mods, showing mod details,
/// installing mods, or updating mods.
///
/// # Errors
/// Returns an error if there are issues with parsing arguments, accessing the mods directory,
/// or performing mod-related operations.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let mods_dir = cli.mods_dir.unwrap_or(fileutil::get_mods_directory()?);

    // Handle commands based on user input.
    match &cli.command {
        Commands::List => {
            // List all installed mods in the mods directory.
            let installed_mods = list_installed_mods(&mods_dir)?;
            if installed_mods.is_empty() {
                println!("No mods are currently installed.");
                return Ok(());
            }

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
            let installed_mods = list_installed_mods(&mods_dir)?;
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

        Commands::Install(args) => {
            // Install a mod by fetching its information from the mod registry.
            let installed_mods = list_installed_mods(&mods_dir)?;

            if installed_mods
                .iter()
                .any(|mod_info| mod_info.manifest.name == args.name)
            {
                println!("You already have '{}' installed.", args.name);
                return Ok(());
            }

            let downloader = ModDownloader::new(&mods_dir);
            let mod_registry_data = downloader.fetch_mod_registry().await?;
            let mod_registry = ModRegistry::from(mod_registry_data).await?;

            println!("Starting installation of the mod '{}'", args.name);
            if let Some(mod_info) = mod_registry.get_mod_info(&args.name) {
                downloader
                    .download_mod(&mod_info.download_url, &mod_info.name, &mod_info.checksums)
                    .await?;
                println!("Installation finished successfully!");
            } else {
                println!("The mod '{}' could not be found.", args.name);
            }
        }

        Commands::Update(args) => {
            // Update installed mods by checking for available updates in the mod registry.
            let downloader = ModDownloader::new(&mods_dir);
            let mod_registry_data = downloader.fetch_mod_registry().await?;
            let mod_registry = ModRegistry::from(mod_registry_data).await?;

            println!("Checking mod updates...");
            let available_updates = check_updates(&mods_dir, &mod_registry)?;
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

                    for update in available_updates {
                        let downloader = downloader.clone();

                        let handle = tokio::spawn(async move {
                            let result = downloader
                                .download_mod(&update.url, &update.name, &update.hash)
                                .await;

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
