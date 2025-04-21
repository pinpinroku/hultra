use bytes::Bytes;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::path::{Path, PathBuf};
use tokio::{fs, io::AsyncWriteExt};
use tracing::info;
use xxhash_rust::xxh64::Xxh64;

use crate::{constant::MOD_REGISTRY_URL, error::Error};

/// Manage mod downloads
#[derive(Debug, Clone)]
pub struct ModDownloader {
    client: Client,
    registry_url: String,
    download_dir: PathBuf,
}

impl ModDownloader {
    pub fn new(download_dir: &Path) -> Self {
        Self {
            client: Client::new(),
            registry_url: String::from(MOD_REGISTRY_URL),
            download_dir: download_dir.to_path_buf(),
        }
    }

    /// Fetch remote mod registry, returns bytes of response
    pub async fn fetch_mod_registry(&self) -> Result<Bytes, Error> {
        info!("Fetching remote mod registry...");
        let response = self.client.get(&self.registry_url).send().await?;
        let yaml_data = response.bytes().await?;
        Ok(yaml_data)
    }

    /// Download mod file and verify checksum
    pub async fn download_mod(
        &self,
        url: &str,
        name: &str,
        expected_hash: &[String],
    ) -> Result<(), Error> {
        info!("Start downloading mod: {}", name);

        let response = self.client.get(url).send().await?.error_for_status()?;
        info!("Status code: {}", response.status().as_u16());

        let filename = util::determine_filename(&response)?;
        let download_path = self.download_dir.join(filename);
        info!("Destination: {}", download_path.display());

        let total_size = response.content_length().unwrap_or(0);
        info!("Total file size: {}", total_size);

        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"));

        let mut stream = response.bytes_stream();

        let mut hasher = Xxh64::new(0);
        let mut file = fs::File::create(&download_path).await?;
        let mut downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            hasher.update(&chunk);
            let new = std::cmp::min(downloaded + (chunk.len() as u64), total_size);
            downloaded = new;
            pb.set_position(new);
        }

        pb.finish_with_message("Download complete");

        // Verify checksum
        let hash = hasher.digest();
        let hash_str = format!("{:016x}", hash);
        info!("xxhash of downloaded file: {}", hash_str);

        println!("\n  Verifying checksum...");
        if expected_hash.contains(&hash_str) {
            println!("  Checksum verified!");
        } else {
            println!("  Checksum verification failed!");
            fs::remove_file(&download_path).await?;
            println!("  Downloaded file removed");
            return Err(Error::InvalidChecksum {
                file: download_path,
                computed: hash_str,
                expected: expected_hash.to_vec(),
            });
        }

        Ok(())
    }
}

mod util {
    use super::*;
    use reqwest::{Response, Url};
    use uuid::Uuid;

    /// Determines the most appropriate filename for a downloaded mod using URL and metadata
    pub fn determine_filename(response: &Response) -> Result<String, Error> {
        // Try to extract filename from the URL path.
        let filename_from_url = extract_filename_from_url(response.url());

        // Try to extract filename from the ETag header.
        let filename_from_etag = extract_filename_from_etag(response);

        // Choose the best available filename or generate a random one
        let mod_filename = filename_from_url
            .or(filename_from_etag)
            .unwrap_or_else(|| format!("unknown-mod_{}.zip", Uuid::new_v4()));

        Ok(mod_filename)
    }

    /// Extracts a filename from the last segment of a URL path
    fn extract_filename_from_url(url: &Url) -> Option<String> {
        url.path_segments()
            .and_then(|mut segments| segments.next_back().filter(|&segment| !segment.is_empty()))
            .map(String::from)
    }

    /// Creates a filename using the ETag header value, properly formatted with extension
    fn extract_filename_from_etag(response: &Response) -> Option<String> {
        response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|etag_value| etag_value.to_str().ok())
            .map(|etag| etag.trim_matches('"').to_string())
            .map(|etag| format!("{}.zip", etag))
    }
}
