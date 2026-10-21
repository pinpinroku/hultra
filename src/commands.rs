//! CLI layer.
//!
//! All of the command arguments are defined in this module.
//! Each modules have `run(args: Args)` function for CLI output.
//! Actual business logic like `install`, or `update` are defined in the upper modules (src/lib.rs, or core/network/download.rs).
use std::collections::HashSet;

use clap::{Args, ValueEnum};

pub mod everest;
pub mod install;
pub mod list;
pub mod update;

/// Options specific to downloading.
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
        You can specify up to 5 mirrors, but providing fewer will restrict download attempts to only those mirrors.",
        default_value = "otobot,risingsunlight,gb,jade,wegfan"
    )]
    pub mirror_priority: Vec<Mirror>,

    /// Enables GitHub mirror for database retrieval.
    #[arg(short = 'm', long)]
    pub use_api_mirror: bool,

    /// Maximum number of concurrent downloads [range: 1-6]
    #[arg(short, long, default_value_t = 4, value_parser = clap::value_parser!(u8).range(1..=6))]
    pub jobs: u8,
}

/// Supported mirrors.
#[derive(Debug, Clone, PartialEq, Eq, ValueEnum, Hash)]
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
    /// France.
    RisingSunLight,
}

impl Mirror {
    /// Generates the full mirror URL for a given mirror name.
    fn url_for_name(&self, mirror_name: &str) -> String {
        match *self {
            Mirror::Gb => {
                format!("https://gamebanana.com/mmdl/{}", mirror_name)
            }
            Mirror::Jade => {
                format!(
                    "https://celestemodupdater.0x0a.de/banana-mirror/{}.zip",
                    mirror_name
                )
            }
            Mirror::Wegfan => {
                format!(
                    "https://celeste.weg.fan/api/v2/download/gamebanana-files/{}",
                    mirror_name
                )
            }
            Mirror::Otobot => {
                format!(
                    "https://banana-mirror-mods.celestemods.com/{}.zip",
                    mirror_name
                )
            }
            Mirror::RisingSunLight => format!(
                "https://library.risingsunlight.dev/celeste/mods/{}.zip",
                mirror_name
            ),
        }
    }
}

/// Represents mirror priority.
#[derive(Debug, Clone)]
pub struct Mirrors(Vec<Mirror>);

impl From<Vec<Mirror>> for Mirrors {
    /// Converts to this type from user input.
    fn from(value: Vec<Mirror>) -> Self {
        Self(value)
    }
}

impl Mirrors {
    /// Resolves Mirrors into actual list of mirror URLs.
    ///
    /// ### Example
    ///
    /// ```
    /// let mirrors = vec![Mirror::Gb, Mirror::Jade, Mirror::Wegfan];
    /// let urls = mirrors.resolve("123456");
    /// for url in urls {
    ///     println!("URL: {}", url)
    /// }
    /// ```
    pub fn resolve(&self, mirror_name: &str) -> Vec<String> {
        // NOTE retains order while removing duplicates
        let mut seen = HashSet::new();
        self.0
            .iter()
            .filter(|x| seen.insert(*x))
            .map(|mirror| mirror.url_for_name(mirror_name))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve() {
        let mirrors: Mirrors = Mirrors(vec![Mirror::Otobot, Mirror::Gb, Mirror::Jade]);
        let result = mirrors.resolve("1298450");
        assert_eq!(result.len(), 3, "should return three URLs");
        assert_eq!(
            result.first().unwrap(),
            &"https://banana-mirror-mods.celestemods.com/1298450.zip".to_string()
        )
    }

    #[test]
    fn test_resolve_duplicate_entries() {
        let mirrors: Mirrors = Mirrors(vec![Mirror::Otobot, Mirror::Otobot, Mirror::Jade]);
        let result = mirrors.resolve("1298450");
        assert_eq!(result.len(), 2, "should return only two URLs");
        assert_eq!(
            result.first().unwrap(),
            &"https://banana-mirror-mods.celestemods.com/1298450.zip".to_string()
        )
    }
}
