use std::collections::HashMap;

use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use tracing::debug;

use crate::{constant::MOD_REGISTRY_URL, fetch};

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
    async fn fetch(client: &Client) -> Result<RemoteModRegistry>;
    fn get_mod_name_by_id(&self, mod_id: u32) -> Option<&String>;
}

impl ModRegistryQuery for RemoteModRegistry {
    /// Fetches the Remote Mod Registry from the maddie480's server.
    async fn fetch(client: &Client) -> Result<Self> {
        fetch::fetch_remote_data::<Self>(MOD_REGISTRY_URL, client).await
    }

    /// Gets a mod name that matches the given mod ID.
    fn get_mod_name_by_id(&self, mod_id: u32) -> Option<&String> {
        debug!(
            "Looking up the remote mod information that matches the mod ID: {}",
            mod_id
        );
        self.iter()
            .find(|(_, manifest)| manifest.gamebanana_id == mod_id)
            .map(|(mod_name, _)| mod_name)
    }
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
    fn test_find_mod_registry_by_id() {
        let mod_registry = dummy_registry();

        let result = mod_registry.get_mod_name_by_id(42);
        assert!(result.is_some());
        let found_key = result.unwrap();
        assert_eq!(found_key, "SpeedrunTool");

        assert!(mod_registry.get_mod_name_by_id(12345).is_none());
    }
}
