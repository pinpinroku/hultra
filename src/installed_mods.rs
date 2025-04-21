use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};
use tracing::{info, warn};

use crate::{
    error::Error,
    fileutil::hash_file,
    fileutil::{find_installed_mod_archives, read_manifest_file_from_zip},
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
        manifest_entries
            .pop_front()
            .ok_or_else(|| Error::NoEntriesInModManifest(manifest_entries))
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
    pub fn new(archive_path: PathBuf, manifest: ModManifest) -> Self {
        Self {
            archive_path,
            manifest,
            checksum: None,
        }
    }

    pub fn checksum(&mut self) -> Result<&str, Error> {
        if self.checksum.is_none() {
            let computed_hash = hash_file(&self.archive_path)?;
            self.checksum = Some(computed_hash);
        }
        // unwrap is fine here
        Ok(self.checksum.as_deref().unwrap())
    }
}

/// List installed mods which has valid manifest file
pub fn list_installed_mods(mods_dir: &Path) -> Result<InstalledModList, Error> {
    let archive_paths = find_installed_mod_archives(mods_dir)?;
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
                    .expect("File name shoud be exist");
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

/// Check available updates for all installed mods
pub fn check_updates(
    mods_dir: &Path,
    mod_registry: &ModRegistry,
) -> Result<Vec<AvailableUpdateInfo>, Error> {
    let installed_mods = list_installed_mods(mods_dir)?;
    // update_mod_hashes(&mut installed_mods);

    let mut available_updates = Vec::new();
    for mut local_mod in installed_mods {
        if let Some(remote_mod) = mod_registry.get_mod_info(&local_mod.manifest.name) {
            // Compute hash on demand
            if let Ok(computed_hash) = local_mod.checksum() {
                if remote_mod.has_matching_hash(computed_hash) {
                    continue; // No update avilable
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
