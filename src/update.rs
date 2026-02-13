use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
};

use tracing::instrument;

use crate::{cache::CacheEntry, local_mods::LocalMod, registry::RemoteMod};

/// Identifies local mods that require updates by comparing file hashes with the remote registry.
///
/// Returns a map of `RemoteMod` information for any mods that have changed,
/// with update information.
///
/// ### Logic Flow:
/// 1. **Registry Lookup**: Verifies if the local mod exists in the remote registry.
/// 2. **Inode Resolution**: Attempts to retrieve the mod's inode.
/// 3. **Cache Query**: Checks the DB for a cached file hash associated with the inode.
/// 4. **Hash Comparison**:
///    - If cached: Compares the local hash against the remote registry's hash.
///    - If not cached: Treats as "no update" (unexpected state).
/// 5. **Update Detection**: Marks as an update if hashes do not match.
#[instrument(skip_all)]
pub fn detect(
    cache_db: BTreeMap<u64, CacheEntry>,
    mut registry: HashMap<String, RemoteMod>,
    local_mods: &[LocalMod],
) -> (HashMap<String, RemoteMod>, Vec<UpdateInfo>) {
    let mut available_mods = HashMap::with_capacity(local_mods.len());
    let mut available_info = Vec::with_capacity(local_mods.len());

    for local_mod in local_mods {
        let name = local_mod.name();

        // verify if the local mod exist in the remote registry
        let Some(remote_mod) = registry.get(name) else {
            continue;
        };

        // attempts to retrieve the mod's inode
        let Ok(inode) = local_mod.get_inode() else {
            continue;
        };

        // check if an update is required
        let is_update_needed = cache_db
            .get(&inode)
            .map(|entry| !remote_mod.checksums.contains(entry.hash()))
            .unwrap_or(false);

        // extract the metadata from the remote registry if an update is required
        if is_update_needed && let Some((name, remote_mod)) = registry.remove_entry(name) {
            let update_info = UpdateInfo {
                name: name.clone(),
                current_version: local_mod.version().to_string(),
                available_version: remote_mod.version().to_string(),
            };
            available_info.push(update_info);
            available_mods.insert(name, remote_mod);
        }
    }

    (available_mods, available_info)
}

#[derive(Debug)]
pub struct UpdateInfo {
    name: String,
    current_version: String,
    available_version: String,
}

impl Display for UpdateInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "* {}: {} -> {}",
            self.name, self.current_version, self.available_version
        )
    }
}
