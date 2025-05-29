use std::collections::HashMap;

use anyhow::Result;
use reqwest::Client;
use serde::de::DeserializeOwned;

use crate::{
    dependency::{DependencyGraph, ModDependency, ModDependencyQuery},
    mod_registry::{ModRegistryQuery, RemoteModInfo, RemoteModRegistry},
};

/// Fetches the remote data from the given URL and parses it into the specified type.
pub async fn fetch_remote_data<T>(url: &str, client: &Client) -> Result<T>
where
    T: DeserializeOwned,
{
    let response = client.get(url).send().await?.error_for_status()?;

    tracing::debug!("Response headers: {:#?}", response.headers());
    let bytes = response.bytes().await?;

    tracing::info!("Parsing the binary data from the response");
    let data = serde_yaml_ng::from_slice::<T>(&bytes)?;

    Ok(data)
}

/// Fetches online database.
pub async fn fetch_online_database() -> Result<(
    HashMap<String, RemoteModInfo>,
    HashMap<String, ModDependency>,
)> {
    let client = reqwest::ClientBuilder::new()
        .http2_prior_knowledge()
        .gzip(true)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let spinner = crate::download::pb_style::create_spinner();
    let (mod_registry, dependency_graph) = tokio::try_join!(
        RemoteModRegistry::fetch(&client),
        DependencyGraph::fetch(&client)
    )?;
    spinner.finish_and_clear();
    Ok((mod_registry, dependency_graph))
}
