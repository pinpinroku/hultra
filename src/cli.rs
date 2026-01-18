//! Command line interface for the mod management tool.
use std::{ops::Deref, path::PathBuf, str::FromStr};

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Enum representing supported mirrors.
#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
pub enum Mirror {
    /// Default GameBanana Server (United States).
    Gb,
    /// Germany.
    Jade,
    /// China.
    Wegfan,
    /// North America.
    Otobot,
}

impl Mirror {
    /// Generates the full mirror URL for a given GameBanana ID.
    pub fn url_for_id(&self, gbid: u32) -> String {
        match self {
            Mirror::Gb => {
                // The original GameBanana download URL format.
                format!("https://gamebanana.com/mmdl/{}", gbid)
            }
            Mirror::Jade => {
                // The Jade mirror URL template.
                format!(
                    "https://celestemodupdater.0x0a.de/banana-mirror/{}.zip",
                    gbid
                )
            }
            Mirror::Wegfan => {
                // The Wegfan mirror URL template.
                format!(
                    "https://celeste.weg.fan/api/v2/download/gamebanana-files/{}",
                    gbid
                )
            }
            Mirror::Otobot => {
                // The Otobot mirror URL template.
                format!("https://banana-mirror-mods.celestemods.com/{}.zip", gbid)
            }
        }
    }
}

/// Command line interface.
#[derive(Debug, Parser)]
#[command(version, about = "A simple cli tool to update/install mods for Celeste.", long_about = None)]
pub struct Cli {
    /// Subcommands of the CLI.
    #[command(subcommand)]
    pub commands: Command,

    /// Directory where mods are installed.
    #[arg(short = 'd', long = "directory", value_name = "DIR")]
    pub directory: Option<PathBuf>,

    /// Priority of the mirror list separated by commas.
    #[arg(
        value_enum,
        short = 'm',
        long = "mirror-priority",
        value_name = "MIRROR",
        value_delimiter = ',',
        long_help = "Priority of the mirror list separated by commas.
        This option allows you to specify the order in which mirrors should be tried when downloading mods.
        You can specify up to 4 mirrors, but providing fewer will restrict download attempts to only those mirrors.",
        default_value = "otobot,gb,jade,wegfan"
    )]
    pub mirror_priority: Vec<Mirror>,

    /// Use a GitHub mirror for database retrieval to reduce pre-processing time.
    #[arg(long)]
    pub use_api_mirror: bool,

    /// Verbose mode.
    #[arg(short, long)]
    pub verbose: bool,
}

/// Subcommands of the CLI.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// List installed mods.
    List,
    /// Install mods by the URL of the GameBanana page where the mod featured on.
    Install(InstallArgs),
    /// Update installed mods.
    Update,
}

/// Arguments of `Install` command.
#[derive(Debug, Args)]
pub struct InstallArgs {
    /// URL(s) of mod page on GameBanana.
    #[arg(required = true, num_args = 1..20)]
    pub urls: Vec<GamebananaUrl>,
}

#[derive(thiserror::Error, Debug)]
pub enum ArgumentError {
    #[error(
        "last path segment of URL must be a positive integer up to {}",
        u32::MAX
    )]
    ParseLastSegAsInt(#[from] std::num::ParseIntError),
    #[error("it must be starts with 'https://gamebanana.com/mods/'")]
    InvalidUrl,
}

#[derive(Debug, Clone)]
pub struct GamebananaUrl(String);

impl FromStr for GamebananaUrl {
    type Err = ArgumentError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        s.strip_prefix("https://gamebanana.com/mods/")
            .ok_or(ArgumentError::InvalidUrl)?
            .parse::<u32>()?;
        Ok(GamebananaUrl(s.to_string()))
    }
}

impl Deref for GamebananaUrl {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl GamebananaUrl {
    pub fn extract_id(&self) -> Result<u32, ArgumentError> {
        let id_part = self
            .0
            .strip_prefix("https://gamebanana.com/mods/")
            .ok_or(ArgumentError::InvalidUrl)?;
        let id = id_part.parse()?;
        Ok(id)
    }
}
