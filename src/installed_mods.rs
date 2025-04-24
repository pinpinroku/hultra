use serde::{Deserialize, Serialize};
use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
};
use tracing::{info, warn};

use crate::{
    error::Error,
    fileutil::{
        find_installed_mod_archives, hash_file, read_manifest_file_from_zip, read_updater_blacklist,
    },
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

impl LocalModInfo {
    /// Creates a new `LocalModInfo` instance.
    ///
    /// # Arguments
    /// * `archive_path` - Path to the mod's zip archive.
    /// * `manifest` - Parsed manifest information for the mod.
    pub fn new(archive_path: PathBuf, manifest: ModManifest) -> Self {
        Self {
            archive_path,
            manifest,
            checksum: None,
        }
    }

    /// Computes and retrieves the checksum of the mod archive.
    ///
    /// # Returns
    /// * `Ok(&str)` - Computed checksum as a string reference.
    /// * `Err(Error)` - If the file cannot be hashed.
    pub fn checksum(&mut self) -> Result<&str, Error> {
        if self.checksum.is_none() {
            let computed_hash = hash_file(&self.archive_path)?;
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

/// Check available updates for all installed mods.
///
/// # Arguments
/// * `mods_directory` - Path to the directory containing installed mods.
/// * `mod_registry` - Registry containing remote mod information.
///
/// # Returns
/// * `Ok(Vec<AvailableUpdateInfo>)` - List of available updates for mods.
/// * `Err(Error)` - If there are issues fetching or computing update information.
pub fn check_updates(
    mods_directory: &Path,
    mod_registry: &ModRegistry,
) -> Result<Vec<AvailableUpdateInfo>, Error> {
    // TODO: take archive_paths and filtered_intalled_mods as arguments
    let archive_paths = find_installed_mod_archives(mods_directory)?;
    let mut installed_mods = list_installed_mods(archive_paths)?;

    let blacklist = read_updater_blacklist(mods_directory)?;
    remove_blacklisted_mods(&mut installed_mods, &blacklist)?;

    let mut available_updates = Vec::new();
    for mut local_mod in installed_mods {
        if let Some(remote_mod) = mod_registry.get_mod_info(&local_mod.manifest.name) {
            // Compute hash on demand
            if let Ok(computed_hash) = local_mod.checksum() {
                if remote_mod.has_matching_hash(computed_hash) {
                    continue; // No update available
                };
                let available_mod = remote_mod.clone();
                available_updates.push(AvailableUpdateInfo {
                    name: local_mod.manifest.name,
                    current_version: local_mod.manifest.version,
                    available_version: available_mod.version,
                    url: available_mod.download_url,
                    hash: available_mod.checksums,
                    existing_path: local_mod.archive_path,
                });
            } else {
                return Err(Error::FileIsNotHashed);
            }
        }
    }

    Ok(available_updates)
}

/// Removes mods whose archive paths match entries in the updater blacklist from the provided vector.
///
/// # Arguments
/// * `installed_mods` - A mutable reference to a vector of installed mods
/// * `blacklist` - A reference to the `HashSet` which stored full path of the blacklisted files
///
/// # Returns
/// * `Result<(), Error>` - Result indicating success or error during blacklist processing
fn remove_blacklisted_mods(
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
mod tests {
    use std::io::Write;

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
