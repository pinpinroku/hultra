//! Command list and global options.
//!
//! TODO: Move match arms in main.rs to here
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::{
    commands::{
        self, DownloadOption,
        everest::{EverestSubCommand, network::NetworkCommand},
        install::InstallArgs,
    },
    config::AppConfig,
    everest::client::EverestClient,
};

/// Command line interface.
#[derive(Debug, Clone, Parser)]
#[command(version, about = "A simple cli tool to update/install mods for Celeste.", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub commands: Command,

    /// Directory where the Celeste is installed.
    #[arg(short = 'd', long = "directory", value_name = "DIR", global = true)]
    pub directory: Option<PathBuf>,

    /// Writes logs to the specified file.
    #[arg(long, value_name = "PATH", global = true)]
    pub log_file: Option<PathBuf>,
}

/// Subcommands of the CLI.
#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// List installed mods.
    List,

    /// Install mods from the GameBanana URLs.
    Install(InstallArgs),

    /// Update mods.
    Update(DownloadOption),

    /// Manage Everest.
    #[command(subcommand)]
    Everest(EverestSubCommand),
}

pub async fn dispatch(args: Cli, config: AppConfig) -> anyhow::Result<()> {
    match args.commands {
        Command::List => commands::list::run(&config)?,
        Command::Install(args) => commands::install::run(&args, &config).await?,
        Command::Update(args) => commands::update::run(&args, &config).await?,
        Command::Everest(subcommand) => match subcommand {
            EverestSubCommand::Version => commands::everest::version::run(&config)?,
            EverestSubCommand::NetworkRequired(action) => {
                let option = action.network_option();
                let client = EverestClient::new()?;
                let builds = client.fetch_database(option.use_api_mirror).await?;

                match action {
                    NetworkCommand::List(args) => {
                        commands::everest::network::list::run(&args, &builds)
                    }
                    NetworkCommand::Update(_) => {
                        commands::everest::network::update::run(&config, &builds, &client).await?
                    }
                    NetworkCommand::Install(args) => {
                        commands::everest::network::install::run(&args, &builds, &client, &config)
                            .await?
                    }
                }
            }
        },
    }
    Ok(())
}
