use serde::{Deserialize, Serialize};
use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
    time::Instant,
};
use tokio::sync::OnceCell;
use tracing::debug;

use crate::{
    error::Error,
    fileutil::{self, hash_file, read_manifest_file_from_archive},
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

    /// Compute checksum if not already computed, then cache it.
    ///
    /// # Returns
    /// * `Ok(&str)` - Computed checksum as a string reference.
    /// * `Err(Error)` - If the file could not be read.
    async fn checksum(&self) -> Result<&str, Error> {
        self.checksum
            .get_or_try_init(async || {
                tracing::debug!(
                    "Computing checksum for {}",
                    fileutil::replace_home_dir_with_tilde(&self.file_path)
                );
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
/// * `archive_paths` - A reference to the list of all local mod paths.
///
/// # Returns
/// * `Ok(Vec<LocalMod>)` - List of local mods with valid manifests.
/// * `Err(Error)` - If there are issues reading the files or parsing the manifests.
pub fn load_local_mods(archive_paths: &[PathBuf]) -> Result<Vec<LocalMod>, Error> {
    debug!("Start parsing archive files.");
    let start = Instant::now();

    let mut local_mods = Vec::with_capacity(archive_paths.len());

    for archive_path in archive_paths {
        let buffer = read_manifest_file_from_archive(archive_path)?;
        let manifest = ModManifest::from_yaml(&buffer)?;
        let local_mod = LocalMod::new(archive_path.to_path_buf(), manifest);
        local_mods.push(local_mod);
    }
    let duration = start.elapsed();
    debug!("Scanning manifest files took: {:#?}", duration);

    debug!("Sorting the installed mods by name...");
    local_mods.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));

    Ok(local_mods)
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

        let result = load_local_mods(&archive_paths);
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

        let result = load_local_mods(&archive_paths);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_blacklisted_mods() {
        let temp_dir = tempdir().unwrap();
        let mods_dir = temp_dir.path();

        let manifest = generate_test_mod_manifest("BlacklistedMod", "1.0.0");
        let file_path = create_test_mod_archive(mods_dir, &manifest, MANIFEST_FILE_NAME);

        let mut local_mods = vec![LocalMod::new(file_path.to_path_buf(), manifest)];
        let blacklisted_paths: HashSet<PathBuf> = vec![file_path].into_iter().collect();

        remove_blacklisted_mods(&mut local_mods, &blacklisted_paths);
        assert!(local_mods.is_empty());
    }

    #[test]
    fn test_remove_blacklisted_mods_without_entries() {
        let temp_dir = tempdir().unwrap();
        let mods_dir = temp_dir.path();

        let manifest = generate_test_mod_manifest("BlacklistedMod", "1.0.0");
        let file_path = create_test_mod_archive(mods_dir, &manifest, MANIFEST_FILE_NAME);

        let mut local_mods = vec![LocalMod::new(file_path.to_path_buf(), manifest)];
        let empty_blacklisted_paths = HashSet::new();

        remove_blacklisted_mods(&mut local_mods, &empty_blacklisted_paths);
        assert_eq!(local_mods.len(), 1);
    }

    #[test]
    fn test_collect_installed_mod_names() {
        let temp_dir = tempdir().unwrap();
        let mods_dir = temp_dir.path();

        let manifest = generate_test_mod_manifest("Testmod", "1.0.0");
        let file_path = create_test_mod_archive(mods_dir, &manifest, MANIFEST_FILE_NAME);

        let test_local_mods = vec![LocalMod::new(file_path.to_path_buf(), manifest)];
        let result = collect_installed_mod_names(test_local_mods);
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains("Testmod"));
    }
}
