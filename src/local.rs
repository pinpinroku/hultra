use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
};

use anyhow::Context;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use zip_search::ZipSearcher;

use crate::{
    error::Error,
    fileutil::{hash_file, replace_home_dir_with_tilde},
};

/// Represents the `everest.yaml` manifest file that defines a mod.
#[derive(Debug, Default, Deserialize, Serialize, Clone, Hash, PartialEq, Eq)]
pub struct ModManifest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "DLL")]
    dll: Option<String>,
    #[serde(rename = "Dependencies")]
    pub dependencies: Option<Vec<Dependency>>,
    #[serde(rename = "OptionalDependencies")]
    pub optional_dependencies: Option<Vec<Dependency>>,
}

/// Dependency specification for required or optional mod dependencies.
#[derive(Debug, Default, Deserialize, Serialize, Clone, Hash, PartialEq, Eq)]
pub struct Dependency {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Version")]
    pub version: Option<String>,
}

impl ModManifest {
    /// Parses a YAML buffer to return a value of this type.
    fn from_yaml(yaml_buffer: &[u8]) -> Result<Option<Self>, Error> {
        // NOTE: We always need first entry from this collection since that is the primal mod, so we use the `VecDeque<T>` here instead of the `Vec<T>`.
        let mut manifest_entries = serde_yaml_ng::from_slice::<VecDeque<Self>>(yaml_buffer)?;

        // Attempt to retrieve the first entry without unnecessary cloning or element shifting.
        let entry = manifest_entries.pop_front();
        Ok(entry)
    }
}

/// Information about a locally installed mod.
#[derive(Debug, Clone)]
pub struct LocalMod {
    /// Path to the local mod file which contains the mod's assets and manifest
    pub file_path: PathBuf,
    /// Mod manifest resides in the mod file
    pub manifest: ModManifest,
    /// Computed XXH64 hash of the file for update check
    checksum: OnceCell<String>,
}

pub trait Generatable {
    fn checksum(&self) -> anyhow::Result<&str>;
    fn from_path(file_path: &Path) -> anyhow::Result<LocalMod>;
    fn load_local_mods(archive_paths: &[PathBuf]) -> Vec<LocalMod>;
}

impl Generatable for LocalMod {
    /// Compute checksum if not already computed, then cache it.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read.
    fn checksum(&self) -> anyhow::Result<&str> {
        self.checksum
            .get_or_try_init(|| {
                let computed_hash = hash_file(&self.file_path)?;
                Ok(computed_hash)
            })
            .map(|hash| hash.as_str())
    }

    /// Creates a `LocalMod` instance from the given file path.
    ///
    /// # Errors
    /// Returns an error if the manifest file cannot be found or parsed.
    fn from_path(file_path: &Path) -> anyhow::Result<Self> {
        const MANIFEST: &str = "everest.yaml";

        let debug_filename = replace_home_dir_with_tilde(file_path);

        // Find a manifest file in zip
        let mut zip_searcher = ZipSearcher::new(file_path)?;
        match zip_searcher.find_file(MANIFEST)? {
            Some(entry) => {
                let mut buffer = zip_searcher.read_file(&entry)?;
                // Check for UTF-8 BOM and remove if present
                if buffer.starts_with(&[0xEF, 0xBB, 0xBF]) {
                    buffer.drain(0..3);
                }

                // Parses the file
                if let Some(manifest) = ModManifest::from_yaml(&buffer).with_context(|| {
                    format!(
                        "Failed to parse manifest file '{}' in '{}'",
                        MANIFEST, debug_filename
                    )
                })? {
                    Ok(Self {
                        file_path: file_path.to_path_buf(),
                        manifest,
                        checksum: OnceCell::new(),
                    })
                } else {
                    anyhow::bail!(
                        "Manifest file '{}' in '{}' is empty or invalid",
                        MANIFEST,
                        debug_filename
                    );
                }
            }
            None => anyhow::bail!("'{}' not found in '{}'", MANIFEST, debug_filename),
        }
    }

    /// Loads all local mods from the provided archive paths.
    fn load_local_mods(archive_paths: &[PathBuf]) -> Vec<Self> {
        use rayon::prelude::*;

        tracing::info!("Start parsing archive files.");
        let mut local_mods: Vec<Self> = archive_paths
            .par_iter()
            .filter_map(|archive_path| match Self::from_path(archive_path) {
                Ok(local_mod) => Some(local_mod),
                Err(e) => {
                    tracing::warn!(
                        "Failed to read manifest from '{}': {}",
                        replace_home_dir_with_tilde(archive_path),
                        e
                    );
                    None
                }
            })
            .collect();

        tracing::info!("Sorting the installed mods by name...");
        local_mods.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));

        local_mods
    }
}

/// Removes LocalMod whose file path matches any blacklisted path from the given vector.
///
/// If the given collection is empty, this function does nothing.
///
/// # Arguments
/// * `local_mods` - A mutable reference of the vector which stored LocalMods
/// * `blacklisted_paths` - A reference to the `HashSet` which stored **full path** of the blacklisted files
pub fn remove_blacklisted_mods(
    local_mods: &mut Vec<LocalMod>,
    blacklisted_paths: &HashSet<PathBuf>,
) {
    local_mods.retain(|local_mod| !blacklisted_paths.contains(&local_mod.file_path))
}

/// Collects and returns mod names which are already installed locally.
pub fn collect_installed_mod_names(local_mods: Vec<LocalMod>) -> Result<HashSet<String>, Error> {
    let installed_mod_names: HashSet<_> = local_mods
        .into_iter()
        .map(|installed| installed.manifest.name)
        .collect();
    tracing::debug!("Installed mod names: {:?}", installed_mod_names);
    Ok(installed_mod_names)
}

#[cfg(test)]
mod tests_for_files {

    use super::*;

    #[test]
    fn test_from_yaml_parse_valid_manifest() {
        let yaml = r#"
        - Name: TestMod
          Version: 1.0.0
        "#;

        let result = ModManifest::from_yaml(yaml.as_bytes());
        assert!(result.is_ok());
        let manifest = result.unwrap().unwrap();

        assert_eq!(manifest.name, "TestMod");
        assert_eq!(manifest.version, "1.0.0");
    }

    #[test]
    fn test_from_yaml_parse_invalid_manifest() {
        let yaml = r#"
        TestMod
          Version: 1.0.0
        "#;

        let result = ModManifest::from_yaml(yaml.as_bytes());
        assert!(result.is_err());
    }
}
