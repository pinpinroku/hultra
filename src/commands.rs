//! CLI layer.
//!
//! All of the command arguments are defined in this module.
//! Each modules have `run(args: Args)` function for CLI output.
//! Actual business logic like `install`, or `update` are defined in the upper modules (src/lib.rs, or core/network/download.rs).
use std::collections::HashSet;

use clap::{Args, ValueEnum};
use tracing::warn;

pub mod everest;
pub mod install;
pub mod list;
pub mod update;

/// Supported mirrors.
#[derive(Debug, Clone, PartialEq, Eq, ValueEnum, Hash)]
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

impl Mirror {
    /// Generates the full mirror URL for a given GameBanana ID.
    fn url_for_id(&self, gbid: &str) -> String {
        match *self {
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

#[derive(Debug, Clone)]
pub struct Mirrors(pub Vec<Mirror>);

impl Mirrors {
    /// ### Example
    /// ```
    /// let mirrors = vec![Mirror::Gb, Mirror::Jade];
    /// let urls = mirrors.resolve("https://gamebanan.com/mmdl/123456");
    /// for url in urls {
    ///     println!("{}", url)
    /// }
    /// ```
    pub fn resolve(&self, url: &str) -> Vec<String> {
        let Some(gbid) = url.strip_prefix("https://gamebanana.com/mmdl/") else {
            warn!("failed to extract Gamebanana ID from '{}'", url);
            return vec![url.to_string()];
        };
        // NOTE retains order while removing duplicates
        let mut seen = HashSet::new();
        self.0
            .iter()
            .filter(|x| seen.insert(*x))
            .map(|mirror| mirror.url_for_id(gbid))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve() {
        let url = "https://gamebanana.com/mmdl/1298450";
        let mirrors: Mirrors = Mirrors(vec![Mirror::Otobot, Mirror::Gb, Mirror::Jade]);
        let result = mirrors.resolve(url);
        assert_eq!(result.len(), 3, "should return three URLs");
        assert_eq!(
            result.first().unwrap(),
            &"https://banana-mirror-mods.celestemods.com/1298450.zip".to_string()
        )
    }

    #[test]
    fn test_resolve_duplicate_entries() {
        let url = "https://gamebanana.com/mmdl/1298450";
        let mirrors: Mirrors = Mirrors(vec![Mirror::Otobot, Mirror::Otobot, Mirror::Jade]);
        let result = mirrors.resolve(url);
        assert_eq!(result.len(), 2, "should return only two URLs");
        assert_eq!(
            result.first().unwrap(),
            &"https://banana-mirror-mods.celestemods.com/1298450.zip".to_string()
        )
    }
}
