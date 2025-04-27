use std::{collections::HashMap, path::Path};

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::download::Downloadable;

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

impl Downloadable for RemoteModInfo {
    fn name(&self) -> &str {
        &self.name
    }
    fn url(&self) -> &str {
        &self.download_url
    }
    fn checksums(&self) -> &[String] {
        &self.checksums
    }
    fn version(&self) -> &str {
        &self.version
    }
    fn existing_path(&self) -> Option<&Path> {
        None
    }
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

// HACK: Replace ModRegistry to this simple type
// type Entries = HashMap<String, RemoteModInfo>;

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
    pub fn get_mod_info_by_name(&self, name: &str) -> Option<&RemoteModInfo> {
        info!("Getting remote mod information for the mod: {}", name);
        self.entries.get(name)
    }

    /// Retrieves mod information by game page URL.
    ///
    /// # Arguments
    /// * `url` - The URL of the mod to retrieve.
    ///
    /// # Returns
    /// * `Some(&RemoteModInfo)` - If the mod is found.
    /// * `None` - If the mod is not found.
    pub fn get_mod_info_from_url(&self, url: &str) -> Option<&RemoteModInfo> {
        info!("Getting remote mod information for the URL: {}", url);
        let id = url
            .split("/")
            .last()
            .and_then(|id_str| id_str.parse::<u32>().ok());
        if let Some(id) = id {
            self.entries
                .values()
                .find(|manifest| manifest.gamebanana_id == id)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Generates a `RemoteModInfo` instance with default or specified values.
    pub fn generate_remote_mod_info(
        name: &str,
        version: &str,
        checksums: Vec<String>,
    ) -> RemoteModInfo {
        RemoteModInfo {
            name: name.to_string(),
            version: version.to_string(),
            file_size: 1024,
            updated_at: 1234567890,
            download_url: "https://gamebanana.com/mmdl/567812".to_string(),
            checksums,
            gamebanana_type: "Tool".to_string(),
            gamebanana_id: 123456,
        }
    }

    /// Generates a `ModRegistry` instance with default or specified mod entries.
    pub fn generate_mod_registry(entries: Vec<(&str, RemoteModInfo)>) -> ModRegistry {
        let mut registry_entries = HashMap::new();
        for (name, mod_info) in entries {
            registry_entries.insert(name.to_string(), mod_info);
        }
        ModRegistry {
            entries: registry_entries,
        }
    }

    #[test]
    fn test_remote_mod_info_has_matching_hash() {
        let mod_info = generate_remote_mod_info(
            "Test Mod",
            "1.0.0",
            vec![String::from("abcd1234"), String::from("efgh5678")],
        );

        assert!(mod_info.has_matching_hash("abcd1234"));
        assert!(!mod_info.has_matching_hash("xyz9876"));
    }

    #[test]
    fn test_mod_registry_get_mod_info_by_name() {
        let mod1 = generate_remote_mod_info("Mod1", "1.0.0", vec![String::from("hash1")]);
        let mod2 = generate_remote_mod_info("Mod2", "2.0.0", vec![String::from("hash2")]);
        let registry = generate_mod_registry(vec![("Mod1", mod1.clone()), ("Mod2", mod2.clone())]);

        let mod_info = registry.get_mod_info_by_name("Mod1");
        assert!(mod_info.is_some());
        assert_eq!(mod_info.unwrap().version, "1.0.0");

        let nonexistent_mod = registry.get_mod_info_by_name("Nonexistent");
        assert!(nonexistent_mod.is_none());
    }

    #[test]
    fn test_mod_registry_get_mod_info_from_url() {
        let mod1 = generate_remote_mod_info("Mod1", "1.0.0", vec![String::from("hash1")]);
        let registry = generate_mod_registry(vec![("Mod1", mod1.clone())]);

        let mod_info = registry.get_mod_info_from_url("https://gamebanan.com/mods/123456");
        assert!(mod_info.is_some());
        assert_eq!(mod_info.unwrap().gamebanana_id, 123456);
        assert_eq!(
            mod_info.unwrap().download_url,
            "https://gamebanana.com/mmdl/567812"
        );

        let not_url = registry.get_mod_info_from_url("Mod 1");
        assert!(not_url.is_none());
    }
}
