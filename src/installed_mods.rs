use serde::{Deserialize, Serialize};
use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
};
use tracing::{debug, info, warn};

use crate::{
    error::Error,
    fileutil::{hash_file, read_manifest_file_from_zip},
    mod_registry::ModRegistry,
};

/// Represents the `everest.yaml` manifest file that defines a mod
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModManifest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "DLL")]
    pub dll: Option<String>,
    #[serde(rename = "Dependencies")]
    pub dependencies: Option<Vec<Dependency>>,
    #[serde(rename = "OptionalDependencies")]
    pub optional_dependencies: Option<Vec<Dependency>>,
}

/// Dependency specification for required or optional mod dependencies
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Dependency {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Version")]
    pub version: Option<String>,
}

impl ModManifest {
    /// Parses the mod manifest YAML buffer into a structured `ModManifest` object.
    pub fn parse_mod_manifest_from_yaml(yaml_buffer: &[u8]) -> Result<Self, Error> {
        let mut manifest_entries = serde_yaml_ng::from_slice::<VecDeque<ModManifest>>(yaml_buffer)?;

        // Attempt to retrieve the first entry without unnecessary cloning.
        Ok(manifest_entries.pop_front().unwrap())
    }
}

/// Collection of all installed mods and their metadata
pub type InstalledModList = Vec<LocalModInfo>;

/// Information about a locally installed mod
#[derive(Debug, Deserialize, Serialize)]
pub struct LocalModInfo {
    /// Path to the zip file which contains the mod's assets and manifest
    #[serde(rename = "Filename")]
    pub archive_path: PathBuf,
    /// Mod manifest
    pub manifest: ModManifest,
    /// Computed XXH64 hash of the mod archive for update verification
    #[serde(rename = "xxHash")]
    checksum: Option<String>,
}

pub trait GenerateLocalDatabase {
    fn new(archive_path: PathBuf, manifest: ModManifest) -> Self;
    fn archive_path(&self) -> PathBuf;
    fn manifest(&self) -> ModManifest;
    fn checksum(&mut self) -> Result<&str, Error>;
}

impl GenerateLocalDatabase for LocalModInfo {
    /// Creates a new `LocalModInfo` instance.
    ///
    /// # Arguments
    /// * `archive_path` - Path to the mod's zip archive.
    /// * `manifest` - Parsed manifest information for the mod.
    fn new(archive_path: PathBuf, manifest: ModManifest) -> Self {
        Self {
            archive_path,
            manifest,
            checksum: None,
        }
    }

    fn archive_path(&self) -> PathBuf {
        self.archive_path.clone()
    }

    fn manifest(&self) -> ModManifest {
        self.manifest.clone()
    }

    /// Computes and retrieves the checksum of the mod archive.
    ///
    /// # Returns
    /// * `Ok(&str)` - Computed checksum as a string reference.
    /// * `Err(Error)` - If the file could not be read.
    fn checksum(&mut self) -> Result<&str, Error> {
        debug!("Checksum of the mod: {:#?}", self.checksum);

        if self.checksum.is_none() {
            let computed_hash = hash_file(&self.archive_path)?;
            debug!("Computed hash of the mod: {:#?}", computed_hash);
            self.checksum = Some(computed_hash);
        }

        // unwrap is fine here
        Ok(self.checksum.as_deref().unwrap())
    }
}

/// List installed mods with valid manifest files.
///
/// # Arguments
/// * `archive_paths` - A list of all installed mod archive paths.
///
/// # Returns
/// * `Ok(InstalledModList)` - List of installed mods with valid manifests.
/// * `Err(Error)` - If there are issues reading the files or parsing the manifests.
pub fn list_installed_mods(archive_paths: Vec<PathBuf>) -> Result<InstalledModList, Error> {
    let mut installed_mods = Vec::with_capacity(archive_paths.len());

    for archive_path in archive_paths {
        let manifest_content = read_manifest_file_from_zip(&archive_path)?;
        match manifest_content {
            Some(buffer) => {
                let manifest = ModManifest::parse_mod_manifest_from_yaml(&buffer)?;
                let mod_info = LocalModInfo::new(archive_path, manifest);
                installed_mods.push(mod_info);
            }
            None => {
                let debug_path = archive_path
                    .file_name()
                    .and_then(|path| path.to_str())
                    .expect("File name should be exist");
                warn!(
                    "No mod manifest file (everest.yaml) found in {}.\n\
                \t# The file might be named 'everest.yml' or located in a subdirectory.\n\
                \t# Please contact the mod creator about this issue or just ignore this message.\n\
                \t# Updates will be skipped for this mod.",
                    debug_path
                )
            }
        }
    }
    // Sort by name
    info!("Sorting results by name...");
    installed_mods.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));

    Ok(installed_mods)
}

