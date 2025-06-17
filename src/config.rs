use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};

use crate::{
    cli::Cli,
    constant::{STEAM_MODS_DIRECTORY_PATH, UPDATER_BLACKLIST_FILE},
    fileutil,
};

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
    /// If the user's home directory could not be determined, an error is returned.
    pub fn new(cli: &Cli) -> Result<Arc<Self>> {
        let directory = cli
            .mods_directory
            .clone()
            .or_else(get_default_mods_directory)
            .context(
                "could not determine home directory location!\
                please specify the mods directory using --mods-dir",
            )?;

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
    /// If the mods directory does not exist or cannot be read, an error is returned.
    pub fn find_installed_mod_archives(&self) -> Result<Vec<PathBuf>> {
        let debug_filename = fileutil::replace_home_dir_with_tilde(&self.directory);
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

    /// Returns a set of file paths if any are found in the `updaterblacklist.txt`.
    ///
    /// Returns `None` if the file is not found in the given mods directory.
    ///
    /// # Errors
    /// Returns an error if the file cannot be opened.
    pub fn read_updater_blacklist(&self) -> Result<Option<HashSet<PathBuf>>> {
        tracing::info!("Checking for the blacklisted mods...");
        let path = self.directory.join(UPDATER_BLACKLIST_FILE);

        if !path.exists() {
            return Ok(None);
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);

        // NOTE: Stores the results in HashSet for O(1) lookups
        let mut filenames: HashSet<PathBuf> = HashSet::new();
        for (line_number, line_result) in reader.lines().enumerate() {
            match line_result {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        tracing::debug!("Skipping line {}: '{}'", line_number + 1, trimmed);
                        continue;
                    }
                    // NOTE: It is easier to compare them as full paths.
                    filenames.insert(self.directory.join(trimmed));
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to read line {} in {}: {}",
                        line_number + 1,
                        path.display(),
                        e
                    );
                    continue;
                }
            }
        }

        tracing::debug!("Blacklist contains {} entries.", filenames.len());

        Ok(Some(filenames))
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
    use std::io::Write;
    use tempfile::{TempDir, tempdir};

    /// Helper to create a Config with a temp mods directory.
    fn config_with_temp_dir() -> (Config, TempDir) {
        let temp_dir = tempdir().unwrap();
        let config = Config {
            directory: temp_dir.path().to_path_buf(),
            mirror_preferences: String::new(),
        };
        (config, temp_dir)
    }

    #[test]
    fn test_find_installed_mod_archives_success() {
        let (config, temp_dir) = config_with_temp_dir();
        let file_path = temp_dir.path().join("test.zip");
        fs::File::create(&file_path).unwrap();

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

    #[test]
    fn test_read_updater_blacklist_success() {
        let (config, temp_dir) = config_with_temp_dir();
        let blacklist_file = temp_dir.path().join(UPDATER_BLACKLIST_FILE);

        let mut file = File::create(&blacklist_file).unwrap();
        writeln!(file, "blacklisted_mod_1.zip").unwrap();
        writeln!(file, "blacklisted_mod_2.zip").unwrap();

        let result = config.read_updater_blacklist();
        assert!(result.is_ok());

        let optional_blacklist = result.unwrap();
        assert!(optional_blacklist.is_some());

        let blacklist = optional_blacklist.unwrap();
        assert!(blacklist.contains(&temp_dir.path().join("blacklisted_mod_1.zip")));
        assert!(blacklist.contains(&temp_dir.path().join("blacklisted_mod_2.zip")));
    }

    #[test]
    fn test_read_updater_blacklist_missing() {
        let (config, _) = config_with_temp_dir();
        let result = config.read_updater_blacklist();
        assert!(result.is_ok());

        let optional_blacklist = result.unwrap();
        assert!(optional_blacklist.is_none());
    }
}
