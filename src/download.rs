use std::{borrow::Cow, fs, io::Write, path::Path, sync::Arc, time::Duration};

use anyhow::Result;
use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar};
use reqwest::{Client, Response};
use tempfile::NamedTempFile;
use tokio::sync::Semaphore;
use xxhash_rust::xxh64::Xxh64;

use crate::{config::Config, download, fileutil, mod_registry::RemoteModInfo};

mod util;

/// Downloads a mod file, returns the file path.
async fn download_mod(
    client: &Client,
    mod_name: &str,
    mirror_urls: &[Cow<'_, str>],
    expected_hashes: &[String],
    download_dir: &Path,
    pb: &ProgressBar,
) -> Result<()> {
    tracing::debug!("Original mod name: {}", mod_name);
    let sanitized_name = util::sanitize(mod_name);

    tracing::debug!("Sanitized name: {}", sanitized_name);
    let filename = format!("{}.zip", &sanitized_name);

    let install_destination = download_dir.join(&filename);
    tracing::debug!(
        "Install destination: {}",
        fileutil::replace_home_dir_with_tilde(&install_destination)
    );

    let msg = pb_style::truncate_msg(mod_name);

    for url in mirror_urls {
        let response = client.get(url.as_ref()).send().await?;
        if response.status().is_success() {
            pb.set_message(msg.to_string());
            match download_and_write(response, &install_destination, expected_hashes, pb).await {
                Ok(_) => {
                    pb.finish_with_message(format!("🍓 {mod_name} [{filename}]"));
                    return Ok(());
                }
                Err(e) => {
                    tracing::error!("{}", e);
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
) -> Result<()> {
    let debug_filename = fileutil::replace_home_dir_with_tilde(install_destination);
    let mut temp_file = NamedTempFile::new()?;

    let mut stream = response.bytes_stream();
    let mut hasher = Xxh64::new(0);

    tracing::info!("Verifying checksum for {}", debug_filename);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        temp_file.write_all(&chunk)?;
        hasher.update(&chunk);
        pb.inc(chunk.len() as u64);
    }
    let computed_hash = hasher.digest();
    let hash_str = format!("{computed_hash:016x}");

    tracing::debug!("computed hash: {:?}", hash_str,);
    tracing::debug!("expected hash: {:?}", expected_hashes);
    tracing::info!("Checksum verification passed for {}", debug_filename);

    if !expected_hashes.contains(&hash_str) {
        anyhow::bail!(
            "Checksum verification failed for '{}': computed hash '{}' does not match expected hashes: {:?}",
            debug_filename,
            hash_str,
            expected_hashes
        );
        // NOTE: The temp file will be removed automatically when they goes out scope
        // or when the program exits. So we don't need to remove it manually.
    }

    tracing::info!("Checksum verified");

    if install_destination.exists() {
        tracing::debug!(
            "'{}' is already exists. Trying to remove it",
            debug_filename
        );
        fs::remove_file(install_destination)?;
        tracing::info!("The previous version has been deleted");
    }

    // NOTE: The permissions are set to 0600 because of copy operation.
    // This is a restriction in the linux system which uses tempfs as external mount point.
    fs::copy(temp_file, install_destination)?;
    tracing::info!("The file saved in '{}'", debug_filename);

    Ok(())
}

/// Downloads mods concurrently with a limit on the number of concurrent downloads.
///
/// # Errors
/// Returns an error if any of the downloads fail or if there are issues with the tasks.
pub async fn download_mods_concurrently(
    mods: &[(String, RemoteModInfo)],
    config: Arc<Config>,
    concurrent_limit: usize,
) -> Result<()> {
    tracing::info!(
        "Preparing to download {} mods with concurrency limit {}",
        mods.len(),
        concurrent_limit
    );
    tracing::debug!(
        "Mods to download: {:?}",
        mods.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );

    if mods.is_empty() {
        tracing::info!("No mods to download");
        return Ok(());
    }

    let semaphore = Arc::new(Semaphore::new(concurrent_limit));
    let mp = MultiProgress::new();
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .build()?;

    let mut handles = Vec::with_capacity(mods.len());

    for (name, remote_mod) in mods {
        let semaphore = semaphore.clone();
        let config = config.clone();
        let client = client.clone();
        let mp = mp.clone();
        let name = name.clone();
        let remote_mod = remote_mod.clone();

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await?;
            let pb = mp.add(ProgressBar::new(remote_mod.file_size));
            pb.set_style(pb_style::new());
            let msg = pb_style::truncate_msg(&name);
            pb.set_message(msg.to_string());

            let mirror_urls = mirror_list::get_all_mirror_urls(
                &remote_mod.download_url,
                config.mirror_preferences(),
            );

            download::download_mod(
                &client,
                &name,
                &mirror_urls,
                &remote_mod.checksums,
                config.directory(),
                &pb,
            )
            .await
        });
        handles.push(handle);
    }

    let mut errors = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                tracing::error!("Failed to download the mod: {}", err);
                errors.push(err);
            }
            Err(err) => {
                tracing::error!("Failed to join tasks: {}", err);
                errors.push(err.into());
            }
        }
    }

    if errors.is_empty() {
        tracing::info!("Successfully download the mods")
    } else {
        for (i, error) in errors.iter().enumerate() {
            tracing::error!("Error {}: {}", i + 1, error)
        }
        anyhow::bail!("Failed to download the mods: {:?}", errors)
    }

    Ok(())
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
