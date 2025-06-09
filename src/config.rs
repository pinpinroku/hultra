use std::path::{Path, PathBuf};

use anyhow::Result;
use thiserror::Error;

use crate::cli::Cli;

#[derive(Debug, Error)]
enum ConfigError {
    /// Error indicating that user's home directory could not be determined.
    #[error(
        "could not determine home directory location!\
        please specify the mods directory using --mods-dir"
    )]
    CouldNotDetermineHomeDirectory,
}

/// Config to manage mods.
#[derive(Debug)]
pub struct Config {
    /// The path to the directory where the mods are stored.
    directory: PathBuf,
    /// List of mirror names, separated by commas (e.g., "gb,wegfan,jade,otobot")
    mirror_preferences: String,
}

impl Config {
    pub fn new(cli: &Cli) -> Result<Self> {
        Ok(Self {
            directory: cli
                .mods_dir
                .clone()
                .unwrap_or(get_default_mods_directory()?),
            mirror_preferences: cli.mirror_preferences.to_string(),
        })
    }

    /// Path to the mods directory
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Priority of download mirrors
    pub fn mirror_preferences(&self) -> &str {
        &self.mirror_preferences
    }
}

/// Returns the default path to the mods directory.
///
/// # Errors
/// Returns `CouldNotDetermineHomeDirectory` if user's home directory could not be determined.
fn get_default_mods_directory() -> Result<PathBuf> {
    std::env::home_dir()
        .map(|home_path| home_path.join(".local/share/Steam/steamapps/common/Celeste/Mods"))
        .ok_or(ConfigError::CouldNotDetermineHomeDirectory.into())
}
