use serde::{Deserialize, Serialize};
use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
    time::Instant,
};
use tokio::sync::OnceCell;
use tracing::debug;

use crate::{
    error::Error,
    fileutil::{hash_file, read_manifest_file_from_archive},
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
#[derive(Debug, Deserialize, Serialize, Clone, Hash, PartialEq, Eq)]
pub struct Dependency {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Version")]
    pub version: String,
}

impl ModManifest {
    /// Parses a YAML buffer to return a value of this type.
    pub fn from_yaml(yaml_buffer: &[u8]) -> Result<Self, Error> {
        // NOTE: We always need first entry from this collection since that is the primal mod, so we use the `VecDeque<T>` here instead of the `Vec<T>`.
        let mut manifest_entries = serde_yaml_ng::from_slice::<VecDeque<Self>>(yaml_buffer)?;

        // Attempt to retrieve the first entry without unnecessary cloning or element shifting.
        let entry = manifest_entries
            .pop_front()
            .ok_or_else(|| Error::MissingManifestEntry(manifest_entries))?;
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
    fn new(file_path: PathBuf, manifest: ModManifest) -> Self;
    fn file_path(&self) -> &Path;
    fn manifest(&self) -> &ModManifest;
    async fn checksum(&self) -> Result<&str, Error>;
}

impl Generatable for LocalMod {
    /// Creates a new `LocalMod` instance.
    fn new(file_path: PathBuf, manifest: ModManifest) -> Self {
        Self {
            file_path,
            manifest,
            checksum: OnceCell::new(),
        }
    }

    fn file_path(&self) -> &Path {
        &self.file_path
    }

    fn manifest(&self) -> &ModManifest {
        &self.manifest
    }

    /// Compute checksum if not already computed, then cache it.
    ///
    /// # Returns
    /// * `Ok(&str)` - Computed checksum as a string reference.
    /// * `Err(Error)` - If the file could not be read.
    async fn checksum(&self) -> Result<&str, Error> {
        self.checksum
            .get_or_try_init(async || {
                tracing::debug!("Computing checksum for {}", self.file_path.display());
                let computed_hash = hash_file(&self.file_path).await?;
                Ok(computed_hash)
            })
            .await
            .map(|hash| hash.as_str())
    }
}

/// Load local mods with valid manifest files.
///
/// # Arguments
/// * `archive_paths` - A list of all local mod paths.
///
/// # Returns
/// * `Ok(Vec<LocalMod>)` - List of local mods with valid manifests.
/// * `Err(Error)` - If there are issues reading the files or parsing the manifests.
pub fn load_local_mods(archive_paths: Vec<PathBuf>) -> Result<Vec<LocalMod>, Error> {
    debug!("Start parsing archive files.");
    let start = Instant::now();

    let mut local_mods = Vec::with_capacity(archive_paths.len());

    for archive_path in archive_paths {
        let buffer = read_manifest_file_from_archive(&archive_path)?;
        let manifest = ModManifest::from_yaml(&buffer)?;
        let local_mod = LocalMod::new(archive_path, manifest);
        local_mods.push(local_mod);
    }
    let duration = start.elapsed();
    debug!("Scanning manifest files took: {:#?}", duration);

    debug!("Sorting the installed mods by name...");
    local_mods.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));

    Ok(local_mods)
}

/// Removes mods whose archive paths match entries in the updater blacklist from the provided vector.
///
/// # Arguments
/// * `installed_mods` - A mutable reference to a vector of installed mods
/// * `blacklist` - A reference to the `HashSet` which stored full path of the blacklisted files
///
/// # Returns
/// * `Result<(), Error>` - Result indicating success or error during blacklist processing
pub fn remove_blacklisted_mods(
    installed_mods: &mut Vec<LocalMod>,
    blacklist: &HashSet<PathBuf>,
) -> Result<(), Error> {
    if blacklist.is_empty() {
        return Ok(());
    }

    // Remove mods whose archive_path matches any blacklisted path
    installed_mods.retain(|mod_info| !blacklist.contains(&mod_info.file_path));

    Ok(())
}

#[cfg(test)]
mod tests_for_files {
    use std::{io::Write, path::Path};

    use super::*;
    use tempfile::{NamedTempFile, tempdir};

    const MANIFEST_FILE_NAME: &str = "everest.yaml";

    fn generate_test_mod_manifest(name: &str, version: &str) -> ModManifest {
        ModManifest {
            name: name.to_string(),
            version: version.to_string(),
            ..Default::default()
        }
    }

    fn create_test_mod_archive(
        mods_dir: &Path,
        manifest: &ModManifest,
        manifest_file_name: &str,
    ) -> PathBuf {
        let archive_path = mods_dir.join(format!("{}.zip", manifest.name));
        let file = std::fs::File::create(&archive_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);

        // Serialize the manifest to YAML as a sequence
        let manifest_yaml = serde_yaml_ng::to_string(&vec![manifest]).unwrap();

        zip.start_file(manifest_file_name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(manifest_yaml.as_bytes()).unwrap();
        zip.finish().unwrap();

        archive_path
    }

    #[test]
    fn test_from_yaml_parse_valid_manifest() {
        let yaml = r#"
        - Name: TestMod
          Version: 1.0.0
        "#;

        let result = ModManifest::from_yaml(yaml.as_bytes());
        assert!(result.is_ok());
        let manifest = result.unwrap();

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

    #[test]
    fn test_load_local_mods_with_manifest() {
        let temp_dir = tempdir().unwrap();
        let mods_dir = temp_dir.path();
        let manifest = generate_test_mod_manifest("TestMod", "1.0.0");
        let path = create_test_mod_archive(mods_dir, &manifest, MANIFEST_FILE_NAME);

        let archive_paths = vec![path];

        let result = load_local_mods(archive_paths);
        assert!(result.is_ok());

        let local_mods = result.unwrap();
        assert_eq!(local_mods.len(), 1);
        assert_eq!(local_mods[0].manifest.name, "TestMod");
    }

    #[test]
    fn test_load_local_mods_without_manifest() {
        let path = NamedTempFile::with_suffix(".zip")
            .unwrap()
            .path()
            .to_path_buf();

        let archive_paths = vec![path];

        let result = load_local_mods(archive_paths);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_blacklisted_mods() {
        let temp_dir = tempdir().unwrap();
        let mods_dir = temp_dir.path();

        let manifest = generate_test_mod_manifest("BlacklistedMod", "1.0.0");
        let archive_path = create_test_mod_archive(mods_dir, &manifest, MANIFEST_FILE_NAME);

        let mut installed_mods = vec![LocalMod::new(archive_path.to_path_buf(), manifest)];
        let blacklist: HashSet<PathBuf> = vec![archive_path].into_iter().collect();

        let result = remove_blacklisted_mods(&mut installed_mods, &blacklist);
        assert!(result.is_ok());
        assert!(installed_mods.is_empty());
    }
}
