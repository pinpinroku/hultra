//! Domain model of dependency graph to resolve missing dependency of mods.
use std::collections::{HashMap, HashSet, VecDeque};

use serde::Deserialize;
use tracing::{debug, instrument, warn};

use crate::core::registry::EverestUpdateYaml;

/// Represents `mod_dependency_graph.yaml`.
#[derive(Debug, Default, Deserialize)]
#[serde(transparent)]
pub struct DependencyGraph {
    /// Detail of nodes
    nodes: HashMap<String, DependencyNode>,
}

impl DependencyGraph {
    /// Resolves which mods need to be downloaded by checking the target IDs against
    /// the registry and filtering out already installed mods, including dependencies.
    ///
    /// This implementation assumes that if the target mods are already installed,
    /// all of their dependencies are also guaranteed to be installed.
    pub fn resolve_missing_mods(
        &self,
        target_ids: &HashSet<u32>,
        registry: &EverestUpdateYaml,
        installed_names: &HashSet<String>,
    ) -> HashSet<String> {
        // 1. Retrieve mod names associated with the provided IDs
        let target_names = registry.get_names_by_ids(target_ids);

        // 2. Check if all target mods are already installed.
        // If they are, we assume dependencies are already satisfied.
        if installed_names.is_superset(&target_names) {
            return HashSet::new();
        }

        // 3. Traverse the dependency graph to list all required mods (BFS)
        // This is only executed if at least one target or its dependency is missing.
        self.bfs_traversal(target_names)
    }

    /// Traverses the dependency graph using BFS from multiple starting mods.
    ///
    /// # Returns
    ///
    /// A `HashSet` containing all required mods, including:
    /// - The starting mods themselves
    /// - All direct and transitive dependencies
    #[instrument(skip(self))]
    fn bfs_traversal(&self, start_mods: HashSet<String>) -> HashSet<String> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Adds starting mods to queue
        for start_mod in start_mods {
            queue.push_back(start_mod);
        }

        while let Some(current) = queue.pop_front() {
            if !visited.insert(current.clone()) {
                continue; // Already visited
            }
            if let Some(node) = self.get_node_by_key(&current) {
                for dep in &node.dependencies {
                    if !matches!(dep.name(), "Celeste" | "Everest" | "EverestCore") {
                        queue.push_back(dep.name().to_string());
                    }
                }
            } else {
                warn!(?current, "not found in dep graph");
            }
        }

        debug!("found dependencies: {:?}", visited);

        visited
    }

    /// Gets the node information for a given mod name.
    fn get_node_by_key(&self, key: &str) -> Option<&DependencyNode> {
        self.nodes.get(key)
    }
}

/// Each entry of the `mod_dependency_graph.yaml`.
#[derive(Debug, Default, Deserialize)]
struct DependencyNode {
    /// List of dependencies.
    #[serde(rename = "Dependencies")]
    dependencies: Vec<Dependency>,
}

/// Dependency of the mod.
#[derive(Debug, Default, Deserialize)]
pub struct Dependency {
    #[serde(rename = "Name")]
    name: String,
}

impl Dependency {
    /// Returns the name of the dependency.
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests_graph {
    use super::*;

    #[test]
    fn test_bfs_traversal() {
        let yaml_data = r#"
DarkMatterJourney:
  Dependencies:
    - Name: "MoreLockBlocks"
      Version: "1.0.0"
MoreLockBlocks:
  Dependencies: []
darkmoonruins:
  Dependencies:
    - Name: "AvBdayHelper2021"
      Version: "1.0.0"
AvBdayHelper2021:
  Dependencies:
    - Name: "ExtendedVariantMode"
      Version: "1.0.0"
ExtendedVariantMode:
  Dependencies: []
"#;
        let graph: DependencyGraph = serde_yaml_ng::from_slice(yaml_data.as_bytes()).unwrap();
        let mut start_mods = HashSet::new();
        start_mods.insert("DarkMatterJourney".to_string());
        start_mods.insert("darkmoonruins".to_string());
        let all_required = graph.bfs_traversal(start_mods);

        let expected_mods: HashSet<String> = [
            "DarkMatterJourney",
            "MoreLockBlocks",
            "darkmoonruins",
            "AvBdayHelper2021",
            "ExtendedVariantMode",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        assert_eq!(all_required, expected_mods);
    }
}
