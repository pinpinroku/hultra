use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use thiserror::Error;

use crate::{cli::Cli, constant::STEAM_MODS_DIRECTORY_PATH, fileutil::replace_home_dir_with_tilde};

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

    /// Scans the mods directory and returns a list of all installed mod archive files.
    ///
    /// # Errors
    ///
    /// If the mods directory does not exist or cannot be read, an error is returned.
    pub fn find_installed_mod_archives(&self) -> anyhow::Result<Vec<PathBuf>> {
        let debug_filename = replace_home_dir_with_tilde(&self.directory);
        if !self.directory.exists() {
            anyhow::bail!("The mods directory does not exist: {}", debug_filename);
        }

        tracing::debug!("Scanning the installed mod archives in {}", debug_filename);

        let directory_entries = fs::read_dir(&self.directory)
            .map_err(|e| anyhow::anyhow!("Failed to read mods directory: {}", e))?;
        let mod_archives = directory_entries
            .flatten() // eliminates unreadable directory entries
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
            })
            .collect::<Vec<PathBuf>>();

        tracing::info!("Found local mod files: {}", mod_archives.len());

        Ok(mod_archives)
    }
}

/// Returns the path to the mods directory.
///
/// If the user's home directory could not be determined, it returns None.
fn get_default_mods_directory() -> Option<PathBuf> {
    env::home_dir().map(|home_path| home_path.join(STEAM_MODS_DIRECTORY_PATH))
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::tempdir;

    #[test]
    fn test_find_installed_mod_archives_success() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.zip");
        fs::File::create(&file_path).unwrap();

        // Create a Config instance with the temp_dir as the mods directory
        let config = Config {
            directory: temp_dir.path().to_path_buf(),
            mirror_preferences: String::new(),
        };

        let result = config.find_installed_mod_archives();

        assert!(result.is_ok());
        let archives = result.unwrap();
        assert_eq!(archives.len(), 1);
        assert_eq!(archives[0], file_path);
    }

    #[test]
    fn test_find_installed_mod_archives_missing_directory() {
        let nonexistent_path = Path::new("nonexistent_directory");

        let config = Config {
            directory: nonexistent_path.to_path_buf(),
            mirror_preferences: String::new(),
        };

        let result = config.find_installed_mod_archives();

        assert!(result.is_err());
    }
}
