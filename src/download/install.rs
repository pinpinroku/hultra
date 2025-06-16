use std::{collections::HashSet, sync::Arc, time::Duration};

use indicatif::{MultiProgress, ProgressBar};
use reqwest::{Client, Url};
use tokio::sync::Semaphore;

use crate::{
    config::Config,
    dependency::{DependencyGraph, ModDependencyQuery},
    download,
    error::Error,
    mod_registry::RemoteModRegistry,
};

/// Downloads all mods with dependencies if any of them are missing.
///
/// # Errors
/// Returns an error if any of the downloads fail or if there are issues with the tasks.
pub async fn install_mod(
    mod_name: &str,
    mod_registry: &RemoteModRegistry,
    dependency_graph: &DependencyGraph,
    installed_mod_names: &HashSet<String>,
    config: Arc<Config>,
) -> anyhow::Result<()> {
    const CONCURRENT_LIMIT: usize = 6;
    let semaphore = Arc::new(Semaphore::new(CONCURRENT_LIMIT));

    // Collects required dependencies for the mod including the mod itself
    let dependencies = dependency_graph.collect_all_dependencies_bfs(mod_name);

    // Filters out missing dependencies
    let missing_deps = dependencies
        .difference(installed_mod_names)
        .collect::<Vec<_>>();
    tracing::debug!("Missing dependencies are found: {:?}", missing_deps);

    tracing::info!("Start installing the new mod [{}]", mod_name);
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .build()?;
    let mp = MultiProgress::new();

    let mut handles = Vec::with_capacity(missing_deps.len());

    for name in missing_deps {
        if let Some(remote_mod) = mod_registry.get(name) {
            let name = name.to_owned();
            let remote_mod = remote_mod.to_owned();

            let semaphore = semaphore.clone();
            let client = client.clone();
            let config = config.clone();

            let mp = mp.clone();

            let handle = tokio::spawn(async move {
                let _permit = semaphore.acquire().await?;

                let pb = mp.add(ProgressBar::new(remote_mod.file_size));
                pb.set_style(super::pb_style::new());

                let msg = super::pb_style::truncate_msg(&name);
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
            handles.push(handle)
        };
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

/// Parses the given url string, converts it to the mod ID.
pub fn parse_mod_page_url(page_url_str: &str) -> Result<u32, Error> {
    let page_url =
        Url::parse(page_url_str).map_err(|_| Error::InvalidUrl(page_url_str.to_owned()))?;

    // Check scheme
    match page_url.scheme() {
        "http" | "https" => {}
        _ => return Err(Error::UnsupportedScheme(page_url_str.to_owned())),
    }

    // Check host
    if page_url.host_str() != Some("gamebanana.com") {
        return Err(Error::InvalidGameBananaUrl(page_url.clone()));
    }

    // Check path segments
    let mut segments = page_url
        .path_segments()
        .ok_or_else(|| Error::InvalidGameBananaUrl(page_url.clone()))?;

    // Expected path: /mods/12345
    match (segments.next(), segments.next()) {
        (Some("mods"), Some(id_str)) => {
            let id = id_str
                .parse::<u32>()
                .map_err(|_| Error::InvalidModId(id_str.to_owned()))?;
            Ok(id)
        }
        _ => Err(Error::InvalidGameBananaUrl(page_url.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_url() {
        let url = "https://gamebanana.com/mods/12345";
        assert_eq!(parse_mod_page_url(url).unwrap(), 12345);
    }

    #[test]
    fn test_invalid_scheme() {
        let url = "ftp://gamebanana.com/mods/12345";
        assert!(parse_mod_page_url(url).is_err());
    }

    #[test]
    fn test_invalid_host() {
        let url = "https://example.com/mods/12345";
        assert!(parse_mod_page_url(url).is_err());
    }

    #[test]
    fn test_missing_id() {
        let url = "https://gamebanana.com/mods/";
        assert!(parse_mod_page_url(url).is_err());
    }

    #[test]
    fn test_non_numeric_id() {
        let url = "https://gamebanana.com/mods/abc";
        assert!(parse_mod_page_url(url).is_err());
    }
}
