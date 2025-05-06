use indicatif::{MultiProgress, ProgressBar};
use reqwest::{Client, Url};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
};
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

use crate::{
    download,
    error::Error,
    fileutil,
    local::ModManifest,
    mod_registry::{ModRegistryQuery, RemoteModInfo, RemoteModRegistry},
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

/// Installs a mod and checks for the missing dependencies, if missing install them.
pub async fn install(
    client: &Client,
    (name, manifest): (&str, &RemoteModInfo),
    mod_registry: &RemoteModRegistry,
    download_dir: &Path,
    installed_mod_names: HashSet<String>,
    pb: &ProgressBar,
) -> Result<(), Error> {
    let style = super::pb_style::new();
    pb.set_style(style);

    let msg = super::pb_style::truncate_msg(name);
    pb.set_message(msg.into_owned());

    let downloaded_file_path = download::download_mod(
        client,
        name,
        &manifest.download_url,
        &manifest.checksums,
        download_dir,
        pb,
    )
    .await?;

    debug!(
        "[{}] is now installed in {}.",
        name,
        fileutil::replace_home_dir_with_tilde(&downloaded_file_path)
    );

    if let Some(dependencies) = check_dependencies(&downloaded_file_path)? {
        debug!("Filetering out already installed dependencies.");
        let missing_dependencies: Vec<_> = dependencies.difference(&installed_mod_names).collect();
        if missing_dependencies.is_empty() {
            info!("You already have all the dependencies required by this mod.");
            return Ok(());
        }

        info!("Start downloading the dependencies...\n");
        resolve_dependencies(client, mod_registry, download_dir, missing_dependencies).await?;
    }

    Ok(())
}

/// Downloads all missing dependencies concurrently.
async fn resolve_dependencies(
    client: &Client,
    mod_registry: &HashMap<String, RemoteModInfo>,
    download_dir: &Path,
    missing_dependency_names: Vec<&String>,
) -> Result<(), Error> {
    let mp = MultiProgress::new();
    let style = super::pb_style::new();

    let semaphore = Arc::new(Semaphore::new(6));
    let mut handles = Vec::with_capacity(missing_dependency_names.len());

    for dependency_name in missing_dependency_names {
        if let Some(manifest) = mod_registry.get_mod_info_by_name(dependency_name) {
            let semaphore = Arc::clone(&semaphore);

            let mp = mp.clone();
            let style = style.clone();

            let dependency_name = dependency_name.clone();
            let manifest = manifest.clone();
            let client = client.clone();
            let download_dir = download_dir.to_path_buf();
            debug!(
                "Manifest of dependency: {}\n{:#?}",
                dependency_name, manifest
            );

            let handle = tokio::spawn(async move {
                let _permit = semaphore.acquire().await?;

                let total_size = super::get_file_size(&client, &manifest.download_url).await?;
                let pb = mp.add(ProgressBar::new(total_size));
                pb.set_style(style);

                let msg = super::pb_style::truncate_msg(&dependency_name);
                pb.set_message(msg.to_string());

                let downloaded_file_path = download::download_mod(
                    &client,
                    &dependency_name,
                    &manifest.download_url,
                    &manifest.checksums,
                    &download_dir,
                    &pb,
                )
                .await?;

                drop(_permit);

                Ok(downloaded_file_path)
            });

            handles.push(handle);
        } else {
            warn!(
                "Could not find information about the mod '{}'.\n\
                    The modder may have misspelled the name.",
                dependency_name
            );
        }
    }

    // Collects all errors instead of stopping at the first one.
    let mut errors = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => (),                               // Task completed successfully
            Ok(Err(err)) => errors.push(err),              // Task returned an error
            Err(err) => errors.push(Error::TaskJoin(err)), // Task panicked
        }
    }

    // Actually handle the errors
    if errors.is_empty() {
        info!("All required dependencies installed successfully!");
    } else {
        // Log all errors
        for (i, err) in errors.iter().enumerate() {
            error!("Error {}: {}", i + 1, err);
        }
        // Return multiple update errors
        return Err(Error::MultipleUpdate(errors));
    }

    Ok(())
}

/// Check for dependencies, if found return `HashSet<String>`, otherwise return `None`.
fn check_dependencies(downloaded_file_path: &Path) -> Result<Option<HashSet<String>>, Error> {
    info!("\nChecking for missing dependencies...");
    // Attempt to read the manifest file. If it doesn't exist, return an error.
    let buffer = fileutil::read_manifest_file_from_archive(downloaded_file_path)?;

    // Parse the manifest file
    let manifest = ModManifest::from_yaml(&buffer)?;
    debug!("Manifest content: {:#?}", manifest);

    // Retrieve dependencies if available, filtering out "Everest" and "EverestCore"
    if let Some(dependencies) = manifest.dependencies {
        let filtered_deps = dependencies
            .into_iter()
            // NOTE: "Everest" and "EverestCore (deprecated)" are primal dependencies, so there is no need to download them
            .filter(|dependency| !matches!(dependency.name.as_str(), "Everest" | "EverestCore"))
            .map(|dependency| dependency.name)
            .collect::<HashSet<String>>();
        debug!("Filtered dependencies: {:#?}", filtered_deps);
        Ok(Some(filtered_deps))
    } else {
        warn!("No dependencies found. This is weird. Even 'Everest' is not listed.");
        Ok(None)
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
