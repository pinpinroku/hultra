//! Handle resolving missing dependency of the mod.
use std::collections::HashSet;

use crate::{core::registry::EverestUpdateYaml, dependency::DependencyGraph};

/// Returns a list of mods that actually need to be downloaded.
pub fn resolve_missing_mods(
    target_ids: &HashSet<u32>,
    registry: &EverestUpdateYaml,
    graph: &DependencyGraph, // TODO make this function method of this type, and move to dependency.rs
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
