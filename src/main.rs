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
use mod_registry::ModRegistry;
use tracing::{debug, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .compact()
        .with_max_level(tracing::Level::ERROR)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(true)
        .init();

    info!("Application starts");

    let cli = Cli::parse();
    debug!("Command passed: {:#?}", &cli.command);

    // Initialize downloader early for list and update commands
    let home = cli.mods_dir.unwrap_or(fileutil::get_mods_directory()?);
    let downloader = ModDownloader::new(&home);

    match &cli.command {
        Commands::List => {
            let installed_mods = downloader.list_installed_mods()?;
            if installed_mods.is_empty() {
                println!("No mods are currently installed.");
                return Ok(());
            }

            println!("\nInstalled mods ({} found):", installed_mods.len());
            for mod_info in installed_mods {
                println!("- {} (version {})", mod_info.mod_name, mod_info.version);
            }
        }

        Commands::Show(args) => {
            println!("Checking installed mod information...");
            let installed_mods = downloader.list_installed_mods()?;
            if let Some(mod_info) = installed_mods.iter().find(|m| m.mod_name == args.name) {
                println!("Mod Information:");
                println!("- Name: {}", mod_info.mod_name);
                println!("- Version: {}", mod_info.version);
            } else {
                println!("The mod '{}' is not currently installed.", args.name);
            }
        }

        // For remaining commands, fetch the remote mod registry
        _ => {
            let mod_registry_data = downloader.fetch_mod_registry().await?;
            let mod_registry = ModRegistry::from(mod_registry_data).await?;

            match &cli.command {
                Commands::Install(args) => {
                    println!("Starting installation of the mod '{}'...", args.name);
                    if let Some(mod_info) = mod_registry.get_mod_info(&args.name) {
                        println!("Downloading mod files...");
                        downloader
                            .download_mod(
                                &mod_info.download_url,
                                &mod_info.name,
                                &mod_info.checksums,
                            )
                            .await?;
                        println!("Installation finished successfully!");
                    } else {
                        println!("The mod '{}' could not be found.", args.name);
                    }
                }
                Commands::Update(args) => {
                    println!("Checking mod updates...");
                    let available_updates = downloader.check_updates(&mod_registry)?;
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
                            for update in available_updates {
                                println!("\nUpdating {}...", update.name);
                                downloader
                                    .download_mod(&update.url, &update.name, &update.hash)
                                    .await?;
                                println!(
                                    "  Updated {} to version {}",
                                    update.name, update.available_version
                                );
                            }
                            println!("\nAll updates installed successfully!");
                        } else {
                            println!("\nRun with --install to install these updates");
                        }
                    }
                }
                // Catch-all arm (should not be reached because all subcommands are handled)
                _ => {
                    println!("Use --help to see available commands");
                }
            }
        }
    }

    Ok(())
}
