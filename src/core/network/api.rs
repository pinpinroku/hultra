//! API Client.
//!
//! Fetches mod registry and dependency graph from server.
use std::time::Duration;

use reqwest::Client;
use tokio::try_join;
use tracing::instrument;

use crate::{
    commands::DownloadOption, core::registry::EverestUpdateYaml, dependency::DependencyGraph,
    ui::create_spinner,
};

/// Fetches registry and graph at once.
pub async fn fetch(
    client: Client,
    opt: &DownloadOption,
) -> anyhow::Result<(EverestUpdateYaml, DependencyGraph)> {
    let api_client = ApiClient::new(client);
    let source = ApiSource::from(opt);

    let spinner = create_spinner();
    let (registry, graph) = try_join!(
        api_client.fetch_everest_update_yaml(source),
        api_client.fetch_graph(source)
    )?;
    spinner.finish_and_clear();
    Ok((registry, graph))
}

/// Fetches registry.
pub async fn fetch_registry(
    client: Client,
    opt: &DownloadOption,
) -> anyhow::Result<EverestUpdateYaml> {
    let api_client = ApiClient::new(client);
    let source = ApiSource::from(opt);

    let spinner = create_spinner();
    let registry = api_client.fetch_everest_update_yaml(source).await?;
    spinner.finish_and_clear();
    Ok(registry)
}

/// Client for API.
#[derive(Debug, Clone)]
pub struct ApiClient {
    client: reqwest::Client,
}

/// API sources.
#[derive(Debug, Clone, Copy)]
pub enum ApiSource {
    Primary,
    Mirror,
}

impl From<&DownloadOption> for ApiSource {
    fn from(value: &DownloadOption) -> Self {
        if value.use_api_mirror {
            Self::Mirror
        } else {
            Self::Primary
        }
    }
}

/// API Resouces.
#[derive(Debug, Clone, Copy)]
enum ApiResource {
    Registry,
    DependencyGraph,
}

impl ApiSource {
    fn url_for(&self, resource: ApiResource) -> &'static str {
        match (self, resource) {
            (Self::Primary, ApiResource::Registry) => {
                "https://maddie480.ovh/celeste/everest_update.yaml"
            }
            (Self::Primary, ApiResource::DependencyGraph) => {
                "https://maddie480.ovh/celeste/mod_dependency_graph.yaml"
            }
            (Self::Mirror, ApiResource::Registry) => {
                "https://everestapi.github.io/updatermirror/everest_update.yaml"
            }
            (Self::Mirror, ApiResource::DependencyGraph) => {
                "https://everestapi.github.io/updatermirror/mod_dependency_graph.yaml"
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ApiError {
    #[error("Failed to fetch database")]
    Network(#[from] reqwest::Error),
    #[error("Failed to parse API response as YAML format")]
    DeserializeYaml(#[from] serde_yaml_ng::Error),
}

impl ApiClient {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    #[instrument(skip(self))]
    async fn fetch_yaml<T>(&self, source: ApiSource, resource: ApiResource) -> Result<T, ApiError>
    where
        for<'de> T: serde::Deserialize<'de>,
    {
        let url = source.url_for(resource);

        let bytes = self
            .client
            .get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        Ok(serde_yaml_ng::from_slice(&bytes)?)
    }

    pub async fn fetch_everest_update_yaml(
        &self,
        source: ApiSource,
    ) -> Result<EverestUpdateYaml, ApiError> {
        self.fetch_yaml(source, ApiResource::Registry).await
    }

    pub async fn fetch_graph(&self, source: ApiSource) -> Result<DependencyGraph, ApiError> {
        self.fetch_yaml(source, ApiResource::DependencyGraph).await
    }
}
