#![allow(dead_code)]
use std::path::Path;

use futures_util::StreamExt;
use indicatif::ProgressBar;
use reqwest::Client;
use tokio::{fs, io::AsyncWriteExt};
use tracing::{debug, error, info};
use xxhash_rust::xxh64::Xxh64;

use crate::error::Error;

/// Get the file size
pub async fn get_file_size(client: Client, url: &str) -> Result<u64, Error> {
    debug!(
        "Get the file size by sending HEAD request to the server: {}",
        url
    );

    let response = client.head(url).send().await?.error_for_status()?;
    debug!("Status code: {:#?}", response.status());

    let total_size = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|length_header| length_header.to_str().ok())
        .and_then(|length_str| length_str.parse::<u64>().ok())
        .unwrap_or(0);
    debug!("Total size: {}", total_size);

    Ok(total_size)
}

/// Download an archived file from gamebanana server
pub async fn download_file(
    client: Client,
    name: &str,
    url: &str,
    expected_hashes: &[String],
    download_dir: &Path,
    pb: ProgressBar,
) -> Result<(), Error> {
    // TODO: Handling errors like 404 or 500+
    let response = client.get(url).send().await?.error_for_status()?;
    debug!("[{}] Status code: {:#?}", name, response.status());

    let filename = crate::download::util::determine_filename(response.url(), response.headers());
    let download_path = download_dir.join(filename);
    info!("[{}] Destination: {:#?}", name, download_path);

    let total_size = response.content_length().unwrap_or(0);
    info!("[{}] Total file size: {}", name, total_size);

    pb.set_length(total_size);

    let computed_hash = download_and_write(response, &download_path, pb).await?;

    info!("\n[{}] 🔍 Verifying checksum...", name);
    verify_checksum(computed_hash, expected_hashes, &download_path).await?;

    Ok(())
}

async fn download_and_write(
    response: reqwest::Response,
    download_path: &Path,
    pb: ProgressBar,
) -> Result<u64, Error> {
    let mut stream = response.bytes_stream();
    let mut hasher = Xxh64::new(0);
    let mut file = fs::File::create(download_path).await?;
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        hasher.update(&chunk);
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        pb.set_position(downloaded);
    }

    pb.finish();

    Ok(hasher.digest())
}

/// Verifies the checksum of the downloaded file.
async fn verify_checksum(
    computed_hash: u64,
    expected_hash: &[String],
    download_path: &Path,
) -> Result<(), Error> {
    let hash_str = format!("{:016x}", computed_hash);
    info!(
        "Xxhash in u64: {:#?}, formatted string: {:#?}",
        computed_hash, hash_str
    );

    if expected_hash.contains(&hash_str) {
        info!("✅ Checksum verified!");
        Ok(())
    } else {
        error!("❌ Checksum verification failed!");
        fs::remove_file(&download_path).await?;
        info!("[Cleanup] Downloaded file removed 🗑️");
        Err(Error::InvalidChecksum {
            file: download_path.to_path_buf(),
            computed: hash_str,
            expected: expected_hash.to_vec(),
        })
    }
}
