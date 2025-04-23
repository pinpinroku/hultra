use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// The main CLI structure for the Everest Mod CLI application
#[derive(Debug, Parser)]
#[command(version, about = "Mod management tool for Celeste", long_about = None)]
pub struct Cli {
    /// Directory where mods are stored. This option applies to all commands
    #[arg(short = 'd', long = "mods-dir", value_name = "DIR")]
    pub mods_dir: Option<PathBuf>,

    /// The subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// The set of available subcommands for the Everest Mod CLI
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Install a mod
    Install(InstallArgs),
    /// List installed mods
    List,
    /// Show detailed information about an installed mod
    Show(ShowArgs),
    /// Check for updates
    Update(UpdateArgs),
}

/// Arguments for the `install` subcommand
#[derive(Debug, Args)]
pub struct InstallArgs {
    /// The name of the mod to install
    pub name: String,
}

/// Arguments for the `show` subcommand
#[derive(Debug, Args)]
pub struct ShowArgs {
    /// The name of the mod to show details for
    pub name: String,
}

/// Arguments for the `update` subcommand
#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Install available updates
    #[arg(long, action)]
    pub install: bool,
}
