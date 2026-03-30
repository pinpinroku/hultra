//! Command list and global options.
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::commands::{DownloadOption, InstallArgs};

pub use everest::{EverestSubCommand, NetworkCommand};

mod everest;

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
