#![allow(dead_code)]
use bytes::Bytes;
use reqwest::Client;

use crate::error::Error;

/// Fetch the remote mod registry
pub async fn fetch_mod_registry(client: Client) -> Result<Bytes, Error> {
    let url = "https://maddie480.ovh/celeste/everest_update.yaml";
    let response = client.get(url).send().await?;
    Ok(response.bytes().await?)
}

/// Get the file size
pub async fn get_file_size(client: Client, url: &str) -> Result<u64, Error> {
    let response = client.head(url).send().await?.error_for_status()?;
    let total_size = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|length_header| length_header.to_str().ok())
        .and_then(|length_str| length_str.parse::<u64>().ok())
        .unwrap_or(0);
    Ok(total_size)
}
