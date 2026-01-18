//! Dependency graph to resolve dependency efficiently.
use std::collections::{HashMap, HashSet, VecDeque};

use serde::Deserialize;

/// Each entry of the `mod_dependency_graph.yaml`.
#[derive(Debug, Default, Deserialize)]
pub struct DependencyNode {
    /// A list of dependencies.
    #[serde(rename = "Dependencies")]
    dependencies: Vec<Dependency>,
}

/// Represents `mod_dependency_graph.yaml`.
#[derive(Debug, Default, Deserialize)]
pub struct DependencyGraph {
    /// Detail of nodes
    pub nodes: HashMap<String, DependencyNode>,
}

impl DependencyGraph {
    /// Creates a new instance of `DependencyGraph`.
    fn new(nodes: HashMap<String, DependencyNode>) -> Self {
        Self { nodes }
    }

    /// Parses YAML bytes to return a value of this type.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, serde_yaml_ng::Error> {
        tracing::info!("parsing dependency graph");
        let nodes = serde_yaml_ng::from_slice(bytes).inspect_err(|err| {
            tracing::error!(?err, "failed to parse 'mod_dependency_graph.yaml'")
        })?;
        Ok(Self::new(nodes))
    }

    /// Traverses the dependency graph using BFS from multiple starting mods.
    ///
    /// # Returns
    ///
    /// A `HashSet` containing all required mods, including:
    /// - The starting mods themselves
    /// - All direct and transitive dependencies
    pub fn bfs_traversal(&self, start_mods: HashSet<&str>) -> HashSet<String> {
        tracing::info!("starting to traverse dependency graph");
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Adds starting mods to queue
        for start_mod in start_mods {
            queue.push_back(start_mod.to_string());
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
                tracing::warn!(?current, "not found in dep graph");
            }
        }

        tracing::debug!("found dependencies: {:?}", visited);

        visited
    }

    /// Gets the node information for a given mod name.
    fn get_node_by_key(&self, key: &str) -> Option<&DependencyNode> {
        self.nodes.get(key)
    }
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
        let graph = DependencyGraph::from_slice(yaml_data.as_bytes()).unwrap();
        let mut start_mods = HashSet::new();
        start_mods.insert("DarkMatterJourney");
        start_mods.insert("darkmoonruins");
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
