use serde::{Deserialize, Serialize};
use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
    time::Instant,
};
use tracing::{debug, warn};

use crate::{
    error::Error,
    fileutil::{hash_file, read_manifest_file_from_zip, replace_home_dir_with_tilde},
    mod_registry::{RemoteModRegistry, get_mod_info_by_name},
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
    fn archive_path(&self) -> &Path;
    fn manifest(&self) -> &ModManifest;
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

    fn archive_path(&self) -> &Path {
        &self.archive_path
    }

    fn manifest(&self) -> &ModManifest {
        &self.manifest
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
    debug!("Start parsing archive files.");
    let mut installed_mods = Vec::with_capacity(archive_paths.len());

    let start = Instant::now();
    for archive_path in archive_paths {
        debug!(
            "Reading the file '{}'",
            replace_home_dir_with_tilde(&archive_path)
        );
        let manifest_content = read_manifest_file_from_zip(&archive_path)?;
        match manifest_content {
            Some(buffer) => {
                debug!("Manifest file detected. Trying to parse them.");
                let manifest = ModManifest::parse_mod_manifest_from_yaml(&buffer)?;
                let mod_info = LocalModInfo::new(archive_path, manifest);
                installed_mods.push(mod_info);
            }
            None => {
                // HACK: Collect failed mods in another vector
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
    let duration = start.elapsed();
    debug!("Manifest file scanning takes: {:#?}", duration);

    // Sort by name
    debug!("Sorting the installed mods by name...");
    installed_mods.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));

    Ok(installed_mods)
}

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
    mod_registry: &RemoteModRegistry,
) -> Result<Option<AvailableUpdateInfo>, Error> {
    // Look up remote mod info
    let manifest = local_mod.manifest();
    let remote_mod = match get_mod_info_by_name(mod_registry, &manifest.name) {
        Some(info) => info,
        None => return Ok(None), // No remote info, skip update check.
    };

    // Compute checksum
    let computed_hash = local_mod.checksum()?;

    // Continue only if the hash doesn't match
    if remote_mod.1.has_matching_hash(computed_hash) {
        return Ok(None);
    }

    let remote_mod = remote_mod.1.clone();
    let manifest = local_mod.manifest();

    Ok(Some(AvailableUpdateInfo {
        name: manifest.name.to_string(),
        current_version: manifest.version.to_string(),
        available_version: remote_mod.version,
        url: remote_mod.download_url,
        hash: remote_mod.checksums,
        existing_path: local_mod.archive_path().to_path_buf(),
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
    fn test_list_installed_mods() {
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