/// Update information about the mod
#[derive(Debug)]
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
    pub hash: Vec<String>,
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
    mut local_mod: impl GenerateLocalDatabase,
    mod_registry: &ModRegistry,
) -> Result<Option<AvailableUpdateInfo>, Error> {
    // Look up remote mod info
    let manifest = local_mod.manifest();
    let remote_mod = match mod_registry.get_mod_info(&manifest.name) {
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
        name: manifest.name,
        current_version: manifest.version,
        available_version: remote_mod.version,
        url: remote_mod.download_url,
        hash: remote_mod.checksums,
        existing_path: local_mod.archive_path(),
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
    installed_mods: Vec<impl GenerateLocalDatabase>,
    mod_registry: &ModRegistry,
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
    installed_mods: &mut Vec<LocalModInfo>,
    blacklist: &HashSet<PathBuf>,
) -> Result<(), Error> {
    if blacklist.is_empty() {
        return Ok(());
    }

    // Remove mods whose archive_path matches any blacklisted path
    installed_mods.retain(|mod_info| !blacklist.contains(&mod_info.archive_path));

    Ok(())
}

#[cfg(test)]
mod tests_for_files {
    use std::io::Write;
    use std::path::Path;

    use super::*;
    use tempfile::tempdir;

    const VALID_MANIFEST_FILE: &str = "everest.yaml";
    const INVALID_MANIFEST_FILE: &str = "everest.yml";

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

        let result = ModManifest::parse_mod_manifest_from_yaml(yaml.as_bytes());
        assert!(result.is_ok());
        let manifest = result.unwrap();

        assert_eq!(manifest.name, "TestMod");
        assert_eq!(manifest.version, "1.0.0");
    }

    #[test]
    fn test_list_installed_mods_with_valid_manifest() {
        let temp_dir = tempdir().unwrap();
        let mods_dir = temp_dir.path();
        let manifest = generate_test_mod_manifest("TestMod", "1.0.0");
        let path = create_test_mod_archive(mods_dir, &manifest, VALID_MANIFEST_FILE);

        let archive_paths = vec![path];

        let result = list_installed_mods(archive_paths);
        assert!(result.is_ok());

        let installed_mods = result.unwrap();
        assert_eq!(installed_mods.len(), 1);
        assert_eq!(installed_mods[0].manifest.name, "TestMod");
    }

    #[test]
    fn test_list_installed_mods_with_invalid_manifest() {
        let temp_dir = tempdir().unwrap();
        let mods_dir = temp_dir.path();
        let manifest = generate_test_mod_manifest("TestMod", "1.0.0");
        let path = create_test_mod_archive(mods_dir, &manifest, INVALID_MANIFEST_FILE);

        let archive_paths = vec![path];

        let result = list_installed_mods(archive_paths);
        assert!(result.is_ok());

        let installed_mods = result.unwrap();
        assert_eq!(installed_mods.len(), 0);
    }

    #[test]
    fn test_remove_blacklisted_mods() {
        let temp_dir = tempdir().unwrap();
        let mods_dir = temp_dir.path();

        let manifest = generate_test_mod_manifest("BlacklistedMod", "1.0.0");
        let archive_path = create_test_mod_archive(mods_dir, &manifest, VALID_MANIFEST_FILE);

        let mut installed_mods = vec![LocalModInfo::new(archive_path.clone(), manifest)];
        let blacklist: HashSet<PathBuf> = vec![archive_path].into_iter().collect();

        let result = remove_blacklisted_mods(&mut installed_mods, &blacklist);
        assert!(result.is_ok());
        assert!(installed_mods.is_empty());
    }
}

#[cfg(test)]
mod tests_for_updates {
    use crate::mod_registry::RemoteModInfo;

    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    // Fake LocalModInfo for tests
    struct FakeLocalModInfo {
        archive_path: PathBuf,
        manifest: ModManifest,
        checksum: Option<String>,
    }

    impl GenerateLocalDatabase for FakeLocalModInfo {
        fn new(archive_path: PathBuf, manifest: ModManifest) -> Self {
            Self {
                archive_path,
                manifest,
                checksum: None,
            }
        }

        fn archive_path(&self) -> PathBuf {
            self.archive_path.clone()
        }

        fn manifest(&self) -> ModManifest {
            self.manifest.clone()
        }

        // dummy-hash method for tests
        fn checksum(&mut self) -> Result<&str, Error> {
            if self.checksum.is_none() {
                self.checksum = Some("dummy-hash".to_string());
            }
            Ok(self.checksum.as_deref().unwrap())
        }
    }

    // Helper function to create a FakeLocalModInfo.
    fn create_local_mod(
        name: &str,
        version: &str,
        archive: &str,
        precomputed_hash: Option<&str>,
    ) -> FakeLocalModInfo {
        let manifest = ModManifest {
            name: name.to_string(),
            version: version.to_string(),
            dll: None,
            dependencies: None,
            optional_dependencies: None,
        };
        let mut local_mod = FakeLocalModInfo::new(PathBuf::from(archive), manifest);
        if let Some(hash) = precomputed_hash {
            local_mod.checksum = Some(hash.to_string());
        }
        local_mod
    }

