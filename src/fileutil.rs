#![allow(deprecated)]
use std::{
    collections::HashSet,
    env::home_dir,
    fs::{self, File},
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};

use tracing::info;
use xxhash_rust::xxh64::Xxh64;
use zip::{ZipArchive, result::ZipError};

use crate::constant::{MOD_MANIFEST_FILE, STEAM_MODS_DIRECTORY_PATH, UPDATER_BLACKLIST_FILE};
use crate::error::Error;

/// Returns the path to the user's mods directory based on platform-specific conventions.
///
/// # Returns
/// * `Ok(PathBuf)` - The path to the mods directory if detected successfully.
/// * `Err(Error)` - An error if the home directory could not be determined.
pub fn get_mods_directory() -> Result<PathBuf, Error> {
    info!("Detecting Celeste/Mods directory...");
    // NOTE: `std::env::home_dir()` will be undeprecated in rust 1.87.0
    home_dir()
        .map(|home_path| home_path.join(STEAM_MODS_DIRECTORY_PATH))
        .ok_or(Error::CouldNotDetermineHomeDir)
}

/// Scans the mods directory and returns a list of all installed mod archive files.
///
/// # Arguments
/// * `mods_directory` - A reference to the `Path` representing the mods directory.
///
/// # Returns
/// * `Ok(Vec<PathBuf>)` - A vector containing paths to all mod archive files found.
/// * `Err(Error)` - An error if the mods directory does not exist or cannot be read.
pub fn find_installed_mod_archives(mods_directory: &Path) -> Result<Vec<PathBuf>, Error> {
    if !mods_directory.exists() {
        return Err(Error::MissingModsDirectory);
    }

    info!("Scanning installed mod archives in {:#?}", mods_directory);

    let mut mod_archives = Vec::new();
    let entries = fs::read_dir(mods_directory)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            mod_archives.push(path);
        }
    }

    Ok(mod_archives)
}

/// Reads the mod manifest file from a given ZIP archive.
///
/// # Arguments
/// * `zip_path` - A reference to the `Path` of the ZIP archive.
///
/// # Returns
/// * `Ok(Some(Vec<u8>))` - The content of the manifest file if found.
/// * `Ok(None)` - If the manifest file is not present in the archive.
/// * `Err(Error)` - An error if the ZIP archive could not be read.
pub fn read_manifest_file_from_zip(zip_path: &Path) -> Result<Option<Vec<u8>>, Error> {
    let zip_file = File::open(zip_path)?;
    let reader = BufReader::new(zip_file);
    let mut zip_archive = ZipArchive::new(reader)?;

    match zip_archive.by_name(MOD_MANIFEST_FILE) {
        Ok(mut file) => {
            // NOTE: Max file size of `everest.yaml` should be under 10KB
            let mut buffer = Vec::with_capacity(12 * 1024);
            file.read_to_end(&mut buffer)?;

            // Check for UTF-8 BOM and remove if present
            if buffer.len() >= 3 && buffer[0] == 0xEF && buffer[1] == 0xBB && buffer[2] == 0xBF {
                buffer.drain(0..3);
            }

            Ok(Some(buffer))
        }
        Err(ZipError::FileNotFound) => Ok(None),
        Err(err) => Err(Error::Zip(err)),
    }
}

/// Computes the xxhash of a given file and returns it as a hexadecimal string.
///
/// # Arguments
/// * `file_path` - A reference to the `Path` of the file to be hashed.
///
/// # Returns
/// * `Ok(String)` - The hexadecimal representation of the file hash.
/// * `Err(Error)` - An error if the file could not be read.
pub fn hash_file(file_path: &Path) -> Result<String, Error> {
    let file = std::fs::File::open(file_path)?;
    let mut reader = std::io::BufReader::new(file);
    let mut hasher = Xxh64::new(0);
    let mut buffer = [0u8; 8192]; // Read in 8 KB chunks
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    let hash_str = format!("{:016x}", hasher.digest());
    Ok(hash_str)
}

