//! This module provides functionality to locate and extract the manifest file
//! (e.g., `everest.yaml`) from a ZIP archive. It handles reading the ZIP file,
//! searching for the manifest, and returning its contents as a byte vector.
use std::path::Path;

use zip_search::ZipSearcher;

/// Errors that can occur while finding the manifest file in a ZIP archive.
#[derive(Debug, thiserror::Error)]
pub enum ZipError {
    /// The manifest file does not have any mod entries.
    #[error(
        "the manifest file could not be found. It may be misspelled or have the extension `.yml`"
    )]
    NotFound,
    /// Failed to parse the ZIP file. Broken ZIP format.
    #[error(transparent)]
    Parse(#[from] zip_search::ZipSearchError),
}

/// Finds manifest file in the ZIP file and returns the bytes of its contents.
///
/// # Errors
///
/// - `ZipError::NotFound`: The manifest file not found in given path.
/// - `ZipError::Parse`: Could not parse ZIP archive. Broken or invalid.
pub(crate) fn find_manifest(file_path: &Path) -> Result<Vec<u8>, ZipError> {
    const MANIFEST_FILE_NAME: &str = "everest.yaml";

    let mut zip_searcher = ZipSearcher::new(file_path)?;

    match zip_searcher.find_file(MANIFEST_FILE_NAME) {
        Ok(Some(entry)) => {
            let mut buffer = zip_searcher.read_file(&entry)?;

            // Check for UTF-8 BOM and remove if present
            if buffer.starts_with(&[0xEF, 0xBB, 0xBF]) {
                buffer.drain(0..3);
            }

            Ok(buffer)
        }
        Ok(None) => Err(ZipError::NotFound),
        Err(err) => Err(ZipError::Parse(err)),
    }
}

// TODO: Implement functions which can find `Dialog/English.txt` file in the ZIP file.
//       We can modify the `find_manifest` function to accept a filename parameter.

#[cfg(test)]
mod tests_zip {
    use super::*;

    use std::path::Path;

    #[test]
    fn test_find_manifest_in_zip_valid() -> anyhow::Result<()> {
        let mod_path = Path::new("./test/test-mod.zip");
        let result = find_manifest(mod_path);
        assert!(result.is_ok());

        let manifest_bytes = result?;
        assert!(!manifest_bytes.is_empty());

        Ok(())
    }

    #[test]
    fn test_find_manifest_in_zip_invalid() {
        let mod_path = Path::new("./test/missing-manifest.zip");
        let result = find_manifest(mod_path);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .is_some_and(|e| matches!(e, ZipError::NotFound))
        );
    }
}
