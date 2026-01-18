//! The registry of the mods, represents a database file: `everest_update.yaml`.
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Deserializer};
use tracing::instrument;

/// Represents `everest_update.yaml`.
#[derive(Debug, Default, Deserialize)]
pub struct ModRegistry {
    /// All mods in the registry mapped by their unique names.
    pub mods: HashMap<String, RemoteMod>,
    /// Inverted index for the mod lookup by ID
    id_to_names: HashMap<u32, Vec<String>>,
}

impl ModRegistry {
    /// Returns value of this type while generating inverted index for the mod lookup by ID.
    pub fn new(mods: HashMap<String, RemoteMod>) -> Self {
        let id_to_names = mods.iter().fold(
            HashMap::new(),
            |mut acc: HashMap<u32, Vec<String>>, (name, info)| {
                acc.entry(info.gamebanana_id)
                    .or_default()
                    .push(name.clone());
                acc
            },
        );
        ModRegistry { mods, id_to_names }
    }

    /// Parses `everest_update.yaml` from the provided byte slice.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, serde_yaml_ng::Error> {
        tracing::info!("parsing remote registry");
        let mods: HashMap<String, RemoteMod> = serde_yaml_ng::from_slice(bytes)
            .inspect_err(|err| tracing::error!(?err, "failed to parse 'everest_update.yaml'"))?;
        tracing::info!(found_entries = mods.len());
        Ok(ModRegistry::new(mods))
    }

    /// Returns the mod names for the given mod IDs, if any exist in the registry.
    #[instrument(skip(self))]
    pub fn get_names_by_ids(&self, mod_ids: &[u32]) -> HashSet<&str> {
        mod_ids
            .iter()
            .filter_map(|id| self.id_to_names.get(id)) // -> &Vec<String>
            .flatten() // -> &String
            .map(|s| s.as_str()) // -> &str
            .collect()
    }
}

/// Extracts mod records matching given names.
pub fn extract_target_mods(
    mut registry: HashMap<String, RemoteMod>,
    names: &HashSet<String>,
) -> HashMap<String, RemoteMod> {
    names
        .iter()
        .filter_map(|name| registry.remove_entry(name))
        .collect()
}

/// Each entry in `everest_update.yaml` containing information about a mod.
#[derive(Debug, Default, Deserialize, Clone)]
pub struct RemoteMod {
    /// Version string. This value may not follow any specific versioning scheme. Do not expect it to be SemVer.
    #[serde(rename = "Version")]
    version: String,
    /// Download link for the mod file.
    #[serde(rename = "URL")]
    pub(crate) download_url: String,
    /// File size of the mod file, a.k.a. `Content-Length`.
    #[serde(rename = "Size")]
    pub(crate) file_size: u64,
    /// XxHash checksums for the file. (e.g. "f437bf0515368130")
    #[serde(rename = "xxHash", deserialize_with = "hex_vec_to_u64_vec")]
    pub(crate) checksums: Vec<u64>,
    /// Reference ID of the GameBanana page.
    #[serde(rename = "GameBananaId")]
    gamebanana_id: u32,
}

/// Converts `Vec<String>` to `Vec<u64>`.
///
/// XxHash is written as string format. So we deserialize it into u64 for comparison.
fn hex_vec_to_u64_vec<'de, D>(deserializer: D) -> Result<Vec<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let s_vec: Vec<String> = Deserialize::deserialize(deserializer)?;

    s_vec
        .into_iter()
        .map(|s| u64::from_str_radix(&s, 16).map_err(serde::de::Error::custom))
        .collect()
}

impl RemoteMod {
    /// Returns the version string of the mod.
    pub fn version(&self) -> &str {
        &self.version
    }
}

#[cfg(test)]
mod tests_registry {
    use super::*;

    use std::fs;

    fn load_registry_from_yaml(path: &str) -> ModRegistry {
        let bytes = fs::read(path).expect("test file should be read");
        let mods = serde_yaml_ng::from_slice(&bytes).expect("YAML format should be parsed");
        ModRegistry::new(mods)
    }

    #[test]
    fn test_mod_registry_new_and_inverted_index() {
        let mut mods = HashMap::new();
        mods.insert(
            "ModA".to_string(),
            RemoteMod {
                gamebanana_id: 42,
                ..Default::default()
            },
        );
        mods.insert(
            "ModB".to_string(),
            RemoteMod {
                gamebanana_id: 42,
                ..Default::default()
            },
        );
        let registry = ModRegistry::new(mods);

        assert!(
            registry
                .id_to_names
                .get(&42)
                .is_some_and(|id| id.len() == 2)
        );
        assert!(
            registry
                .id_to_names
                .get(&42)
                .is_some_and(|v| v.contains(&"ModA".to_string()))
        );
        assert!(
            registry
                .id_to_names
                .get(&42)
                .is_some_and(|v| v.contains(&"ModB".to_string()))
        );
    }

    #[test]
    fn test_mod_registry_from_slice_and_mods() {
        let registry = load_registry_from_yaml("./tests/everest_update.yaml");
        let mods = registry.mods;
        assert!(!mods.is_empty());
        // Check that at least one mod has expected fields
        assert!(
            mods.iter()
                .next()
                .is_some_and(|(_name, mod_info)| !mod_info.version.is_empty()
                    && !mod_info.download_url.is_empty())
        );
    }

    #[test]
    fn test_get_mod_names_by_id() {
        let registry = load_registry_from_yaml("./tests/everest_update.yaml");
        // Pick a known ID from the test file
        let known_id = registry.id_to_names.keys().next().cloned().unwrap();
        let names_opt = registry.get_names_by_ids(&[known_id]);
        assert!(!names_opt.is_empty());
    }
}