/// Reads the updater blacklist file from the specified mods directory and returns a set of archive file paths.
///
/// # Arguments
/// * `mods_directory` - A reference to the `Path` where the updater blacklist file is stored.
///
/// # Returns
/// * `Ok(HashSet<PathBuf>)` - A HashSet containing the archive file paths if the file was read successfully.
/// * `Err(io::Error)` - An error if there was an issue reading the file.
pub fn read_updater_blacklist(mods_directory: &Path) -> Result<HashSet<PathBuf>, Error> {
    let path = mods_directory.join(UPDATER_BLACKLIST_FILE);

    // If the blacklist file is missing, return empty HashSet
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) => match err.kind() {
            std::io::ErrorKind::NotFound => return Ok(HashSet::new()),
            _ => return Err(Error::Io(err)),
        },
    };

    let reader = BufReader::new(file);

    let mut filenames = Vec::new();
    for line_result in reader.lines() {
        let line = line_result?; // Propagate any error.
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            filenames.push(trimmed.to_string());
        }
    }

    // Convert blacklist entries to full paths and store in HashSet for O(1) lookups
    let blacklisted_paths: HashSet<PathBuf> = filenames
        .into_iter()
        .map(|filename| mods_directory.join(filename))
        .collect();

    Ok(blacklisted_paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, tempdir};
    use zip::write::{SimpleFileOptions, ZipWriter};

    const MOD_MANIFEST_FILE: &str = "everest.yaml";

    // Helper function to create a zip file with a manifest
    fn create_test_zip(manifest_content: Option<&[u8]>) -> NamedTempFile {
        let temp_file = NamedTempFile::new().unwrap();
        let file = File::create(temp_file.path()).unwrap();
        let mut zip = ZipWriter::new(file);

        if let Some(content) = manifest_content {
            zip.start_file(MOD_MANIFEST_FILE, SimpleFileOptions::default())
                .unwrap();
            zip.write_all(content).unwrap();
        }

        zip.finish().unwrap();
        temp_file
    }

    #[test]
    fn test_get_mods_directory_success() {
        let mods_dir = get_mods_directory();
        assert!(mods_dir.is_ok());
        let path = mods_dir.unwrap();
        assert!(path.ends_with(STEAM_MODS_DIRECTORY_PATH));
    }

    #[test]
    fn test_find_installed_mod_archives_success() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.zip");
        File::create(&file_path).unwrap();

        let result = find_installed_mod_archives(temp_dir.path());

        assert!(result.is_ok());
        let archives = result.unwrap();
        assert_eq!(archives.len(), 1);
        assert_eq!(archives[0], file_path);
    }

    #[test]
    fn test_find_installed_mod_archives_missing_directory() {
        let nonexistent_path = Path::new("nonexistent_directory");

        let result = find_installed_mod_archives(nonexistent_path);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::MissingModsDirectory));
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
        assert!(matches!(result.unwrap_err(), Error::Io(_)));
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
        let blacklist = result.unwrap();
        assert!(blacklist.contains(&temp_dir.path().join("blacklisted_mod_1.zip")));
        assert!(blacklist.contains(&temp_dir.path().join("blacklisted_mod_2.zip")));
    }

    #[test]
    fn test_read_updater_blacklist_missing() {
        let temp_dir = tempdir().unwrap();

        let result = read_updater_blacklist(temp_dir.path());

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty())
    }

    #[test]
    fn test_read_manifest_file_success() {
        let content = b"test manifest content".to_vec();
        let temp_zip = create_test_zip(Some(&content));

        let result = read_manifest_file_from_zip(temp_zip.path());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(content));
    }

    #[test]
    fn test_read_manifest_file_with_utf8_bom() {
        let mut content = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        content.extend_from_slice(b"test manifest content");
        let expected_content = b"test manifest content".to_vec();
        let temp_zip = create_test_zip(Some(&content));

        let result = read_manifest_file_from_zip(temp_zip.path());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(expected_content));
    }

    #[test]
    fn test_read_manifest_file_not_found() {
        let temp_zip = create_test_zip(None);

        let result = read_manifest_file_from_zip(temp_zip.path());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_read_invalid_zip_file() {
        let temp_file = NamedTempFile::new().unwrap();
        File::create(temp_file.path())
            .unwrap()
            .write_all(b"not a zip file")
            .unwrap();

        let result = read_manifest_file_from_zip(temp_file.path());

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::Zip(ZipError::InvalidArchive(_))
        ));
    }

    #[test]
    fn test_read_nonexistent_file() {
        let nonexistent_path = Path::new("nonexistent.zip");

        let result = read_manifest_file_from_zip(nonexistent_path);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Io(_)));
    }
}
