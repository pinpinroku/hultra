use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use reqwest::Url;

use crate::error::ModPageUrlParseError;

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

        * gb     => 'Default GameBanana Server (United States)',
        * jade   => 'Germany',
        * wegfan => 'China',
        * otobot => 'North America',

        If the download from the current server fails, the application will
        automatically fall back to the next server in the priority list to
        retry the download. You can also restrict the fallback servers by
        providing a comma-separated list (e.g., \"otobot,jade\"), which will
        limit the retries to only those specified servers.",
        default_value = "otobot,gb,jade,wegfan"
    )]
    pub mirror_preferences: String,

    /// Verbose mode: Write verbose logs to the file
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

impl InstallArgs {
    /// Parses the given url string, converts it to the mod ID.
    ///
    /// # Errors
    /// Returns an error if the URL is invalid, has an unsupported scheme,
    /// or does not match the expected GameBanana mod page format.
    pub fn parse_mod_page_url(&self) -> Result<u32, ModPageUrlParseError> {
        let page_url_str = &self.mod_page_url;
        let page_url = Url::parse(page_url_str)
            .map_err(|_| ModPageUrlParseError::InvalidUrl(page_url_str.to_owned()))?;

        // Check scheme
        match page_url.scheme() {
            "http" | "https" => {}
            _ => {
                return Err(ModPageUrlParseError::UnsupportedScheme(
                    page_url_str.to_owned(),
                ));
            }
        }

        // Check host
        if page_url.host_str() != Some("gamebanana.com") {
            return Err(ModPageUrlParseError::InvalidGameBananaUrl(page_url.clone()));
        }

        // Check path segments
        let mut segments = page_url
            .path_segments()
            .ok_or_else(|| ModPageUrlParseError::InvalidGameBananaUrl(page_url.clone()))?;

        // Expected path: /mods/12345
        match (segments.next(), segments.next()) {
            (Some("mods"), Some(id_str)) => {
                let id = id_str
                    .parse::<u32>()
                    .map_err(|_| ModPageUrlParseError::InvalidModId(id_str.to_owned()))?;
                Ok(id)
            }
            _ => Err(ModPageUrlParseError::InvalidGameBananaUrl(page_url.clone())),
        }
    }
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

#[cfg(test)]
mod tests_page_url {
    use super::*;

    #[test]
    fn test_valid_url() {
        let args = InstallArgs {
            mod_page_url: "https://gamebanana.com/mods/12345".to_string(),
        };
        assert_eq!(args.parse_mod_page_url().unwrap(), 12345);
    }

    #[test]
    fn test_invalid_scheme() {
        let args = InstallArgs {
            mod_page_url: "ftp://gamebanana.com/mods/12345".to_string(),
        };
        assert!(args.parse_mod_page_url().is_err());
    }

    #[test]
    fn test_invalid_host() {
        let args = InstallArgs {
            mod_page_url: "https://example.com/mods/12345".to_string(),
        };
        assert!(args.parse_mod_page_url().is_err());
    }

    #[test]
    fn test_missing_id() {
        let args = InstallArgs {
            mod_page_url: "https://gamebanana.com/mods/".to_string(),
        };
        assert!(args.parse_mod_page_url().is_err());
    }

    #[test]
    fn test_non_numeric_id() {
        let args = InstallArgs {
            mod_page_url: "https://gamebanana.com/mods/abc".to_string(),
        };
        assert!(args.parse_mod_page_url().is_err());
    }
}
