use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, about = "Mod management tool for Celeste", long_about = None)]
pub struct Cli {
    /// Directory where mods are stored. This option applies to all commands.
    #[arg(short = 'd', long = "mods-dir", value_name = "DIR")]
    pub mods_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

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

#[derive(Debug, Args)]
pub struct InstallArgs {
    /// Mod name
    pub name: String,
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    /// Mod name
    pub name: String,
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Install available updates
    #[arg(long, action)]
    pub install: bool,
}
