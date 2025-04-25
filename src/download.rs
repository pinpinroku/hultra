use bytes::Bytes;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::path::{Path, PathBuf};
use tokio::{fs, io::AsyncWriteExt};
use tracing::info;
use xxhash_rust::xxh64::Xxh64;

use crate::{constant::MOD_REGISTRY_URL, error::Error};

/// Manages mod downloads and registry fetching.
#[derive(Debug, Clone)]
pub struct ModDownloader {
    client: Client,
    registry_url: String,
    download_dir: PathBuf,
}

impl ModDownloader {
    /// Creates a new `ModDownloader` with the specified download directory.
    ///
    /// # Parameters
    /// - `download_dir`: The directory where downloaded mods will be stored.
    ///
    /// # Returns
    /// A new instance of `ModDownloader`.
    pub fn new(download_dir: &Path) -> Self {
        Self {
            client: Client::new(),
            registry_url: String::from(MOD_REGISTRY_URL),
            download_dir: download_dir.to_path_buf(),
        }
    }

    /// Fetches the remote mod registry.
    ///
    /// # Returns
    /// - `Ok(Bytes)`: The raw bytes of the registry file upon success.
    /// - `Err(Error)`: An error if the request or parsing fails.
    pub async fn fetch_mod_registry(&self) -> Result<Bytes, Error> {
        info!("Fetching remote mod registry...");
        let response = self.client.get(&self.registry_url).send().await?;
        let yaml_data = response.bytes().await?;
        Ok(yaml_data)
    }

    /// Downloads a mod file, saves it locally, and verifies its integrity.
    ///
    /// # Parameters
    /// - `url`: The URL to download the mod from.
    /// - `name`: The mod's name (used for logging).
    /// - `expected_hash`: Slice of acceptable xxHash checksum strings (in hexadecimal).
    ///
    /// # Returns
    /// - `Ok(())` if the download and checksum verification succeed.
    /// - `Err(Error)` if an error occurs during download or checksum verification.
    pub async fn download_mod(
        &self,
        url: &str,
        name: &str,
        expected_hash: &[String],
    ) -> Result<(), Error> {
        println!("\nDownloading {}:", name);

        let response = self.client.get(url).send().await?.error_for_status()?;
        info!("Status code: {:#?}", response.status());

        let filename = util::determine_filename(response.url(), response.headers());
        let download_path = self.download_dir.join(filename);
        info!("Destination: {:#?}", download_path);

        let total_size = response.content_length().unwrap_or(0);
        info!("Total file size: {}", total_size);

        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("#>-"),
        );

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

        let hash = hasher.digest();
        let hash_str = format!("{:016x}", hash);
        info!(
            "Xxhash in u64: {:#?}, formatted string: {:#?}",
            hash, hash_str
        );

        // Verify checksum
        println!("\n🔍 Verifying checksum of the mod '{}'", name);
        if expected_hash.contains(&hash_str) {
            println!("✅ Checksum verified!");
        } else {
            println!("❌ Checksum verification failed!");
            fs::remove_file(&download_path).await?;
            println!("[Cleanup] Downloaded file removed 🗑️");
            return Err(Error::InvalidChecksum {
                file: download_path,
                computed: hash_str,
                expected: expected_hash.to_vec(),
            });
        }

        Ok(())
    }
}

/// Utility functions for determining filenames and handling mod download metadata.
mod util {
    use reqwest::{Url, header::HeaderMap};
    use uuid::Uuid;

    /// Determines the most appropriate filename for a downloaded mod using the URL and headers.
    ///
    /// # Parameters
    /// - `url`: The URL from which to extract the filename.
    /// - `headers`: The HTTP headers from which to extract the ETag.
    ///
    /// # Returns
    /// - `String`: The determined filename.
    pub fn determine_filename(url: &Url, headers: &HeaderMap) -> String {
        extract_filename_from_url(url)
            .or_else(|| extract_filename_from_etag(headers))
            .unwrap_or_else(|| format!("unknown-mod_{}.zip", Uuid::new_v4()))
    }

