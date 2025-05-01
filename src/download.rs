use futures_util::StreamExt;
use reqwest::Client;
use std::path::{Path, PathBuf};
use tokio::{fs, io::AsyncWriteExt};
use tracing::{debug, error, info};
use xxhash_rust::xxh64::Xxh64;

use crate::{error::Error, fileutil::replace_home_dir_with_tilde};

pub mod install;
pub mod update;

#[allow(dead_code)] // NOTE: Maybe use this for progress bar in the future.
/// Get the file size
async fn get_file_size(client: &Client, url: &str) -> Result<u64, Error> {
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

/// Download a single mod file
pub async fn download_mod(
    client: &Client,
    mod_name: &str,
    url: &str,
    expected_hashes: &[String],
    download_dir: &Path,
) -> Result<PathBuf, Error> {
    info!("📥 Downloading {}", mod_name);

    debug!("URL: {}", url);
    debug!(
        "Destination directory: {}",
        replace_home_dir_with_tilde(download_dir)
    );

    let response = client.get(url).send().await?.error_for_status()?;
    debug!("Response status: {}", response.status());

    let filename = util::determine_filename(response.url(), response.headers());
    let download_path = download_dir.join(&filename);

    info!("Saving as: {}", filename);
    debug!("Full path: {}", replace_home_dir_with_tilde(&download_path));

    let computed_hash = download_and_write(response, &download_path).await?;

    verify_checksum(computed_hash, expected_hashes, &download_path).await?;

    Ok(download_path)
}

async fn download_and_write(
    response: reqwest::Response,
    download_path: &Path,
) -> Result<u64, Error> {
    info!(
        "Start writing data to the destination: {}",
        replace_home_dir_with_tilde(download_path)
    );
    let mut stream = response.bytes_stream();
    let mut hasher = Xxh64::new(0);
    let mut file = fs::File::create(download_path).await?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        hasher.update(&chunk);
    }

    Ok(hasher.digest())
}

/// Verifies the checksum of the downloaded file.
async fn verify_checksum(
    computed_hash: u64,
    expected_hashes: &[String],
    download_path: &Path,
) -> Result<(), Error> {
    let hash_str = format!("{:016x}", computed_hash);
    info!("🔍 Verifying checksum...");
    debug!(
        "Xxhash in u64: {:#?}, formatted string: {:#?}",
        computed_hash, hash_str
    );
    debug!(
        "Checking computed hash: {} against expected: {:?}",
        hash_str, expected_hashes
    );

    if expected_hashes.contains(&hash_str) {
        info!("✅ Checksum verified!");
        Ok(())
    } else {
        error!("❌ Checksum verification failed!");
        fs::remove_file(&download_path).await?;
        info!("🗑️ Downloaded file removed.");
        Err(Error::InvalidChecksum {
            file: download_path.to_path_buf(),
            computed: hash_str,
            expected: expected_hashes.to_vec(),
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
            .map(|etag| etag.trim_matches('"'))
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
