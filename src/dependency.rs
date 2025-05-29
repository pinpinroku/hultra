use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{constant::MOD_DEPENDENCY_GRAPH, fetch, local::Dependency};

/// Each entry of the `mod_dependency_graph.yaml`.
#[derive(Debug, Deserialize, Serialize, Clone, Hash, PartialEq, Eq)]
pub struct ModDependency {
    #[serde(rename = "OptionalDependencies")]
    pub optional_dependencies: Vec<Dependency>,
    #[serde(rename = "Dependencies")]
    pub dependencies: Vec<Dependency>,
    #[serde(rename = "URL")]
    pub url: String,
}

/// Represents `mod_dependency_graph.yaml` which is the dependency graph.
pub type DependencyGraph = HashMap<String, ModDependency>;

pub trait ModDependencyQuery {
    async fn fetch(client: &Client) -> Result<DependencyGraph>;
    fn get_mod_info_by_name(&self, name: &str) -> Option<&ModDependency>;
    fn collect_all_dependencies_bfs(&self, mod_name: &str) -> HashSet<String>;
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
            }
        }

        visited
    }
}
