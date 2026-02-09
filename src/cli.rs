use std::{ops::Deref, path::PathBuf, str::FromStr};

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::download::DbBaseUrl;

/// Supported mirrors.
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
                format!("https://gamebanana.com/mmdl/{}", gbid)
            }
            Mirror::Jade => {
                format!(
                    "https://celestemodupdater.0x0a.de/banana-mirror/{}.zip",
                    gbid
                )
            }
            Mirror::Wegfan => {
                format!(
                    "https://celeste.weg.fan/api/v2/download/gamebanana-files/{}",
                    gbid
                )
            }
            Mirror::Otobot => {
                format!("https://banana-mirror-mods.celestemods.com/{}.zip", gbid)
            }
        }
    }
}

/// Command line interface.
#[derive(Debug, Clone, Parser)]
#[command(version, about = "A simple cli tool to update/install mods for Celeste.", long_about = None)]
pub struct Cli {
    /// Subcommands of the CLI.
    #[command(subcommand)]
    pub commands: Command,

    /// Directory where mods are installed.
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

    /// Installs mods from GameBanana URLs.
    Install {
        /// URL(s) of mod page on GameBanana.
        #[arg(required = true, num_args = 1..20)]
        urls: Vec<GamebananaUrl>,

        /// Options specific to downloading.
        #[command(flatten)]
        option: DownloadOption,
    },

    /// Updates mods.
    Update(DownloadOption),
}

#[derive(Debug, Clone, Args)]
pub struct DownloadOption {
    /// Comma-separated list of mirror priorities.
    #[arg(
        value_enum,
        short = 'p',
        long = "mirror-priority",
        value_name = "MIRROR",
        value_delimiter = ',',
        long_help = "Comma-separated list of mirror priorities.
        This option allows you to specify the order in which mirrors should be tried when downloading mods.
        You can specify up to 4 mirrors, but providing fewer will restrict download attempts to only those mirrors.",
        default_value = "otobot,gb,jade,wegfan"
    )]
    pub mirror_priority: Vec<Mirror>,

    /// Enables GitHub mirror for database retrieval.
    #[arg(short = 'm', long)]
    pub use_api_mirror: bool,

    /// Maximum number of concurrent downloads [range: 1-6]
    #[arg(short, long, default_value_t = 4, value_parser = clap::value_parser!(u8).range(1..=6))]
    pub jobs: u8,
}

impl DownloadOption {
    pub fn url_set(&self) -> DbBaseUrl {
        match self.use_api_mirror {
            true => DbBaseUrl::Mirror,
            false => DbBaseUrl::Primary,
        }
    }

    pub fn mirror_priority(&self) -> &Vec<Mirror> {
        &self.mirror_priority
    }
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
