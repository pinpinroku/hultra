use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// The main CLI structure for the Everest Mod CLI application
#[derive(Debug, Parser)]
#[command(version, about = "Mod management tool for Celeste", long_about = None)]
pub struct Cli {
    /// Directory where mods are stored. This option applies to all commands
    #[arg(short = 'd', long = "mods-dir", value_name = "DIR")]
    pub mods_directory: Option<PathBuf>,

    /// Priority of the mirror list separated by commas
    #[arg(
        short = 'm',
        long = "mirror-priority",
        value_name = "MIRROR",
        long_help = "Priority of the mirror list separated by commas (e.g., \"wegfan,jade,gb,otobot\").
        This option only applies to the `install` and the `update` commands,

        * gb     => 'Default GameBanana Server',
        * jade   => 'Germany',
        * wegfan => 'China',
        * otobot => 'North America',

        If the download from the current server fails, the application will
        automatically fall back to the next server in the priority list to
        retry the download. You can also restrict the fallback servers by
        providing a comma-separated list (e.g., \"otobot,jade\"), which will
        limit the retries to only those specified servers.",
        default_value = "gb,jade,wegfan,otobot"
    )]
    pub mirror_preferences: String,

    /// Verbose mode: Display verbose outputs
    #[arg(short, long)]
    pub verbose: bool,

    /// The subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// The set of available subcommands for the Everest Mod CLI
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Install a mod using the URL
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
    /// The URL of the page where the mod is featured on the GameBanana
    pub mod_page_url: String,
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
