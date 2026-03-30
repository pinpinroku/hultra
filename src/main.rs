use anyhow::Context;
use clap::Parser;
use tracing::{debug, info};

use crate::{
    cli::{Cli, Command, EverestSubCommand, NetworkCommand},
    config::{AppConfig, CARGO_PKG_NAME, CARGO_PKG_VERSION},
    everest::{client::EverestClient, version},
};

mod archive;
mod cache;
mod cli;
mod commands;
mod config;
mod core;
mod dependency;
mod everest;
mod local_mods;
mod log;
mod mirror;
mod registry;
mod ui;
mod update;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    log::init_logger(args.log_file.as_deref()).with_context(|| {
        format!(
            "Failed to initialize logging system. Cannot create log file at {:?}",
            args.log_file.as_deref()
        )
    })?;

    info!("{} version {}", CARGO_PKG_NAME, CARGO_PKG_VERSION);
    debug!("\n{:#?}", args);

    // Init app config
    let config = AppConfig::new(args.directory.as_deref())?;

    match args.commands {
        Command::List => commands::list::run(&config)?,
        Command::Install(args) => commands::install::run(&args, &config).await?,
        Command::Update(args) => commands::update::run(&args, &config).await?,
        Command::Everest(subcommand) => match subcommand {
            EverestSubCommand::Version => {
                let current_v = version::ensure_installed_version(config.root_dir())?;
                println!("{}", current_v);
                return Ok(());
            }
            EverestSubCommand::NetworkRequired(action) => {
                let option = action.network_option();
                let client = EverestClient::new()?;
                let builds = client.fetch_database(option.use_api_mirror).await?;

                match action {
                    NetworkCommand::List { all, limit, .. } => {
                        let display_n = if all { builds.len() } else { limit };
                        everest::print_builds(builds, display_n)
                    }
                    NetworkCommand::Update(_) => {
                        let current_v = version::ensure_installed_version(config.root_dir())?;
                        let current_b = version::get_installed_branch(&builds, &current_v)
                            .context("Installed version not found on the database")?;
                        let target_build = version::get_latest_build_on_branch(&builds, current_b)
                            .context("No builds found on the branch")?;
                        debug!(?target_build, ?current_v, ?current_b);
                        if current_v == target_build.version {
                            println!("Everest is up-to-date");
                            println!("  {}", target_build);
                            return Ok(());
                        }
                        client
                            .download_and_run_installer(target_build, &config)
                            .await?;
                    }
                    NetworkCommand::Install { version, .. } => {
                        let target_build = builds
                            .iter()
                            .find(|b| b.version == version)
                            .context("Specified version is not available")?;
                        client
                            .download_and_run_installer(target_build, &config)
                            .await?;
                    }
                }
            }
        },
    }
    Ok(())
}
