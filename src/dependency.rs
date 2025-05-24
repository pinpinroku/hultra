use serde::Deserialize;

use crate::{error::Error, fetch};

#[derive(Debug, Deserialize)]
pub struct DependencyGraph {}
/// Fetches the DependencyGraph to resolve the dependencies of the installed mod.
pub async fn _fetch_dependency_graph() -> Result<DependencyGraph, Error> {
    fetch::fetch_remote_data::<DependencyGraph>("DEPENDENCY_GRAPH_URL", "dependency information")
        .await
}
