//! src/core/network.rs
use reqwest::Client;

pub mod api;
pub mod downloader;

/// Shared Client for API fetching and mod downloading.
#[derive(Debug)]
pub struct SharedHttpClient {
    inner: Client,
}

impl SharedHttpClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .https_only(true)
            .gzip(true)
            .build()
            .unwrap_or_default();
        Self { inner: client }
    }

    pub fn inner(&self) -> &Client {
        &self.inner
    }
}
