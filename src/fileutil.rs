use std::{
    borrow::Cow,
    collections::HashSet,
    env,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};

use anyhow::Context;
use xxhash_rust::xxh64::Xxh64;

use crate::constant::UPDATER_BLACKLIST_FILE;
use crate::error::Error;

/// Replaces `/home/user/` with `~/`
pub fn replace_home_dir_with_tilde(destination: &Path) -> Cow<'_, str> {
    // Get the home directory
    let home = match env::home_dir() {
        Some(h) => h,
        None => return destination.to_string_lossy(),
    };

    // Try to strip the home directory prefix
    match destination.strip_prefix(&home) {
        Ok(relative_path) => Cow::Owned(format!("~/{}", relative_path.display())),
        Err(_) => destination.to_string_lossy(),
    }
}

/// Computes the xxhash of a given file and returns it as a hexadecimal string.
///
/// # Errors
/// Returns an error if the file cannot be opened or read.
pub fn hash_file(file_path: &Path) -> anyhow::Result<String> {
    let debug_filename = replace_home_dir_with_tilde(file_path);
    tracing::debug!("Computing checksum for {}", debug_filename);

    let file = File::open(file_path)
        .with_context(|| format!("failed to open the file '{}'", debug_filename))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Xxh64::new(0);
    let mut buffer = [0u8; 64 * 1024]; // Read in 64 KB chunks
    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .context("failed to read the buffer")?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    let hash_str = format!("{:016x}", hasher.digest());
    Ok(hash_str)
}

/// Returns a set of file paths if any are found in the `updaterblacklist.txt`.
///
/// Returns `None` if the file is not found in the given mods directory.
///
/// # Errors
/// Returns an error if the file cannot be opened.
pub fn read_updater_blacklist(mods_directory: &Path) -> Result<Option<HashSet<PathBuf>>, Error> {
    tracing::info!("Checking for the blacklisted mods...");
    let path = mods_directory.join(UPDATER_BLACKLIST_FILE);

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
                filenames.insert(mods_directory.join(trimmed));
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

#[cfg(test)]
mod tests_fileutil {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, tempdir};

    #[test]
    fn test_replace_home_dir() {
        let home = env::home_dir().unwrap();
        let path = home.join("documents/file.txt");
        assert_eq!(replace_home_dir_with_tilde(&path), "~/documents/file.txt");
    }

    #[test]
    fn test_non_home_dir() {
        let path = Path::new("/etc/config.txt");
        assert_eq!(replace_home_dir_with_tilde(path), "/etc/config.txt");
    }

    #[test]
    fn test_hash_file_success() {
        let temp_file = NamedTempFile::new().unwrap();
        write!(temp_file.as_file(), "test data").unwrap();

        let result = hash_file(temp_file.path());

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 16); // Should return a valid 16-character hash
    }

    #[test]
    fn test_hash_file_nonexistent() {
        let nonexistent_path = Path::new("nonexistent_file");

        let result = hash_file(nonexistent_path);

        assert!(result.is_err());
    }

    #[test]
    fn test_read_updater_blacklist_success() {
        let temp_dir = tempdir().unwrap();
        let blacklist_file = temp_dir.path().join(UPDATER_BLACKLIST_FILE);

        let mut file = File::create(&blacklist_file).unwrap();
        writeln!(file, "blacklisted_mod_1.zip").unwrap();
        writeln!(file, "blacklisted_mod_2.zip").unwrap();

        let result = read_updater_blacklist(temp_dir.path());
        assert!(result.is_ok());

        let optional_blacklist = result.unwrap();
        assert!(optional_blacklist.is_some());

        let blacklist = optional_blacklist.unwrap();
        assert!(blacklist.contains(&temp_dir.path().join("blacklisted_mod_1.zip")));
        assert!(blacklist.contains(&temp_dir.path().join("blacklisted_mod_2.zip")));
    }

    #[test]
    fn test_read_updater_blacklist_missing() {
        let temp_dir = tempdir().unwrap();

        let result = read_updater_blacklist(temp_dir.path());
        assert!(result.is_ok());

        let optional_blacklist = result.unwrap();
        assert!(optional_blacklist.is_none());
    }
}
