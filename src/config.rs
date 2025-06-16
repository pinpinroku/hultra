use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
};

use thiserror::Error;

use crate::{cli::Cli, constant::STEAM_MODS_DIRECTORY_PATH};

/// Configuration errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Error indicating that user's home directory could not be determined.
    #[error(
        "could not determine home directory location!\
        please specify the mods directory using --mods-dir"
    )]
    CouldNotDetermineHomeDirectory,
}

/// Config to manage mods.
#[derive(Debug, Clone)]
pub struct Config {
    /// The path to the directory where the mods are stored.
    directory: PathBuf,
    /// List of mirror names, separated by commas (e.g., "gb,wegfan,jade,otobot")
    mirror_preferences: String,
}

impl Config {
    /// Returns an instance of this type.
    ///
    /// If the directory field is `None`, it will fall back to the default.
    ///
    /// # Errors
    ///
    /// If the user's home directory could not be determined, an error is returned.
    pub fn new(cli: &Cli) -> Result<Arc<Self>, ConfigError> {
        let directory = cli
            .mods_directory
            .clone()
            .or_else(get_default_mods_directory)
            .ok_or(ConfigError::CouldNotDetermineHomeDirectory)?;

        Ok(Arc::new(Self {
            directory,
            mirror_preferences: cli.mirror_preferences.to_string(),
        }))
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

/// Returns the path to the mods directory.
///
/// If the user's home directory could not be determined, it returns None.
fn get_default_mods_directory() -> Option<PathBuf> {
    env::home_dir().map(|home_path| home_path.join(STEAM_MODS_DIRECTORY_PATH))
}
