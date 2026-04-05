//! CLI layer.
//!
//! All of the command arguments are defined in this module.
//! Each modules have `run(args: Args)` function for CLI output.
//! Actual business logic like `install`, or `update` are defined in the upper modules (src/lib.rs, or core/network/download.rs).
use clap::{Args, ValueEnum};

pub mod everest;
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
