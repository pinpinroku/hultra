//! Handle resolving missing dependency of the mod.
use std::collections::{HashMap, HashSet};

use crate::{
    dependency::DependencyGraph,
    local_mods::LocalMod,
    registry::{ModRegistry, RemoteMod},
};

/// Resolves dependencies and returns a list of mods that actually need to be downloaded.
pub fn resolve_missing_mods(
    target_ids: &[u32],
    registry: ModRegistry,
    graph: &DependencyGraph,
    installed_mods: &[LocalMod],
) -> HashMap<String, RemoteMod> {
    // 1. Retrieve mod names associated with the provided IDs
    let mod_names = registry.get_names_by_ids(target_ids);

    // 2. Create a set of names for mods already installed locally
    let local_names: HashSet<&str> = installed_mods.iter().map(|m| m.name()).collect();

    // 3. Check if all required mods are already installed
    if local_names.is_superset(&mod_names) {
        return HashMap::new();
    }

    // 4. Traverse the dependency graph to list all required mods (BFS)
    let all_required_deps = graph.bfs_traversal(mod_names);

    // 5. Filter out mods that are already present locally
    let missing_names: HashSet<String> = all_required_deps
        .into_iter()
        .filter(|name| !local_names.contains(name.as_str()))
        .collect();

    // 6. Extract detailed download information from the registry and return it
    registry.extract_targets(&missing_names)
}
