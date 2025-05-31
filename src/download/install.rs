use std::{collections::HashSet, path::Path, time::Duration};

use futures_util::{StreamExt, stream::FuturesUnordered};
use indicatif::{MultiProgress, ProgressBar};
use reqwest::{Client, Url};

use crate::{
    dependency::{DependencyGraph, ModDependencyQuery},
    download,
    error::Error,
    mod_registry::RemoteModRegistry,
};

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

/// Newly implemented install function to download all mods including dependencies at once.
pub async fn install_mod(
    mod_name: &str,
    mod_registry: &RemoteModRegistry,
    dependency_graph: &DependencyGraph,
    installed_mod_names: &HashSet<String>,
    mods_directory: &Path,
) -> anyhow::Result<()> {
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
    let style = super::pb_style::new();

    // Collects metadata of the mods for the downloads
    let tasks = missing_deps
        .iter()
        .filter_map(|k| mod_registry.get(*k).map(|v| (*k, v)))
        .map(|(name, info)| {
            let client = client.clone();
            let mp = mp.clone();
            let style = style.clone();

            async move {
                let pb = mp.add(ProgressBar::new(info.file_size));
                pb.set_style(style);

                let msg = super::pb_style::truncate_msg(name);
                pb.set_message(msg.to_string());

                if let Err(e) = download::download_mod(
                    &client,
                    name,
                    &info.download_url,
                    &info.checksums,
                    mods_directory,
                    &pb,
                )
                .await
                {
                    eprintln!("Error downloading {}: {}", name, e);
                }
            }
        })
        .collect::<FuturesUnordered<_>>();

    // NOTE: I don't know what the `fut: ()` can do within the async block.
    tasks
        .for_each_concurrent(Some(6), |fut| async move { fut })
        .await;

    Ok(())
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
