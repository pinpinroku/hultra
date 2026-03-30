//! CLI layer.
//!
//! All of the command arguments are defined in this module.
//! Each modules have `run(args: Args)` function for CLI output.
//! Actual business logic like `install`, or `update` are defined in the upper modules (src/lib.rs, or core/network/download.rs).
use clap::{Args, ValueEnum};

pub mod install;
pub mod list;
pub mod update;

/// Supported mirrors.
#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
pub enum Mirror {
    /// Default GameBanana Server (United States). US
    Gb,
    /// Germany. DE
    Jade,
    /// China. CN
    Wegfan,
    /// North America. NA
    Otobot,
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

#[derive(Debug, Args, Clone)]
pub struct InstallArgs {
    /// URL(s) of mod page on GameBanana.
    #[arg(required = true, num_args = 1..20)]
    pub urls: Vec<GamebananaUrl>,

    /// Options specific to downloading.
    #[command(flatten)]
    pub option: DownloadOption,
}

use std::{ops::Deref, str::FromStr};

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
