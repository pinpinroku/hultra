use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{
    constant::MOD_DEPENDENCY_GRAPH,
    fetch,
    local::Dependency,
    mod_registry::{RemoteModInfo, RemoteModRegistry},
};

/// Each entry of the `mod_dependency_graph.yaml`.
#[derive(Debug, Default, Deserialize, Serialize, Clone, Hash, PartialEq, Eq)]
pub struct ModDependency {
    #[serde(rename = "OptionalDependencies")]
    optional_dependencies: Vec<Dependency>,
    #[serde(rename = "Dependencies")]
    dependencies: Vec<Dependency>,
    #[serde(rename = "URL")]
    url: String,
}

/// Represents `mod_dependency_graph.yaml` which is the dependency graph.
pub type DependencyGraph = HashMap<String, ModDependency>;

/// A trait for querying mod dependencies.
pub trait ModDependencyQuery {
    async fn fetch(client: &Client) -> Result<DependencyGraph>;
    fn get_mod_info_by_name(&self, name: &str) -> Option<&ModDependency>;
    fn collect_all_dependencies_bfs(&self, mod_name: &str) -> HashSet<String>;
    fn check_dependencies(
        &self,
        mod_name: &str,
        mod_registry: &RemoteModRegistry,
        installed_mod_names: &HashSet<String>,
    ) -> Vec<(String, RemoteModInfo)>;
}

impl ModDependencyQuery for DependencyGraph {
    /// Fetches the Dependency Graph from the maddie480's server.
    async fn fetch(client: &Client) -> Result<Self> {
        fetch::fetch_remote_data::<Self>(MOD_DEPENDENCY_GRAPH, client).await
    }

    /// Gets a mod registry entry that matches the given name.
    fn get_mod_info_by_name(&self, name: &str) -> Option<&ModDependency> {
        tracing::debug!(
            "Getting the dependency information matching the name: {}",
            name
        );
        self.get(name)
    }

    /// Collects all dependencies for a given mod name using iterative BFS.
    fn collect_all_dependencies_bfs(&self, mod_name: &str) -> HashSet<String> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(mod_name);

        while let Some(current_mod) = queue.pop_front() {
            if !visited.insert(current_mod.to_string()) {
                continue;
            }

            if let Some(mod_dep) = self.get_mod_info_by_name(current_mod) {
                for dep in &mod_dep.dependencies {
                    if !matches!(dep.name.as_str(), "Everest" | "EverestCore") {
                        queue.push_back(&dep.name);
                    }
                }
            } else {
                tracing::warn!(
                    "Could not find the mod matching '{}' in the online database",
                    current_mod
                );
            }
        }

        visited
    }

    /// Checks for missing dependencies of a mod.
    ///
    /// Returns a vector of tuples containing the missing dependency name and its remote information.
    fn check_dependencies(
        &self,
        mod_name: &str,
        mod_registry: &RemoteModRegistry,
        installed_mod_names: &HashSet<String>,
    ) -> Vec<(String, RemoteModInfo)> {
        // Collects required dependencies for the mod including the mod itself
        let dependencies = self.collect_all_dependencies_bfs(mod_name);

        // Filters out missing dependencies
        let missing_deps = dependencies
            .difference(installed_mod_names)
            .collect::<Vec<_>>();
        tracing::debug!("Missing dependencies are found: {:?}", missing_deps);

        missing_deps
            .iter()
            .filter_map(|name| {
                let name = (*name).clone();
                if let Some(remote_mod) = mod_registry.get(&name) {
                    tracing::info!(
                        "Dependency [{}] is available: {}",
                        name,
                        remote_mod.download_url
                    );
                    Some((name, remote_mod.to_owned()))
                } else {
                    tracing::warn!("Dependency [{}] is not available in the registry", name);
                    None
                }
            })
            .collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod tests_dependency {
    use super::*;
    use crate::local::Dependency;

    impl ModDependency {
        pub fn new(dependencies: Vec<Dependency>) -> Self {
            Self {
                dependencies,
                ..Default::default()
            }
        }
    }

    fn mock_dep(name: &str) -> Dependency {
        Dependency {
            name: name.to_string(),
            ..Default::default()
        }
    }

    fn sample_graph() -> DependencyGraph {
        let mut graph = DependencyGraph::new();

        // A depends on B and C
        graph.insert(
            "A".to_string(),
            ModDependency::new(vec![mock_dep("B"), mock_dep("C")]),
        );
        // B depends on D
        graph.insert("B".to_string(), ModDependency::new(vec![mock_dep("D")]));
        // C has no dependencies
        graph.insert("C".to_string(), ModDependency::new(vec![]));
        // D has no dependencies
        graph.insert("D".to_string(), ModDependency::new(vec![]));

        graph
    }

    #[test]
    fn test_collect_all_dependencies_bfs() {
        let graph = sample_graph();
        let deps = graph.collect_all_dependencies_bfs("A");
        let expected: std::collections::HashSet<_> =
            ["A", "B", "C", "D"].iter().map(|s| s.to_string()).collect();
        assert_eq!(deps, expected);
    }

    #[test]
    fn test_collect_all_dependencies_bfs_handles_cycles() {
        let mut graph = sample_graph();
        // Add a cycle: D depends on A
        if let Some(d) = graph.get_mut("D") {
            d.dependencies.push(mock_dep("A"));
        }
        let deps = graph.collect_all_dependencies_bfs("A");
        let expected: std::collections::HashSet<_> =
            ["A", "B", "C", "D"].iter().map(|s| s.to_string()).collect();
        assert_eq!(deps, expected); // Should not infinite loop
    }

    #[test]
    fn test_get_mod_info_by_name() {
        let graph = sample_graph();
        let info = graph.get_mod_info_by_name("A");
        assert!(info.is_some());
        assert!(graph.get_mod_info_by_name("nonexistent").is_none());
    }

    #[test]
    fn test_check_dependencies() {
        let graph = sample_graph();
        let mut mod_registry = RemoteModRegistry::new(); // Assume this is properly initialized
        for name in ["A", "B", "C", "D"] {
            mod_registry.insert(name.to_string(), RemoteModInfo::default());
        }
        let installed_mods: HashSet<String> = ["A", "B"].iter().map(|s| s.to_string()).collect();

        let missing_deps = graph.check_dependencies("A", &mod_registry, &installed_mods);
        assert_eq!(missing_deps.len(), 2); // C and D should be missing
        assert!(missing_deps.iter().any(|(name, _)| name == "C"));
        assert!(missing_deps.iter().any(|(name, _)| name == "D"));
    }
}