    /// Extracts a filename from the last segment of a URL path.
    fn extract_filename_from_url(url: &Url) -> Option<String> {
        url.path_segments()
            .and_then(|mut segments| segments.next_back().filter(|&segment| !segment.is_empty()))
            .map(String::from)
    }

    /// Extracts a filename from the ETag header, appending a `.zip` extension.
    fn extract_filename_from_etag(headers: &HeaderMap) -> Option<String> {
        headers
            .get(reqwest::header::ETAG)
            .and_then(|etag_value| etag_value.to_str().ok())
            .map(|etag| etag.trim_matches('"').to_string())
            .map(|etag| format!("{}.zip", etag))
    }

    #[cfg(test)]
    mod tests_util {
        use super::*;
        use reqwest::{
            Url,
            header::{HeaderMap, HeaderValue},
        };
        use uuid::Uuid;

        #[test]
        fn test_extract_filename_from_url_valid() {
            let url = Url::parse("https://files.gamebanana.com/mods/hateline_v022.zip").unwrap();
            let result = extract_filename_from_url(&url);
            assert_eq!(result, Some("hateline_v022.zip".to_string()));
        }

        #[test]
        fn test_extract_filename_from_url_empty_segment() {
            let url = Url::parse("https://gamebanana.com/mods/").unwrap();
            let result = extract_filename_from_url(&url);
            assert_eq!(result, None);
        }

        #[test]
        fn test_extract_filename_from_url_no_segments() {
            let url = Url::parse("https://gamebanana.com").unwrap();
            let result = extract_filename_from_url(&url);
            assert_eq!(result, None);
        }

        #[test]
        fn test_extract_filename_from_etag_valid() {
            let mut headers = HeaderMap::new();
            headers.insert(
                reqwest::header::ETAG,
                HeaderValue::from_static("\"eclair\""),
            );
            let result = extract_filename_from_etag(&headers);
            assert_eq!(result, Some("eclair.zip".to_string()));
        }

        #[test]
        fn test_extract_filename_from_etag_missing() {
            let headers = HeaderMap::new();
            let result = extract_filename_from_etag(&headers);
            assert_eq!(result, None);
        }

        #[test]
        fn test_extract_filename_from_etag_invalid() {
            let mut headers = HeaderMap::new();
            headers.insert(
                reqwest::header::ETAG,
                HeaderValue::from_static("invalid-etag"),
            );
            let result = extract_filename_from_etag(&headers);
            assert_eq!(result, Some("invalid-etag.zip".to_string()));
        }

        #[test]
        fn test_determine_filename_from_url() {
            let url = Url::parse("https://files.gamebanana.com/mods/hateline_v022.zip").unwrap();
            let headers = HeaderMap::new();
            let result = determine_filename(&url, &headers);
            assert_eq!(result, "hateline_v022.zip");
        }

        #[test]
        fn test_determine_filename_from_etag() {
            let url = Url::parse("https://gamebanana.com/mods/").unwrap();
            let mut headers = HeaderMap::new();
            headers.insert(reqwest::header::ETAG, HeaderValue::from_static("\"glyph\""));
            let result = determine_filename(&url, &headers);
            assert_eq!(result, "glyph.zip");
        }

        #[test]
        fn test_determine_filename_fallback_to_uuid() {
            let url = Url::parse("https://gamebanana.com").unwrap();
            let headers = HeaderMap::new();
            let result = determine_filename(&url, &headers);
            assert!(result.starts_with("unknown-mod_"));
            assert!(result.ends_with(".zip"));
            // Verify the UUID part is valid
            let uuid_str = result
                .strip_prefix("unknown-mod_")
                .unwrap()
                .strip_suffix(".zip")
                .unwrap();
            Uuid::parse_str(uuid_str).expect("Generated filename should contain a valid UUID");
        }

        #[test]
        fn test_determine_filename_url_preferred_over_etag() {
            let url = Url::parse("https://files.gamebanana.com/mods/hateline_v022.zip").unwrap();
            let mut headers = HeaderMap::new();
            headers.insert(
                reqwest::header::ETAG,
                HeaderValue::from_static("\"hateline\""),
            );
            let result = determine_filename(&url, &headers);
            assert_eq!(result, "hateline_v022.zip");
        }
    }
}
