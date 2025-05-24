use serde::Deserialize;
use std::collections::HashMap;
use tracing::debug;

use crate::{constant::MOD_REGISTRY_URL, error::Error, fetch};

/// Each entry in `everest_update.yaml` containing information about a mod.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct RemoteModInfo {
    /// Version string
    #[serde(rename = "Version")]
    pub version: String,
    /// Download link for the mod file
    #[serde(rename = "URL")]
    pub download_url: String,
    /// File size
    #[serde(rename = "Size")]
    pub file_size: u64,
    /// xxHash checksums for the file
    #[serde(rename = "xxHash")]
    pub checksums: Vec<String>,
    /// Reference ID of the GameBanana page
    #[serde(rename = "GameBananaId")]
    pub gamebanana_id: u32,
}

impl RemoteModInfo {
    /// Checks if the provided hash matches any of the expected checksums.
    pub fn has_matching_hash(&self, computed_hash: &str) -> bool {
        self.checksums
            .iter()
            .any(|checksum| checksum.eq_ignore_ascii_case(computed_hash))
    }
}

/// Represents the complete `everest_update.yaml` containing all available remote mods.
pub type RemoteModRegistry = HashMap<String, RemoteModInfo>;

pub trait ModRegistryQuery {
    fn get_mod_info_by_name(&self, name: &str) -> Option<&RemoteModInfo>;
    fn find_mod_entry_by_id(&self, mod_id: u32) -> Option<(&String, &RemoteModInfo)>;
}

impl ModRegistryQuery for RemoteModRegistry {
    /// Gets a mod registry entry that matches the given name.
    fn get_mod_info_by_name(&self, name: &str) -> Option<&RemoteModInfo> {
        debug!("Getting the mod information matching the name: {}", name);
        self.get(name)
    }

    /// Finds a mod registry that matches the mod ID.
    fn find_mod_entry_by_id(&self, mod_id: u32) -> Option<(&String, &RemoteModInfo)> {
        debug!(
            "Looking up the remote mod information that matches the mod ID: {}",
            mod_id
        );
        self.iter()
            .find(|(_, manifest)| manifest.gamebanana_id == mod_id)
    }
}

// Fetches the RemoteModRegistry
pub async fn fetch_remote_mod_registry() -> Result<RemoteModRegistry, Error> {
    fetch::fetch_remote_data::<RemoteModRegistry>(MOD_REGISTRY_URL, "online database").await
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::collections::HashMap;

    fn dummy_mod_info(gamebanana_id: u32, checksums: Vec<&str>) -> RemoteModInfo {
        RemoteModInfo {
            gamebanana_id,
            checksums: checksums.into_iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    fn dummy_registry() -> HashMap<String, RemoteModInfo> {
        let mut registry = HashMap::new();
        registry.insert(
            "SpeedrunTool".to_string(),
            dummy_mod_info(42, vec!["abcd1234", "efgh5678"]),
        );
        registry.insert("TASRecorder".to_string(), dummy_mod_info(99, vec![]));
        registry
    }

    #[test]
    fn test_has_matching_hash() {
        let mod_info = dummy_mod_info(0, vec!["abcd1234", "efgh5678"]);
        assert!(mod_info.has_matching_hash("abcd1234"));
        assert!(mod_info.has_matching_hash("efgh5678"));
        assert!(!mod_info.has_matching_hash("notfound"));
    }

    #[test]
    fn test_get_mod_info_by_name() {
        let mod_registry = dummy_registry();

        assert!(mod_registry.get_mod_info_by_name("SpeedrunTool").is_some());
        assert!(
            mod_registry
                .get_mod_info_by_name("NonExistentMod")
                .is_none()
        );
    }

    #[test]
    fn test_find_mod_registry_by_id() {
        let mod_registry = dummy_registry();

        let result = mod_registry.find_mod_entry_by_id(42);
        assert!(result.is_some());
        let (found_key, found_mod) = result.unwrap();
        assert_eq!(found_mod.gamebanana_id, 42);
        assert_eq!(found_key, "SpeedrunTool");

        assert!(mod_registry.find_mod_entry_by_id(12345).is_none());
    }
}
