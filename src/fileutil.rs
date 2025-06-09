use std::{
    borrow::Cow,
    collections::HashSet,
    env,
    fs::{self, File},
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};

use xxhash_rust::xxh64::Xxh64;
use zip::{ZipArchive, result::ZipError};

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

    tracing::debug!(
        "Scanning the installed mod archives in {:?}",
        replace_home_dir_with_tilde(mods_directory)
    );

    let mut mod_archives = Vec::new();
    for entry in fs::read_dir(mods_directory)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
            && path.is_file()
        {
            mod_archives.push(path);
        }
    }

    tracing::debug!("Number of mod files found: {}", mod_archives.len());

    Ok(mod_archives)
}

/// Search manifest file in the zip archive
///
/// # Arguments
/// * `zip_archive` - A mutable reference to the `ZipArchive`.
/// * `filename` - A manifest filename which should be "^everest\.[yaml|yml]$"
///
/// # Returns
/// * `Ok(Some(Vec<u8>))` - The content of the manifest file if found.
/// * `Ok(None)` - If the manifest file is not present in the archive.
/// * `Err(Error)` - An error if the ZIP archive could not be read.
fn read_manifest_from_zip(
    zip_archive: &mut ZipArchive<std::io::BufReader<std::fs::File>>,
    filename: &str,
) -> Result<Option<Vec<u8>>, Error> {
    match zip_archive.by_name(filename) {
        Ok(mut file) => {
            // NOTE: Max file size of `everest.yaml` should be under 10KB
            let mut buffer = Vec::with_capacity(12 * 1024);
            file.read_to_end(&mut buffer)?;

            // Check for UTF-8 BOM and remove if present
            if buffer.starts_with(&[0xEF, 0xBB, 0xBF]) {
                buffer.drain(0..3);
            }
            Ok(Some(buffer))
        }
        Err(ZipError::FileNotFound) => Ok(None),
        Err(err) => Err(Error::Zip(err)),
    }
}

/// Reads the mod manifest file from a given ZIP archive path.
///
/// # Arguments
/// * `archive_path` - A reference to the `Path` of the ZIP archive.
///
/// # Returns
/// * `Ok(Some(Vec<u8>))` - The content of the manifest file if found.
/// * `Ok(None)` - If the manifest file is not present in the ZIP archive.
/// * `Err(Error)` - An error if the ZIP archive could not be read.
pub fn read_manifest_file_from_archive(archive_path: &Path) -> Result<Vec<u8>, Error> {
    let file = File::open(archive_path)?;
    let reader = BufReader::new(file);
    let mut zip_archive = ZipArchive::new(reader)?;

    if let Some(content) = read_manifest_from_zip(&mut zip_archive, "everest.yaml")? {
        return Ok(content); // Return early if found, to prevent duplicate mutable borrows
    }

    // Fallback to alternative filename
    match read_manifest_from_zip(&mut zip_archive, "everest.yml")? {
        Some(content) => Ok(content),
        None => Err(Error::MissingManifestFile(archive_path.to_path_buf())),
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
pub async fn hash_file(file_path: &Path) -> Result<String, Error> {
    use tokio::{
        fs::File,
        io::{AsyncReadExt, BufReader},
    };

    let file = File::open(file_path).await?;
    let mut reader = BufReader::new(file);
    let mut hasher = Xxh64::new(0);
    let mut buffer = [0u8; 64 * 1024]; // Read in 64 KB chunks
    loop {
        let bytes_read = reader.read(&mut buffer).await?;
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

    #[tokio::test]
    async fn test_hash_file_success() {
        let temp_file = NamedTempFile::new().unwrap();
        write!(temp_file.as_file(), "test data").unwrap();

        let result = hash_file(temp_file.path()).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 16); // Should return a valid 16-character hash
    }

    #[tokio::test]
    async fn test_hash_file_nonexistent() {
        let nonexistent_path = Path::new("nonexistent_file");

        let result = hash_file(nonexistent_path).await;

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

    #[test]
    fn test_read_manifest_file_success() {
        let content = b"test manifest content".to_vec();
        let temp_zip = create_test_zip(Some(&content));

        let result = read_manifest_file_from_archive(temp_zip.path());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), content);
    }

    #[test]
    fn test_read_manifest_file_with_utf8_bom() {
        let mut content = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        content.extend_from_slice(b"test manifest content");
        let expected_content = b"test manifest content".to_vec();
        let temp_zip = create_test_zip(Some(&content));

        let result = read_manifest_file_from_archive(temp_zip.path());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_content);
    }

    #[test]
    fn test_read_manifest_file_not_found() {
        let temp_zip = create_test_zip(None);

        let result = read_manifest_file_from_archive(temp_zip.path());

        assert!(result.is_err());
    }

    #[test]
    fn test_read_invalid_zip_file() {
        let temp_file = NamedTempFile::new().unwrap();
        File::create(temp_file.path())
            .unwrap()
            .write_all(b"not a zip file")
            .unwrap();

        let result = read_manifest_file_from_archive(temp_file.path());

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::Zip(ZipError::InvalidArchive(_))
        ));
    }

    #[test]
    fn test_read_nonexistent_file() {
        let nonexistent_path = Path::new("nonexistent.zip");

        let result = read_manifest_file_from_archive(nonexistent_path);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Io(_)));
    }
}
