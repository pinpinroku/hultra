use std::{fmt::Display, path::Path, str::FromStr, time::Duration};

use futures_util::StreamExt;
use indicatif::ProgressBar;
use reqwest::{Client, header::ACCEPT};
use tempfile::{Builder, NamedTempFile};
use tokio::io::AsyncWriteExt;
use tracing::instrument;

use crate::{
    config::{AppConfig, CARGO_PKG_NAME},
    everest::build::EverestBuild,
    log::anonymize,
};

/// Downloads Everest and extracts it to the root directory of Celeste.
pub async fn download(
    client: Client,
    build: &EverestBuild,
    config: &AppConfig,
) -> anyhow::Result<()> {
    let downloader = EverestDownloader::new(client);
    let resource = DownloadResource::try_from(build)?;

    let extract_dir = config.root_dir();
    let spinner = ProgressBar::new_spinner();
    spinner.enable_steady_tick(Duration::from_millis(120));
    spinner.set_message("Downloading Everest");

    downloader.run(&resource, extract_dir, &spinner).await?;
    Ok(())
}

/// Download reasource for the Everest.
#[derive(Debug, Clone)]
struct DownloadResource {
    /// Download URL of the Everest.
    url: EverestDownloadUrl,
    /// Validation for file integrity.
    size: u64,
}

impl Display for DownloadResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "URL: {}, Expected size: {}", self.url(), self.filesize())
    }
}

#[derive(Debug, Clone)]
struct EverestDownloadUrl(String);

impl Display for EverestDownloadUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// TODO make it enum and add more variants
// URL must contains either github or azure
#[derive(Debug, thiserror::Error)]
#[error("failed to convert given string to EversestDownloadUrl")]
struct ParseError;

impl FromStr for EverestDownloadUrl {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.ends_with("main.zip") {
            Ok(Self(s.to_string()))
        } else {
            Err(ParseError)
        }
    }
}

impl TryFrom<&EverestBuild> for DownloadResource {
    type Error = ParseError;

    fn try_from(build: &EverestBuild) -> Result<Self, ParseError> {
        let url = EverestDownloadUrl::from_str(&build.main_download)?;
        Ok(Self {
            url,
            size: build.main_file_size,
        })
    }
}

impl DownloadResource {
    fn url(&self) -> &str {
        &self.url.0
    }
    fn filesize(&self) -> u64 {
        self.size
    }
}

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("failed to download the mod")]
    Network(#[from] reqwest::Error),
    #[error("failed to save the mod")]
    Io(#[from] std::io::Error),
    #[error("failed to extract Everest to the root directory")]
    Archive(#[from] archive::ExtractError),
}

/// Download client for Everest update.
#[derive(Debug, Clone)]
struct EverestDownloader {
    client: Client,
}

impl EverestDownloader {
    fn new(client: Client) -> Self {
        Self { client }
    }
}

impl EverestDownloader {
    #[instrument(skip(spinner), fields(resource = %resource, extract_dir = %anonymize(extract_dir)))]
    async fn run(
        &self,
        resource: &DownloadResource,
        extract_dir: &Path,
        spinner: &ProgressBar,
    ) -> Result<(), Error> {
        let response = self
            .client
            .get(resource.url())
            .timeout(Duration::from_secs(90))
            .header(ACCEPT, "application/octet-stream")
            .send()
            .await?
            .error_for_status()?;

        // Use a temp file for "Verify-then-Commit" strategy.
        let temp_dir = Builder::new()
            .prefix(&format!("{}-", CARGO_PKG_NAME))
            .rand_bytes(6)
            .tempdir()?;
        let named_temp_file = NamedTempFile::new_in(temp_dir.path())?;
        let temp_path = named_temp_file.path();

        // Reopen handle to keep `named_temp_file` (and its path) alive for the final copy.
        let std_file = named_temp_file.reopen()?;
        let mut file = tokio::fs::File::from_std(std_file);

        let mut stream = response.bytes_stream();
        let mut downloaded = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
        }
        file.flush().await?;
        // TODO implement actucal validation
        debug_assert_eq!(downloaded, resource.filesize());

        archive::extract(temp_path, extract_dir)?;
        spinner.finish_and_clear();
        Ok(())
    }
}

mod archive {
    use std::{
        fs::{self, File},
        io,
        path::Path,
    };

    use tracing::{info, instrument};
    use zip::ZipArchive;

    #[derive(Debug, thiserror::Error)]
    pub(super) enum ExtractError {
        #[error(transparent)]
        Zip(#[from] zip::result::ZipError),
        #[error(transparent)]
        Io(#[from] std::io::Error),
    }

    /// Extracts ZIP archive to the specified directory.
    #[instrument]
    pub(super) fn extract(temp_zip: &Path, dest_dir: &Path) -> Result<(), ExtractError> {
        info!("extracting ZIP archive");
        let file = File::open(temp_zip)?;
        let mut archive = ZipArchive::new(file)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;

            let raw_path = file.mangled_name();
            let mut components = raw_path.components();
            components.next();

            let relative_path = components.as_path();

            if relative_path.as_os_str().is_empty() {
                continue;
            }

            let outpath = dest_dir.join(relative_path);

            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent()
                    && !p.exists()
                {
                    fs::create_dir_all(p)?;
                }
                let mut outfile = File::create(&outpath)?;
                io::copy(&mut file, &mut outfile)?;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use std::{
            fs::{self, File},
            io::Write,
        };

        use tempfile::tempdir;
        use zip::write::SimpleFileOptions;

        use super::*;

        #[test]
        fn test_extract_zip_archive_strips_root() -> anyhow::Result<()> {
            let tmp_dir = tempdir()?;
            let zip_path = tmp_dir.path().join("test.zip");
            let dest_dir = tmp_dir.path().join("dest");
            fs::create_dir(&dest_dir)?;

            {
                let file = File::create(&zip_path)?;
                let mut zip = zip::ZipWriter::new(file);
                let options =
                    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

                // main/
                // ├── root_file.txt
                // └── subdir/
                //     └── inner_file.txt

                zip.add_directory("main/", options)?;

                zip.start_file("main/root_file.txt", options)?;
                zip.write_all(b"root content")?;

                zip.add_directory("main/subdir/", options)?;

                zip.start_file("main/subdir/inner_file.txt", options)?;
                zip.write_all(b"inner content")?;

                zip.finish()?;
            }

            extract(&zip_path, &dest_dir).expect("Extraction failed");

            let extracted_root_file = dest_dir.join("root_file.txt");
            assert!(
                extracted_root_file.exists(),
                "root_file.txt should exist in dest root"
            );
            assert_eq!(fs::read_to_string(extracted_root_file)?, "root content");

            let extracted_inner_file = dest_dir.join("subdir/inner_file.txt");
            assert!(
                extracted_inner_file.exists(),
                "subdir/inner_file.txt should exist and keep its structure"
            );
            assert_eq!(fs::read_to_string(extracted_inner_file)?, "inner content");

            assert!(
                !dest_dir.join("main").exists(),
                "The 'main' directory should not exist in dest"
            );

            Ok(())
        }
    }
}
