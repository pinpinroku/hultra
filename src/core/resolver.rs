//! Handle resolving missing dependency of the mod.
use std::collections::HashSet;

use crate::{
    core::{ChecksumError, network::downloader::DownloadTask, registry::EverestUpdateYaml},
    dependency::DependencyGraph,
};

/// Returns a list of mods that actually need to be downloaded.
pub fn resolve_missing_mods(
    target_ids: &HashSet<u32>,
    registry: &EverestUpdateYaml,
    graph: &DependencyGraph, // NOTE こいつがこのモジュールの主体
    installed_names: &HashSet<String>,
) -> HashSet<String> {
    // 1. Retrieve mod names associated with the provided IDs
    let mod_names = registry.get_names_by_ids(target_ids);

    // 3. Check if all required mods are already installed
    if installed_names.is_superset(&mod_names) {
        return HashSet::new();
    }

    // 4. Traverse the dependency graph to list all required mods (BFS)
    graph.bfs_traversal(mod_names)
}

// TODO is this function should be here? might move this to downloader
pub fn create_download_tasks(
    required_names: HashSet<String>,
    installed_names: HashSet<String>,
    registry: EverestUpdateYaml,
) -> Result<Vec<DownloadTask>, ChecksumError> {
    // 5. Filter out mods that are already present locally
    let missing_names: HashSet<String> = required_names
        .into_iter()
        .filter(|name| !installed_names.contains(name))
        .collect();

    // 6. Extract detailed download information from the registry and return it
    registry.create_download_tasks(missing_names)
}
