use std::collections::HashMap;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Each entry in `everest_update.yaml` containing information about a mod.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoteModInfo {
    /// Actual mod name (not filename)
    #[serde(skip)]
    pub name: String,
    /// Version string
    #[serde(rename = "Version")]
    pub version: String,
    /// File size in bytes
    #[serde(rename = "Size")]
    pub file_size: u64,
    /// Timestamp of the last update
    #[serde(rename = "LastUpdate")]
    pub updated_at: u64,
    /// Download link for the mod file
    #[serde(rename = "URL")]
    pub download_url: String,
    /// xxHash checksums for the file
    #[serde(rename = "xxHash")]
    pub checksums: Vec<String>,
    /// Category for the mod (e.g., GameBanana type)
    #[serde(rename = "GameBananaType")]
    pub gamebanana_type: String,
    /// Reference ID of the GameBanana page
    #[serde(rename = "GameBananaId")]
    pub gamebanana_id: u32,
}

impl RemoteModInfo {
    /// Checks if the provided hash matches any of the expected checksums.
    ///
    /// # Arguments
    /// * `computed_hash` - The hash to check against the mod's checksums.
    ///
    /// # Returns
    /// Returns `true` if the hash matches any of the checksums, otherwise `false`.
    pub fn has_matching_hash(&self, computed_hash: &str) -> bool {
        self.checksums
            .iter()
            .any(|checksum| checksum == computed_hash)
    }
}

/// Represents the complete `everest_update.yaml` containing all available remote mods.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModRegistry {
    /// A mapping of mod names to their metadata
    #[serde(flatten)]
    pub entries: HashMap<String, RemoteModInfo>,
}

impl ModRegistry {
    /// Initializes a `ModRegistry` instance from raw binary data.
    ///
    /// # Arguments
    /// * `data` - Raw binary data representing the mod registry.
    ///
    /// # Returns
    /// * `Ok(Self)` - Parsed mod registry.
    /// * `Err(serde_yaml_ng::Error)` - If parsing fails.
    pub async fn from(data: Bytes) -> Result<Self, serde_yaml_ng::Error> {
        info!("Parsing remote mod registry data");
        let mut mod_registry: Self = serde_yaml_ng::from_slice(&data)?;

        // Set the name field for each ModInfo.
        mod_registry
            .entries
            .iter_mut()
            .for_each(|(key, mod_info)| mod_info.name = key.clone());

        Ok(mod_registry)
    }

    /// Retrieves mod information by name.
    ///
    /// # Arguments
    /// * `name` - The name of the mod to retrieve.
    ///
    /// # Returns
    /// * `Some(&RemoteModInfo)` - If the mod is found.
    /// * `None` - If the mod is not found.
    pub fn get_mod_info(&self, name: &str) -> Option<&RemoteModInfo> {
        info!("Getting remote mod information for the mod: {}", name);
        self.entries.get(name)
    }
}
