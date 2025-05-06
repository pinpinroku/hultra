use serde::{Deserialize, Serialize};
use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
    time::Instant,
};
use tracing::debug;

use crate::{
    error::Error,
    fileutil::{hash_file, read_manifest_file_from_archive},
    mod_registry::{ModRegistryQuery, RemoteModRegistry},
};

/// Represents the `everest.yaml` manifest file that defines a mod.
#[derive(Debug, Deserialize, Serialize, Clone, Hash, PartialEq, Eq)]
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
#[derive(Debug, Deserialize)]
pub struct LocalMod {
    /// Path to the local mod file which contains the mod's assets and manifest
    pub file_path: PathBuf,
    /// Mod manifest resides in the mod file
    pub manifest: ModManifest,
    /// Computed XXH64 hash of the file for update check
    checksum: Option<String>,
}

pub trait Generatable {
    fn new(file_path: PathBuf, manifest: ModManifest) -> Self;
    fn file_path(&self) -> &Path;
    fn manifest(&self) -> &ModManifest;
    fn checksum(&mut self) -> Result<&str, Error>;
}

impl Generatable for LocalMod {
    /// Creates a new `LocalMod` instance.
    fn new(file_path: PathBuf, manifest: ModManifest) -> Self {
        Self {
            file_path,
            manifest,
            checksum: None,
        }
    }

    fn file_path(&self) -> &Path {
        &self.file_path
    }

    fn manifest(&self) -> &ModManifest {
        &self.manifest
    }

    /// Sets the checksum of the file by computing them if none.
    ///
    /// # Returns
    /// * `Ok(&str)` - Computed checksum as a string reference.
    /// * `Err(Error)` - If the file could not be read.
    fn checksum(&mut self) -> Result<&str, Error> {
        debug!("Checksum of the mod: {:#?}", self.checksum);

        if self.checksum.is_none() {
            let computed_hash = hash_file(&self.file_path)?;
            debug!("Computed hash of the mod: {:#?}", computed_hash);
            self.checksum = Some(computed_hash);
        }

        // unwrap is fine here
        Ok(self.checksum.as_deref().unwrap())
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

// HACK: Move these logic to `src/download/update.rs`
/// Update information about the mod
#[derive(Debug, Clone)]
pub struct AvailableUpdateInfo {
    /// The Mod name
    pub name: String,
    /// Current version (from LocalModInfo)
    pub current_version: String,
    /// Available version (from RemoteModInfo)
    pub available_version: String,
    /// Download URL of the Mod
    pub url: String,
    /// xxHashes of the file
    pub hashes: Vec<String>,
    /// Outdated file
    pub existing_path: PathBuf,
}

/// Checks for an available update for a single installed mod.
///
/// This function compares the local mod's checksum with the checksum provided by the remote mod registry.
/// If the checksums differ, it indicates that an update is available. If the remote registry does not contain information
/// for the mod, or the checksums match, no update is reported.
///
/// # Arguments
/// * `local_mod` - Information about the locally installed mod, including its current version and checksum.
/// * `mod_registry` - A reference to the mod registry that holds remote mod information.
///
/// # Returns
/// * `Ok(Some(AvailableUpdateInfo))` if an update is available, containing update details.
/// * `Ok(None)` if no update is available (either because the mod is up-to-date or remote info is missing).
/// * `Err(Error)` if there is an error computing the mod's checksum.
fn check_update(
    mut local_mod: impl Generatable,
    mod_registry: &RemoteModRegistry,
) -> Result<Option<AvailableUpdateInfo>, Error> {
    // Look up remote mod info
    let manifest = local_mod.manifest();
    let remote_mod = match mod_registry.get_mod_info_by_name(&manifest.name) {
        Some(info) => info,
        None => return Ok(None), // No remote info, skip update check.
    };

    // Compute checksum
    let computed_hash = local_mod.checksum()?;

    // Continue only if the hash doesn't match
    if remote_mod.has_matching_hash(computed_hash) {
        return Ok(None);
    }

    let remote_mod = remote_mod.clone();
    let manifest = local_mod.manifest();

    Ok(Some(AvailableUpdateInfo {
        name: manifest.name.to_string(),
        current_version: manifest.version.to_string(),
        available_version: remote_mod.version,
        url: remote_mod.download_url,
        hashes: remote_mod.checksums,
        existing_path: local_mod.file_path().to_path_buf(),
    }))
}

/// Check available updates for all installed mods.
///
/// # Arguments
/// * `installed_mods` - A list of information about installed mods.
/// * `mod_registry` - Registry containing remote mod information.
///
/// # Returns
/// * `Ok(Vec<AvailableUpdateInfo>)` - List of available updates for mods.
/// * `Err(Error)` - If there are issues fetching or computing update information.
pub fn check_updates(
    installed_mods: Vec<impl Generatable>,
    mod_registry: &RemoteModRegistry,
) -> Result<Vec<AvailableUpdateInfo>, Error> {
    // Use iterator combinators to process each mod gracefully.
    let updates = installed_mods
        .into_iter()
        .map(|local_mod| check_update(local_mod, mod_registry))
        .collect::<Result<Vec<Option<AvailableUpdateInfo>>, Error>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    Ok(updates)
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
    use std::io::Write;
    use std::path::Path;

    use super::*;
    use tempfile::tempdir;

    const VALID_MANIFEST_FILE: &str = "everest.yaml";

    fn generate_test_mod_manifest(name: &str, version: &str) -> ModManifest {
        ModManifest {
            name: name.to_string(),
            version: version.to_string(),
            dll: None,
            dependencies: None,
            optional_dependencies: None,
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
    fn test_parse_mod_manifest_from_yaml() {
        let yaml = r#"
        - Name: TestMod
          Version: "1.0.0"
        "#;

        let result = ModManifest::from_yaml(yaml.as_bytes());
        assert!(result.is_ok());
        let manifest = result.unwrap();

        assert_eq!(manifest.name, "TestMod");
        assert_eq!(manifest.version, "1.0.0");
    }

    #[test]
    fn test_list_installed_mods() {
        let temp_dir = tempdir().unwrap();
        let mods_dir = temp_dir.path();
        let manifest = generate_test_mod_manifest("TestMod", "1.0.0");
        let path = create_test_mod_archive(mods_dir, &manifest, VALID_MANIFEST_FILE);

        let archive_paths = vec![path];

        let result = load_local_mods(archive_paths);
        assert!(result.is_ok());

        let installed_mods = result.unwrap();
        assert_eq!(installed_mods.len(), 1);
        assert_eq!(installed_mods[0].manifest.name, "TestMod");
    }

    #[test]
    fn test_remove_blacklisted_mods() {
        let temp_dir = tempdir().unwrap();
        let mods_dir = temp_dir.path();

        let manifest = generate_test_mod_manifest("BlacklistedMod", "1.0.0");
        let archive_path = create_test_mod_archive(mods_dir, &manifest, VALID_MANIFEST_FILE);

        let mut installed_mods = vec![LocalMod::new(archive_path.to_path_buf(), manifest)];
        let blacklist: HashSet<PathBuf> = vec![archive_path].into_iter().collect();

        let result = remove_blacklisted_mods(&mut installed_mods, &blacklist);
        assert!(result.is_ok());
        assert!(installed_mods.is_empty());
    }
}
