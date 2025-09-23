//! Local mod management: parsing manifest files and loading local mods.
//!
//! This module provides functionality to load and manage locally installed mods
//! by reading their manifest files from ZIP archives.
use std::{
    io,
    path::{Path, PathBuf},
};

use once_cell::sync::OnceCell;
use thiserror::Error;

use crate::{
    fileutil,
    manifest::{ManifestParseError, ModManifest},
    zip::{self, ZipError},
};

/// Errors that can occur while loading local mods.
#[derive(Debug, Error)]
pub enum LoadModsError {
    /// I/O error occurred while accessing the mod file.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Failed to parse the manifest file.
    #[error(transparent)]
    Manifest(#[from] ManifestParseError),
    /// Failed to parse the ZIP file or manifest not found.
    #[error(transparent)]
    Zip(#[from] ZipError),
}

/// Information about a locally installed mod.
#[derive(Debug, Clone)]
pub struct LocalMod {
    /// Path to the local mod file which contains the mod's assets and manifest
    pub location: PathBuf,
    /// Mod manifest resides in the mod file
    pub manifest: ModManifest,
    /// Computed XXH64 hash of the file for update check
    checksum: OnceCell<String>,
}

impl LocalMod {
    /// Returns a value of this type from the given file path by extracting and parsing the manifest.
    ///
    /// # Errors
    ///
    /// - `ZipError::Parse`: Failed to parse ZIP file. Broken ZIP format.
    /// - `ZipError::NotFound`: The manifest file not found in given path.
    /// - `ManifestParseError::Parse`: Failed to parse YAML format.
    /// - `ManifestParseError::NoEntries`: The manifest file does not have any entries.
    pub fn from_path(mod_path: &Path) -> Result<Self, LoadModsError> {
        let manifest_bytes = zip::find_manifest(mod_path)?;
        let manifest = ModManifest::from_slice(&manifest_bytes)?;
        Ok(Self {
            location: mod_path.to_path_buf(),
            manifest,
            checksum: OnceCell::new(),
        })
    }

    /// Compute checksum if not already computed, then cache it.
    pub fn checksum(&self) -> io::Result<&str> {
        self.checksum
            .get_or_try_init(|| fileutil::hash_file(&self.location))
            .map(|hash| hash.as_str())
    }

    /// Loads all local mods from the provided archive paths.
    ///
    /// # Notes
    ///
    /// Sometimes, `everest.yaml` file may not be present in the mod archive.
    /// In such cases, the function will log a warning and skip that archive.
    ///
    /// # Errors
    ///
    /// This function does not return errors directly. Instead, it logs errors when the manifest file could not be parsed or invalid.
    ///
    /// It's because we cannot do anything if some of the mod archives are broken but that's are not critical to stop the whole process.
    pub fn load_local_mods(archive_paths: &[PathBuf]) -> Vec<LocalMod> {
        use rayon::prelude::*;

        tracing::info!("Found {} mod archives to load", archive_paths.len());
        tracing::info!("Start parsing archive files.");
        let local_mods: Vec<LocalMod> = archive_paths
            .par_iter()
            .filter_map(|archive_path| match LocalMod::from_path(archive_path) {
                Ok(local_mod) => Some(local_mod),
                Err(e) if matches!(e, LoadModsError::Zip(ZipError::NotFound)) => {
                    tracing::warn!("{:?}: {}", archive_path.file_name(), e);
                    None
                }
                Err(e) => {
                    tracing::error!("Failed to load mod from {}: {}", archive_path.display(), e);
                    None
                }
            })
            .collect();
        tracing::info!("Successfully loaded {} local mods", local_mods.len());

        local_mods
    }
}

#[cfg(test)]
mod tests_local_mod {
    use super::*;

    #[test]
    fn test_checksum_computation() -> anyhow::Result<()> {
        let mod_path = Path::new("./test/test-mod.zip");
        let local_mod = LocalMod::from_path(mod_path).unwrap();
        let checksum = local_mod.checksum()?;
        assert!(!checksum.is_empty());
        Ok(())
    }

    #[test]
    fn test_from_path_valid_file() -> anyhow::Result<()> {
        let valid_path = Path::new("./test/test-mod.zip");
        let result = LocalMod::from_path(valid_path);
        assert!(result.is_ok());

        let local_mod = result?;
        assert_eq!(local_mod.location, valid_path);
        assert_eq!(local_mod.manifest.name, "test-mod");
        Ok(())
    }

    #[test]
    fn test_from_path_invalid_file() {
        let invalid_path = Path::new("invalid_mod.zip");
        let result = LocalMod::from_path(invalid_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_local_mods() {
        let archive_paths = vec![PathBuf::from("./test/test-mod.zip")];
        let local_mods = LocalMod::load_local_mods(&archive_paths);
        assert!(!local_mods.is_empty());
        assert_eq!(local_mods[0].manifest.name, "test-mod");
    }
}
