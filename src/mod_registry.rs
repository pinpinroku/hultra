use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::{constant::MOD_REGISTRY_URL, error::Error};

/// Each entry in `everest_update.yaml` containing information about a mod.
#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct RemoteModInfo {
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
            .any(|checksum| checksum.eq_ignore_ascii_case(computed_hash))
    }
}

/// Represents the complete `everest_update.yaml` containing all available remote mods.
pub type RemoteModRegistry = HashMap<String, RemoteModInfo>;

/// Fetching the remote mod registry, then parsed and deserialize into RemoteModRegistry type
pub async fn fetch_remote_mod_registry() -> Result<RemoteModRegistry, Error> {
    info!("🌐 Fetching online database...");
    let client = reqwest::ClientBuilder::new()
        .http2_prior_knowledge()
        .gzip(true)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let response = client
        .get(MOD_REGISTRY_URL)
        .send()
        .await?
        .error_for_status()?;
    debug!("Response headers: {:#?}", response.headers());
    let data = response.bytes().await?;

    debug!("Parsing remote mod registry data.");
    let mod_registry: RemoteModRegistry = serde_yaml_ng::from_slice(&data)?;

    Ok(mod_registry)
}

/// Gets the mod registry entry that corresponds to the provided name.
pub fn get_mod_info_by_name<'a>(
    mod_registry: &'a RemoteModRegistry,
    name: &str,
) -> Option<(&'a String, &'a RemoteModInfo)> {
    debug!("Get the mod information matching the name: {}", name);
    mod_registry.get_key_value(name)
}

/// Gets the mod registry entry that corresponds to the provided URL.
pub fn get_mod_info_by_url<'a>(
    mod_registry: &'a RemoteModRegistry,
    game_page_url: &str,
) -> Option<(&'a String, &'a RemoteModInfo)> {
    debug!(
        "Getting the remote mod information by mod page URL: {}",
        game_page_url
    );
    let id = game_page_url.split('/').next_back()?.parse::<u32>().ok()?;
    mod_registry
        .iter()
        .find(|(_, manifest)| manifest.gamebanana_id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Tests the get_mod_info_from_url function with a dummy registry.
    #[test]
    fn test_get_mod_info_from_url_valid() {
        // Create a dummy mod registry with two entries.
        let mut mod_registry = HashMap::new();
        let key1 = String::from("mod1");
        let mod_info1 = RemoteModInfo {
            version: "1.0".to_string(),
            file_size: 1024,
            updated_at: 1610000000,
            download_url: "https://example.com/mod1".to_string(),
            checksums: vec!["deadbeef".to_string()],
            gamebanana_type: "test".to_string(),
            gamebanana_id: 42,
        };
        let key2 = String::from("mod2");
        let mod_info2 = RemoteModInfo {
            version: "2.0".to_string(),
            file_size: 2048,
            updated_at: 1620000000,
            download_url: "https://example.com/mod2".to_string(),
            checksums: vec!["feedface".to_string()],
            gamebanana_type: "test".to_string(),
            gamebanana_id: 99,
        };

        mod_registry.insert(key1.clone(), mod_info1);
        mod_registry.insert(key2.clone(), mod_info2);

        // Test URL that should match gamebanana_id 42
        let test_url = "https://gamebanana.com/members/42";
        let result = get_mod_info_by_url(&mod_registry, test_url);
        assert!(result.is_some());
        let (found_key, found_mod) = result.unwrap();
        // The found mod should have gamebanana_id 42 and the key should be "mod1"
        assert_eq!(found_mod.gamebanana_id, 42);
        assert_eq!(found_key, &key1);

        // Test URL that does not match any entry
        let invalid_url = "https://gamebanana.com/members/12345";
        let result_invalid = get_mod_info_by_url(&mod_registry, invalid_url);
        assert!(result_invalid.is_none());
    }

    /// Tests the get_mod_info_from_url function with an invalid URL (cannot parse an id).
    #[test]
    fn test_get_mod_info_from_url_invalid_url() {
        let mod_registry: RemoteModRegistry = HashMap::new();
        let malformed_url = "not a valid url";
        let result = get_mod_info_by_url(&mod_registry, malformed_url);
        assert!(result.is_none());
    }

    /// Tests the has_matching_hash method for RemoteModInfo.
    #[test]
    fn test_has_matching_hash() {
        let mod_info = RemoteModInfo {
            version: "1.0".to_string(),
            file_size: 1024,
            updated_at: 1610000000,
            download_url: "https://example.com/mod".to_string(),
            checksums: vec!["abcd1234".to_string(), "efgh5678".to_string()],
            gamebanana_type: "test".to_string(),
            gamebanana_id: 10,
        };

        assert!(mod_info.has_matching_hash("abcd1234"));
        assert!(mod_info.has_matching_hash("efgh5678"));
        assert!(!mod_info.has_matching_hash("notfound"));
    }
}
