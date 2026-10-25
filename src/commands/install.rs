//! Handle install command.
use std::collections::HashSet;

use clap::Args;
use tracing::info;

use crate::{
    config::AppConfig,
    core::{
        local,
        network::{SharedHttpClient, api, downloader},
    },
};

use super::DownloadOption;

#[derive(Debug, Args, Clone)]
pub struct InstallArgs {
    /// Download URLs of Celeste mods.
    #[arg(required = true, value_parser = parse_url_to_id, num_args = 1..20)]
    urls: Vec<u32>,

    #[command(flatten)]
    pub option: DownloadOption,
}

/// Custom error type for URL parsing and ID extraction using `thiserror`
#[derive(thiserror::Error, Debug)]
pub enum UrlParseError {
    #[error("'{0}' is not a valid URL (must start with https://)")]
    InvalidScheme(String),

    #[error("'{0}' must end with .zip")]
    MissingZipExtension(String),

    #[error("'{0}' is missing the required '#FileInfo_{{id}}' fragment")]
    MissingFragment(String),

    #[error("'{0}' has an invalid fragment format (expected #FileInfo_<id>)")]
    InvalidFragmentFormat(String),

    #[error(
        "'{0}' is not an allowed URL pattern.\n\
         Allowed patterns:\n\
         - https://celestemodupdater.0x0a.de/banana-mirror/<id>.zip\n\
         - https://celeste.weg.fan/api/v2/download/gamebanana-files/<id>\n\
         - https://banana-mirror-mods.celestemods.com/<id>.zip\n\
         - https://library.risingsunlight.dev/celeste/mods/<id>.zip\n\
         - https://gamebanana.com/mmdl/<id>\n\
         - https://gamebanana.com/mods/download/<gbid>#FileInfo_<id>"
    )]
    UnknownPattern(String),

    #[error("'{0}' is not a valid u32 integer (got '{1}')")]
    InvalidInteger(String, String),
}

/// Parses the URL, validates it against the 6 allowed patterns, and extracts the u32 ID.
fn parse_url_to_id(s: &str) -> Result<u32, UrlParseError> {
    // For the last pattern, handle the URL fragment (e.g., #FileInfo_12345)
    let mut parts = s.split('#');
    let base_url = parts.next().unwrap_or(s);
    let fragment = parts.next();

    // Check if the base URL starts with https://
    if !base_url.starts_with("https://") {
        return Err(UrlParseError::InvalidScheme(s.to_string()));
    }

    // Strip the "https://" prefix for clean slash-segment matching
    let path_part = &base_url["https://".len()..];
    let segments: Vec<&str> = path_part.split('/').collect();

    // Match against the allowed URL patterns and extract the ID string
    let id_str = match segments.as_slice() {
        // Pattern 1: celestemodupdater.0x0a.de/banana-mirror/{id}.zip
        ["celestemodupdater.0x0a.de", "banana-mirror", file] => file
            .strip_suffix(".zip")
            .ok_or_else(|| UrlParseError::MissingZipExtension(s.to_string()))?,

        // Pattern 2: celeste.weg.fan/api/v2/download/gamebanana-files/{id}
        [
            "celeste.weg.fan",
            "api",
            "v2",
            "download",
            "gamebanana-files",
            id,
        ] => id,

        // Pattern 3: banana-mirror-mods.celestemods.com/{id}.zip
        ["banana-mirror-mods.celestemods.com", file] => file
            .strip_suffix(".zip")
            .ok_or_else(|| UrlParseError::MissingZipExtension(s.to_string()))?,

        // Pattern 4: library.risingsunlight.dev/celeste/mods/{id}.zip
        ["library.risingsunlight.dev", "celeste", "mods", file] => file
            .strip_suffix(".zip")
            .ok_or_else(|| UrlParseError::MissingZipExtension(s.to_string()))?,

        // Pattern 5: gamebanana.com/mmdl/{id}
        ["gamebanana.com", "mmdl", id] => id,

        // Pattern 6: gamebanana.com/mods/download/{gbid}#FileInfo_{id}
        ["gamebanana.com", "mods", "download", _gbid] => {
            let frag = fragment.ok_or_else(|| UrlParseError::MissingFragment(s.to_string()))?;

            frag.strip_prefix("FileInfo_")
                .ok_or_else(|| UrlParseError::InvalidFragmentFormat(s.to_string()))?
        }

        // None of the patterns matched
        _ => return Err(UrlParseError::UnknownPattern(s.to_string())),
    };

    // Parse the extracted ID string into a u32 integer
    id_str
        .parse::<u32>()
        .map_err(|_| UrlParseError::InvalidInteger(s.to_string(), id_str.to_string()))
}

pub async fn run(args: InstallArgs, config: &AppConfig) -> anyhow::Result<()> {
    // Initialize client
    let shared_client = SharedHttpClient::new();

    // Covert URLs into IDs
    let ids: HashSet<u32> = args.urls.into_iter().collect();

    info!("fetching databases");
    let (registry, graph) = api::fetch(shared_client.inner().clone(), &args.option).await?;

    info!("scanning installed mods");
    let installed_names: HashSet<String> = local::scan_mods(&config.mods_dir())?
        .iter()
        .map(|m| m.name().to_string())
        .collect();

    // Resolve missing deps
    info!("resolving missing dependencies");
    let targets = graph.resolve_missing_mods(&ids, &registry, &installed_names);

    if targets.is_empty() {
        println!("You have already installed the mod and its dependencies");
        return Ok(());
    }

    // Convert targets into tasks
    let tasks = registry.into_download_files(targets, installed_names)?;

    // Download all mods
    info!("downloading mods");
    downloader::download_all(
        shared_client.inner().clone(),
        args.option,
        tasks,
        &config.mods_dir(),
    )
    .await?;

    info!("installation completed");
    Ok(())
}
