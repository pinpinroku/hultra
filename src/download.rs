use bytes::Bytes;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::path::{Path, PathBuf};
use tokio::{fs, io::AsyncWriteExt};
use tracing::{error, info};
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
        println!("Fetching online database...");
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
        // TODO: Handling errors like 404 or 500+
        let response = self.client.get(url).send().await?.error_for_status()?;
        info!("[{}] Status code: {:#?}", name, response.status());

        let filename = util::determine_filename(response.url(), response.headers());
        let download_path = self.download_dir.join(filename);
        info!("[{}] Destination: {:#?}", name, download_path);

        let total_size = response.content_length().unwrap_or(0);
        info!("[{}] Total file size: {}", name, total_size);

        let pb = set_progress_bar_style(name, total_size);

        let computed_hash = download_and_write(response, &download_path, pb).await?;

        info!("\n[{}] 🔍 Verifying checksum...", name);
        verify_checksum(computed_hash, expected_hash, &download_path).await?;

        Ok(())
    }
}

/// Set up progress bar style using template.
fn set_progress_bar_style(name: &str, total_size: u64) -> ProgressBar {
    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::with_template(
            "{msg:<} {total_bytes:>40.1.cyan/blue} {bytes_per_sec:.2} {eta_precise:} {bar:60} {percent:}%",
        )
        .expect("Invalid progress bar style. Should be configured properly.")
    );

    // If the name is too long, truncate it and add an elipsis at the end.
    let mut name = name.to_string();
    let max_size = 40;
    if !name.len() <= max_size {
        name = format!("{}...", &name[..max_size - 3])
    }

    pb.set_message(name);
    pb
}

/// Downloads the mod and writes it to a file, updating the progress bar. Returns computed_hash.
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
