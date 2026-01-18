use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use bytes::Bytes;
use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;
use tokio::{fs, io::AsyncWriteExt, sync::Semaphore, time::Duration};
use tracing::{error, info, instrument, warn};
use xxhash_rust::xxh64::Xxh64;

use crate::{config::AppConfig, mirrorlist, registry::RemoteMod};

/// A kind of database.
#[derive(Debug, Clone, Copy)]
pub enum DatabaseKind {
    Update,
    DependencyGraph,
}

/// A type of database URL set.
#[derive(Debug, Clone, Copy, Default)]
pub enum DatabaseUrlSet {
    #[default]
    Primary,
    Mirror,
}

impl DatabaseUrlSet {
    #[inline]
    pub fn get_url(self, kind: DatabaseKind) -> &'static str {
        match (self, kind) {
            (Self::Primary, DatabaseKind::Update) => {
                "https://maddie480.ovh/celeste/everest_update.yaml"
            }
            (Self::Primary, DatabaseKind::DependencyGraph) => {
                "https://maddie480.ovh/celeste/mod_dependency_graph.yaml"
            }
            (Self::Mirror, DatabaseKind::Update) => {
                "https://everestapi.github.io/updatermirror/everest_update.yaml"
            }
            (Self::Mirror, DatabaseKind::DependencyGraph) => {
                "https://everestapi.github.io/updatermirror/mod_dependency_graph.yaml"
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum DownloadError {
    #[error(transparent)]
    Network(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(
        "failed to verify checksum for {file_path:?}: computed {computed}, expected {expected:?}"
    )]
    FileHashMissMatch {
        file_path: PathBuf,
        computed: u64,
        expected: Vec<u64>,
    },
}

#[derive(Debug, Clone)]
pub struct Downloader {
    client: Client,
    semaphore: Arc<Semaphore>,
}

impl Downloader {
    pub fn new(timeout: u64, limit: usize) -> Self {
        Self {
            client: Client::builder()
                .https_only(true)
                .gzip(true)
                .timeout(Duration::from_secs(timeout))
                .build()
                .inspect_err(|err| warn!(?err, "failed to build client, fallbacks to default"))
                .unwrap_or_default(),
            semaphore: Arc::new(Semaphore::new(limit)),
        }
    }

    /// Fetches a database from the specified URL set and kind.
    #[instrument(skip(self, config))]
    pub async fn fetch_database(
        &self,
        kind: DatabaseKind,
        config: &AppConfig,
    ) -> Result<Bytes, reqwest::Error> {
        info!("starting to fetch");
        let db_url = config.url_set().get_url(kind);
        let response = self
            .client
            .get(db_url)
            .send()
            .await
            .inspect_err(|err| error!(?err, "failed to receive response"))?
            .error_for_status()
            .inspect_err(|err| error!(?err, "got bad status"))?;
        let bytes = response
            .bytes()
            .await
            .inspect_err(|err| error!(?err, "failed to get full response body as bytes"))?;
        info!("successfully fetched");
        // HACK we don't need Bytes if we deserialize response directly at here
        // HACK to inmplement that, we need to introduce trait object ApiResponse for ModRegistry and DependencyGraph
        Ok(bytes)
    }

    /// Download multiple files concurrently with a limit on the number of simultaneous downloads.
    #[instrument(skip_all)]
    pub async fn download_files(&self, mods: HashMap<String, RemoteMod>, config: &AppConfig) {
        if mods.is_empty() {
            info!("no mods to download");
            return;
        }

        info!("starting to download files");

        let mp = MultiProgress::new();

        let handles: Vec<_> = mods
            .iter()
            .map(|(name, remote_mod)| {
                let client = self.client.clone();
                let semaphore = self.semaphore.clone();
                let pb = mp.add(create_download_progress_bar(name));

                let mod_name = name.to_owned();
                let mod_info = remote_mod.clone();

                let config = config.clone();

                tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    Self::retry_download_with_mirrros(&client, &mod_name, &mod_info, &pb, &config)
                        .await;
                })
            })
            .collect();

        // Wait for all downloads to complete
        for handle in handles {
            if let Err(err) = handle.await {
                error!(?err)
            }
        }

        info!("successfully downloaded all mods")
    }

    /// Retry downloading a file from multiple mirrors until success or all mirrors are exhausted.
    #[instrument(skip(client, pb, config))]
    async fn retry_download_with_mirrros(
        client: &Client,
        mod_name: &str,
        mod_info: &RemoteMod,
        pb: &ProgressBar,
        config: &AppConfig,
    ) {
        let mut success = false;

        let clean_name = util::sanitize_stem(mod_name).await;
        let file_path = config.mods_dir().join(clean_name).with_extension("zip");

        let mirror_urls =
            mirrorlist::generate_mirrors(&mod_info.download_url, config.mirror_priority()).await;

        for url in mirror_urls {
            match Self::download_file(
                client,
                &url,
                mod_info.file_size,
                &mod_info.checksums,
                &file_path,
                pb,
            )
            .await
            {
                Ok(_) => {
                    info!("download completed");
                    pb.finish_with_message(format!("{} 🍓", mod_name));
                    success = true;
                    break;
                }
                Err(err) => {
                    warn!(?err, "failed to download, trying another mirror");
                    pb.set_message(format!(
                        "{}: Failed to download, trying another mirror.",
                        mod_name
                    ));
                    pb.reset();
                }
            }
        }

        // indicates failure for all mirrors
        if !success {
            error!(
                "failed to download '{}' after trying all mirrors.",
                mod_name
            );
            pb.finish_with_message(format!("{} ❌ Failed", mod_name));
        }
    }

    /// Downloads a single file while hashing the file.
    #[instrument(skip(client, expected_hashes, pb))]
    async fn download_file(
        client: &Client,
        url: &str,
        file_size: u64,
        expected_hashes: &[u64],
        file_path: &Path,
        pb: &ProgressBar,
    ) -> Result<(), DownloadError> {
        info!("starting to download mod");
        let response = client
            .get(url)
            .send()
            .await
            .inspect_err(|err| error!(?err))?
            .error_for_status()
            .inspect_err(|err| error!(?err))?;

        let total_size = response.content_length().unwrap_or(file_size);
        pb.set_length(total_size);
        pb.reset();

        // NOTE `tmpfs` vs `in-memory buffer`: It doesn't matter for modern linux system
        let mut buffer = Vec::with_capacity(total_size as usize);

        let mut hasher = Xxh64::new(0);
        let mut stream = response.bytes_stream();

        info!("starting to retrieve response body");
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.inspect_err(|err| error!(?err))?;
            hasher.update(&chunk);
            buffer.extend_from_slice(&chunk);
            pb.inc(chunk.len() as u64);
        }

        let computed_hash = hasher.digest();
        if !expected_hashes.contains(&computed_hash) {
            return Err(DownloadError::FileHashMissMatch {
                file_path: file_path.to_path_buf(),
                computed: computed_hash,
                expected: expected_hashes.to_vec(),
            });
        }
        info!(computed=computed_hash, expected=?expected_hashes, "file hash matched");

        // NOTE BufWriter has no significant performance improvement here
        let mut file = fs::File::create(file_path)
            .await
            .inspect_err(|err| error!(?err))?;
        file.write_all(&buffer).await?;
        file.flush().await?;

        info!("successfully downloaded and saved");
        // HACK it'd be better to return `computed_hash`, `file_path`, `mtime`, and `size`

        Ok(())
    }
}

