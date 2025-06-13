use std::{borrow::Cow, path::Path};

use futures_util::StreamExt;
use indicatif::ProgressBar;
use reqwest::{Client, Response};
use tempfile::NamedTempFile;
use tokio::io::AsyncWriteExt;
use xxhash_rust::xxh64::Xxh64;

use crate::{error::Error, fileutil::replace_home_dir_with_tilde};

pub mod install;
pub mod update;

/// Returns sanitized mod name or "unnamed" if the given mod name is empty.
fn sanitize(mod_name: &str) -> Cow<'_, str> {
    const BAD_CHARS: [char; 6] = ['/', '\\', '*', '?', ':', ';'];

    let trimmed = mod_name.trim();
    let without_dot = trimmed.strip_prefix('.').unwrap_or(trimmed);

    let mut changed = false;
    let mut result = String::with_capacity(without_dot.len());

    for c in without_dot
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
    {
        let replacement = match c {
            '\r' | '\n' | '\0' => {
                changed = true;
                continue;
            }
            c if BAD_CHARS.contains(&c) => {
                changed = true;
                '_'
            }
            c => c,
        };
        result.push(replacement);
    }

    if result.len() > 255 {
        result.truncate(255);
        changed = true;
    }

    if result.is_empty() {
        Cow::Borrowed("unnamed")
    } else if !changed && result == mod_name {
        Cow::Borrowed(mod_name)
    } else {
        Cow::Owned(result)
    }
}

/// Downloads a mod file, returns the file path.
pub async fn download_mod(
    client: &Client,
    mod_name: &str,
    mirror_urls: &[Cow<'_, str>],
    expected_hashes: &[String],
    download_dir: &Path,
    pb: &ProgressBar,
) -> anyhow::Result<()> {
    tracing::debug!("Original mod name: {}", mod_name);
    let sanitized_name = sanitize(mod_name);

    tracing::debug!("Sanitized name: {}", sanitized_name);
    let filename = format!("{}.zip", &sanitized_name);

    let install_destination = download_dir.join(&filename);
    tracing::debug!(
        "Install destination: {}",
        replace_home_dir_with_tilde(&install_destination)
    );

    let msg = pb_style::truncate_msg(mod_name);

    for url in mirror_urls {
        let response = client.get(url.as_ref()).send().await?;
        if response.status().is_success() {
            pb.set_message(msg.to_string());
            match download_and_write(response, &install_destination, expected_hashes, pb).await {
                Ok(()) => {
                    pb.finish_with_message(format!("🍓 {} [{}]", mod_name, filename));
                    return Ok(());
                }
                Err(e) => {
                    tracing::error!("{}", e);
                    tracing::warn!("Checksum verification failed, trying another mirror");
                    pb.set_message("Checksum verification failed, trying another mirror");
                    continue; // to the next mirror
                }
            }
        } else {
            tracing::warn!("Status: {}", response.status());
            tracing::warn!("Download failed, trying another mirror");
            pb.set_message("Download failed, trying another mirror");
            continue; // to the next mirror
        }
    }
    pb.finish_and_clear();
    anyhow::bail!("Failed to download the mod: {}", mod_name)
}

/// Writes all bytes to the temporary file, verifies the checksum when the write is complete, and then moves them to the destination.
async fn download_and_write(
    response: Response,
    install_destination: &Path,
    expected_hashes: &[String],
    pb: &ProgressBar,
) -> Result<(), Error> {
    let temp_file = NamedTempFile::new()?;

    let mut stream = response.bytes_stream();
    let mut hasher = Xxh64::new(0);
    let mut file = tokio::fs::File::create(&temp_file).await?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        hasher.update(&chunk);
        pb.inc(chunk.len() as u64);
    }
    let computed_hash = hasher.digest();
    let hash_str = format!("{:016x}", computed_hash);

    tracing::info!("🔍 Verifying checksum...");
    tracing::debug!("computed hash: {:?}", hash_str,);
    tracing::debug!("expected hash: {:?}", expected_hashes);

    if expected_hashes.contains(&hash_str) {
        tracing::info!("✅ Checksum verified!");

        // Remove old file if it exists
        if install_destination.exists() {
            tracing::info!(
                "🗑  The previous version has been deleted. {}",
                replace_home_dir_with_tilde(install_destination)
            );
            tokio::fs::remove_file(install_destination).await?;
        }

        tracing::info!(
            "Moving the file to the destination: {}",
            replace_home_dir_with_tilde(install_destination)
        );

        // NOTE: The permissions are set to 0600 because of copy operation.
        // This is a restriction in the linux system which uses tempfs as external mount point.
        tokio::fs::copy(temp_file, install_destination).await?;

        Ok(())
    } else {
        tracing::error!("❌ Checksum verification failed!");
        // NOTE: The temp file will be removed automatically
        Err(Error::InvalidChecksum {
            file: install_destination.to_path_buf(),
            computed: hash_str,
            expected: expected_hashes.to_vec(),
        })
    }
}

