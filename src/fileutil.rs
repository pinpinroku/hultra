use std::{
    borrow::Cow,
    env,
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use anyhow::{Context, Result};
use xxhash_rust::xxh64::Xxh64;

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
pub fn hash_file(file_path: &Path) -> Result<String> {
    let file = File::open(file_path)?;
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

#[cfg(test)]
mod tests_fileutil {
    use super::*;

    use std::io::Write;
    use tempfile::NamedTempFile;

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
}