/// Create a progress bar for downloading a file.
fn create_download_progress_bar(mod_name: &str) -> ProgressBar {
    let pb = ProgressBar::hidden();
    pb.set_style(
        ProgressStyle::with_template(
            "{wide_msg} {total_bytes:>10.1.cyan/blue} {bytes_per_sec:>11.2} {elapsed_precise:>8} [{bar:>40}] {percent:>3}%"
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("#>-")
    );
    pb.set_message(mod_name.to_string());
    pb
}

/// Create a spinner progress bar for fetching online database.
pub fn create_spinner() -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner.set_style(
        ProgressStyle::with_template("{spinner:.bold} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    spinner.set_message("fetching databases...");
    spinner
}

mod util {
    /// Sanitizes a mod name as file stem for Unix file systems.
    ///
    /// # Rules
    /// - Trims leading/trailing whitespace.
    /// - Removes control characters.
    /// - Replaces characters not in the whitelist `[A-Za-z0-9 -_'()]` with `_`.
    /// - Truncates the result to 255 bytes.
    ///
    /// # Panics
    /// All characters in given string must be ASCII, otherwise it will panic.
    ///
    /// # Notes
    /// Mod database only allows ASCII characters for the mod name. So the name should always valid UTF-8 and ASCII.
    pub async fn sanitize_stem(input: &str) -> String {
        let trimmed = input.trim();

        assert!(
            trimmed.is_ascii(),
            "Input string should contains only ASCII characters"
        );

        let sanitized_bytes = trimmed
            .bytes()
            .filter(|c| !c.is_ascii_control())
            .map(|c| {
                if c.is_ascii_alphanumeric() || is_allowed_byte(c) {
                    c
                } else {
                    b'_'
                }
            })
            .take(u8::MAX as usize)
            .collect();

        // NOTE This is safe because `input` is always valid UFT-8 and ASCII
        unsafe { String::from_utf8_unchecked(sanitized_bytes) }
    }

    /// Checks if a byte is allowed in the filename stem.
    #[inline(always)]
    fn is_allowed_byte(b: u8) -> bool {
        matches!(
            b,
            b'A'..=b'Z' |            // Uppercase
            b'a'..=b'z' |            // Lowercase
            b'0'..=b'9' |            // Digits
            b' ' | b'-' | b'_' |     // Separators
            b'\'' | b'(' | b')' |    // Special allowed chars
            b'+' | b','              // Special allowed chars 2 (common in mods name)
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn test_no_change() {
            let input = "valid-filename_123(final)";
            let result = sanitize_stem(input).await;
            assert_eq!(result, "valid-filename_123(final)");
        }

        #[tokio::test]
        async fn test_replace_invalid_chars() {
            let input = "file!?.txt";
            let result = sanitize_stem(input).await;
            assert_eq!(result, "file___txt");
        }

        #[tokio::test]
        async fn test_remove_control_chars() {
            // Control chars should be removed, not replaced
            let input = "file\0name\n";
            let result = sanitize_stem(input).await;
            assert_eq!(result, "filename");
        }

        #[tokio::test]
        async fn test_mixed_whitelist() {
            // Ensure added whitelist chars ' and () are respected
            let input = "  Spooooky's Asset Pack (WIP)  ";
            let result = sanitize_stem(input).await;
            assert_eq!(result, "Spooooky's Asset Pack (WIP)");
        }

        #[tokio::test]
        #[should_panic(expected = "Input string should contains only ASCII characters")]
        async fn test_panic_on_non_ascii() {
            sanitize_stem("Error_日本語").await;
        }
    }
}