/// Style configurations of a progress bar.
pub mod pb_style {
    use indicatif::{ProgressBar, ProgressStyle};
    use std::borrow::Cow;

    const MAX_MSG_LENGTH: usize = 40;
    const ELLIPSIS: &str = "...";

    /// Builds a ProgressBar style, fallbacks to the default.
    pub fn new() -> ProgressStyle {
        ProgressStyle::with_template(
        "{wide_msg} {total_bytes:>10.1.cyan/blue} {bytes_per_sec:>11.2} {elapsed_precise:>8} [{bar:>40}] {percent:>3}%",
    )
    .unwrap_or_else(|_| ProgressStyle::default_bar())
    .progress_chars("#>-")
    }

    /// Truncates a given string and adds an ellipsis at the end if the length exceeds `MAX_MSG_LENGTH`.
    pub fn truncate_msg(msg: &str) -> Cow<'_, str> {
        if msg.len() > MAX_MSG_LENGTH {
            Cow::Owned(format!(
                "{}{}",
                &msg[..MAX_MSG_LENGTH - ELLIPSIS.len()],
                ELLIPSIS
            ))
        } else {
            Cow::Borrowed(msg)
        }
    }

    pub fn create_spinner() -> ProgressBar {
        use indicatif::ProgressStyle;
        use std::time::Duration;

        let spinner = ProgressBar::new_spinner();
        spinner.enable_steady_tick(Duration::from_millis(100));
        spinner.set_style(
            ProgressStyle::with_template("{spinner:.bold} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        spinner.set_message("Fetching online database...");
        spinner
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_truncate_msg_no_truncation() {
            let msg = "Short message";
            // The message length is less than MAX_MSG_LENGTH.
            let result = truncate_msg(msg);
            assert_eq!(result, msg);
        }

        #[test]
        fn test_truncate_msg_exact_length() {
            let msg = "a".repeat(MAX_MSG_LENGTH);
            // If the message length is exactly MAX_MSG_LENGTH,
            // then it should not be truncated.
            let result = truncate_msg(&msg);
            assert_eq!(result, msg);
        }

        #[test]
        fn test_truncate_msg_with_truncation() {
            let original = "Long Name Helper by Helen, Helen's Helper, hELPER"; // 50 chars
            let result = truncate_msg(original);
            // Expected: first (MAX_MSG_LENGTH - ELLIPSIS.len()) characters plus ELLIPSIS.
            let expected = format!(
                "{}{}",
                &original[..(MAX_MSG_LENGTH - ELLIPSIS.len())],
                ELLIPSIS
            );
            assert_eq!(result, expected);
        }

        #[test]
        fn test_truncate_msg_empty_string() {
            let msg = "";
            let result = truncate_msg(msg);
            assert_eq!(result, msg);
        }
    }
}

/// Utility functions for determining filenames and handling mod download metadata.
#[allow(dead_code)]
mod download_util {
    use reqwest::{Url, header::HeaderMap};
    use uuid::Uuid;

    /// Determines the most appropriate filename for a downloaded mod using the URL and headers.
    ///
    /// # Arguments
    /// - `url`: The URL from which to extract the filename.
    /// - `headers`: The HTTP headers from which to extract the ETag.
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
