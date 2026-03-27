//! Network options for the mod management.
use clap::Args;

use crate::{cli::mirror::Mirror, download::DbBaseUrl};

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
