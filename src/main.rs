use std::{env, fs, fs::File, sync::Arc};

use anyhow::{Context, Result};
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

/// Initialize logger
fn setup_logger(verbose: bool) -> Result<()> {
    let log_dir = env::home_dir()
        .context("Could not determine home directory")?
        .join(".local/state/everest-mod-cli/");
    fs::create_dir_all(&log_dir).context("Failed to create log directory")?;

    let log_file_path = log_dir.join("everest-mod-cli.log");
    let log_file = File::create(&log_file_path).context("Failed to create log file")?;

    // Determine the log level based on verbosity
    let log_level = if verbose {
        "everest_mod_cli=debug"
    } else {
        "everest_mod_cli=info"
    };

    // construct a subscriber that prints formatted traces to stdout
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_env_filter(log_level)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .with_writer(log_file)
        .with_ansi(false)
        .finish();

    // Start configuring a `fmt` subscriber
    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    setup_logger(cli.verbose)?;

    tracing::info!("Application starts");

    tracing::debug!("Passed CLI arguments: {:#?}", &cli);
    tracing::debug!("Command passed: {:?}", &cli.command);

    let config = Config::new(&cli)?;

    // Determine the mods directory.
    let mods_directory = config.directory();
    tracing::info!(
        "Using mods directory: '{}'",
        fileutil::replace_home_dir_with_tilde(mods_directory)
    );
    tracing::info!("Mirror preference: {}", config.mirror_preferences());

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
                if let Some(os_str) = local_mod.location.file_name() {
                    println!(
                        "- {} ({})",
                        local_mod.manifest.name,
                        os_str.to_string_lossy()
                    );
                }
            });

            println!();
            println!("✅ {} mods found.", &local_mods.len());
        }

        // Show details of a specific mod if it is installed.
        Commands::Show(args) => {
            tracing::info!("Checking installed mod information...");

            let local_mods = LocalMod::load_local_mods(&archive_paths);

            if let Some(local_mod) = local_mods.iter().find(|m| m.manifest.name == args.name) {
                println!(
                    "📂 {}",
                    fileutil::replace_home_dir_with_tilde(&local_mod.location)
                );
                println!("- Name: {}", local_mod.manifest.name);
                println!("  Version: {}", local_mod.manifest.version);
                if let Some(deps) = &local_mod.manifest.dependencies {
                    println!("  Dependencies:");
                    for dep in deps {
                        println!("    - Name: {}", dep.name);
                        if let Some(version) = &dep.version {
                            println!("      Version: {version}");
                        }
                    }
                }
                if let Some(opt_deps) = &local_mod.manifest.optional_dependencies {
                    println!("  Optional Dependencies:");
                    for dep in opt_deps {
                        println!("    - Name: {}", dep.name);
                        if let Some(version) = &dep.version {
                            println!("      Version: {version}");
                        }
                    }
                }
            } else {
                println!("The mod '{}' is not currently installed.", args.name);
            }
        }

        Commands::Install(_) | Commands::Update(_) => {
            let semaphore = Arc::new(tokio::sync::Semaphore::new(6));
            let download_client = reqwest::ClientBuilder::new()
                .use_rustls_tls()
                .https_only(true)
                .http2_adaptive_window(true)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());

            match &cli.command {
                // Install a mod by fetching its information from the mod registry.
                Commands::Install(args) => {
                    let mod_id = args.parse_mod_page_url()?;
                    // Fetching online database
                    let (mod_registry, dependency_graph) = fetch::fetch_online_database().await?;

                    // Gets the mod name by using the ID from the Remote Mod Registry.
                    let mod_names = mod_registry.get_mod_name_by_id(mod_id);
                    if mod_names.is_empty() {
                        println!("Could not find the mod matches [{mod_id}].");
                        return Ok(());
                    };

                    let mut installed_mod_names = LocalMod::names(&archive_paths);
                    for mod_name in mod_names {
                        if installed_mod_names.contains(mod_name) {
                            println!("You already have [{mod_name}] installed.");
                            continue;
                        }

                        let downloadable_mods = dependency_graph.check_dependencies(
                            mod_name,
                            &mod_registry,
                            &installed_mod_names,
                        );

                        if downloadable_mods.is_empty() {
                            println!("All dependencies for mod [{mod_name}] are already installed");
                            continue;
                        }

                        println!("Downloading mod [{mod_name}] and its dependencies...");
                        download::download_mods_concurrently(
                            &download_client,
                            &downloadable_mods,
                            config.clone(),
                            &semaphore,
                        )
                        .await?;

                        // Prevent duplicate downloads
                        for (mod_name, _) in downloadable_mods {
                            installed_mod_names.insert(mod_name);
                        }
                    }
                }
                Commands::Update(args) => {
                    // Filter installed mods according to the `updaterblacklist.txt`
                    let mut local_mods = LocalMod::load_local_mods(&archive_paths);
                    if let Some(blacklist) = config.read_updater_blacklist()? {
                        local_mods.retain(|local_mod| !blacklist.contains(&local_mod.location));
                    }

                    // Update installed mods by checking for available updates in the mod registry.
                    let spinner = download::pb_style::create_spinner();
                    let api_client = reqwest::ClientBuilder::new()
                        .use_rustls_tls()
                        .https_only(true)
                        .http2_adaptive_window(true)
                        .gzip(true)
                        .build()
                        .unwrap_or_else(|_| reqwest::Client::new());
                    let mod_registry = RemoteModRegistry::fetch(&api_client).await?;
                    spinner.finish_and_clear();
                    drop(spinner);

                    let registry = Arc::new(mod_registry);

                    let available_updates = registry.check_updates(&local_mods);

                    if available_updates.is_empty() {
                        println!("All mods are up to date!");
                    } else if args.install {
                        println!();
                        println!("Installing updates...");
                        download::download_mods_concurrently(
                            &download_client,
                            &available_updates,
                            config,
                            &semaphore,
                        )
                        .await?;
                    } else {
                        println!();
                        println!("Run with --install to install these updates");
                    }
                }
                _ => unreachable!(),
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