    // Helper function to create a dummy RemoteModInfo.
    fn create_remote_mod(
        name: &str,
        version: &str,
        download_url: &str,
        checksums: Vec<&str>,
    ) -> RemoteModInfo {
        RemoteModInfo {
            name: name.to_string(),
            version: version.to_string(),
            file_size: 12345,
            updated_at: 0,
            download_url: download_url.to_string(),
            checksums: checksums.into_iter().map(|s| s.to_string()).collect(),
            gamebanana_type: "dummy".to_string(),
            gamebanana_id: 1,
        }
    }

    // Helper function to create a dummy ModRegistry.
    fn create_mod_registry(entries: Vec<RemoteModInfo>) -> ModRegistry {
        let mut registry_map = HashMap::new();
        for remote_mod in entries {
            let name = remote_mod.name.clone();
            registry_map.insert(name, remote_mod);
        }
        ModRegistry {
            entries: registry_map,
        }
    }

    #[test]
    fn test_check_update_no_update_if_checksum_matches() {
        // Create a local mod whose checksum matches the remote mod registry.
        let local_mod_name = "TestMod";
        let local_mod_version = "1.0.0";
        let archive_path = "path/to/testmod.zip";
        let dummy_hash = "dummy-hash";

        // Create local mod with a checksum that matches the expected one.
        let local_mod = create_local_mod(
            local_mod_name,
            local_mod_version,
            archive_path,
            Some(dummy_hash),
        );

        // Create remote mod info with matching hash.
        let remote_mod = create_remote_mod(
            local_mod_name,
            "1.0.0",
            "http://example.com/testmod.zip",
            vec![dummy_hash],
        );
        let registry = create_mod_registry(vec![remote_mod]);

        // When the hashes match, check_update should return Ok(None)
        let result = check_update(local_mod, &registry).expect("check_update failed");
        assert!(
            result.is_none(),
            "Expected no update info when checksums match"
        );
    }

    #[test]
    fn test_check_update_with_update_available() {
        // Create a local mod whose checksum does not match the remote mod registry.
        let local_mod_name = "TestMod";
        let local_mod_version = "1.0.0";
        let archive_path = "path/to/testmod.zip";
        let dummy_hash = "dummy-hash";

        // Create local mod with a checksum that doesn't match the remote mod's expected hash.
        let local_mod = create_local_mod(
            local_mod_name,
            local_mod_version,
            archive_path,
            Some(dummy_hash),
        );

        // Create remote mod info with a different hash indicating an update.
        let updated_hash = "updated-dummy-hash";
        let remote_mod = create_remote_mod(
            local_mod_name,
            "1.1.0",
            "http://example.com/testmod_v1.1.zip",
            vec![updated_hash],
        );
        let registry = create_mod_registry(vec![remote_mod.clone()]);

        // When the hashes do not match, check_update should return Some(AvailableUpdateInfo)
        let update_info = check_update(local_mod, &registry).expect("check_update failed");
        assert!(
            update_info.is_some(),
            "Expected update info when checksums do not match"
        );
        let info = update_info.unwrap();
        assert_eq!(info.name, local_mod_name);
        assert_eq!(info.current_version, local_mod_version);
        assert_eq!(info.available_version, remote_mod.version);
        assert_eq!(info.url, remote_mod.download_url);
        assert_eq!(info.hash, remote_mod.checksums);
    }

    #[test]
    fn test_check_updates_with_mixed_mods() {
        // Create two local mods; one up-to-date and one outdated.
        let mod_name1 = "TestMod1";
        let mod_name2 = "TestMod2";
        let version1 = "1.0.0";
        let version2 = "2.0.0";
        let archive_path1 = "path/to/testmod1.zip";
        let archive_path2 = "path/to/testmod2.zip";

        // Both local mods will have dummy-hash from our dummy_hash_file.
        let dummy_hash = "dummy-hash";
        let local_mod1 = create_local_mod(mod_name1, version1, archive_path1, Some(dummy_hash));
        let local_mod2 = create_local_mod(mod_name2, version2, archive_path2, Some(dummy_hash));

        // Create a remote registry where:
        // For TestMod1, the hash matches (no update).
        // For TestMod2, the hash is different (update available).
        let remote_mod1 = create_remote_mod(
            mod_name1,
            version1,
            "http://example.com/testmod1.zip",
            vec![dummy_hash],
        );
        let remote_mod2 = create_remote_mod(
            mod_name2,
            "2.1.0",
            "http://example.com/testmod2_v2.1.zip",
            vec!["updated-dummy-hash"],
        );
        let registry = create_mod_registry(vec![remote_mod1, remote_mod2]);

        // Collect installed mods.
        let installed_mods = vec![local_mod1, local_mod2];
        let updates = check_updates(installed_mods, &registry).expect("check_updates failed");

        // Only TestMod2 should have an update
        assert_eq!(updates.len(), 1, "Expected one update available");
        let update = &updates[0];
        assert_eq!(update.name, mod_name2);
        assert_eq!(update.current_version, version2);
        assert_eq!(update.available_version, "2.1.0");
    }
}
