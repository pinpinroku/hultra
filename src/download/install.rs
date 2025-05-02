use reqwest::Client;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};
use tracing::{debug, error, info, warn};

use crate::{
    download,
    error::Error,
    fileutil::{read_manifest_file_from_zip, replace_home_dir_with_tilde},
    installed_mods::ModManifest,
    mod_registry::{ModRegistryQuery, RemoteModInfo, RemoteModRegistry},
};

/// Install a mod
pub async fn install(
    client: &Client,
    (name, manifest): (&str, &RemoteModInfo),
    mod_registry: RemoteModRegistry,
    download_dir: &Path,
    installed_mod_names: HashSet<String>,
) -> Result<(), Error> {
    let download_path = download::download_mod(
        client,
        name,
        &manifest.download_url,
        &manifest.checksums,
        download_dir,
    )
    .await?;

    info!(
        "[{}] is now installed in {}.",
        name,
        replace_home_dir_with_tilde(&download_path)
    );

    if let Some(dependencies) = check_dependencies(&download_path)? {
        debug!("Filetering out already installed dependencies.");
        let missing_dependencies: Vec<_> = dependencies.difference(&installed_mod_names).collect();
        if missing_dependencies.is_empty() {
            info!("You already have all the dependencies required by this mod.");
            return Ok(());
        }

        info!("Start downloading the dependencies...");
        resolve_dependencies(client, mod_registry, download_dir, missing_dependencies).await?;
    }

    Ok(())
}

/// Download all of missing dependencies concurrently
async fn resolve_dependencies(
    client: &Client,
    mod_registry: HashMap<String, RemoteModInfo>,
    download_dir: &Path,
    missing_dependencies: Vec<&String>,
) -> Result<(), Error> {
    let mut handles = Vec::with_capacity(missing_dependencies.len());

    for dependency in missing_dependencies {
        if let Some((mod_name, manifest)) = mod_registry.get_mod_info_by_name(dependency) {
            let mod_name = mod_name.clone();
            let manifest = manifest.clone();
            let client = client.clone();
            let download_dir = download_dir.to_path_buf();
            debug!("Manifest of dependency: {}\n{:#?}", mod_name, manifest);

            let handle = tokio::spawn(async move {
                download::download_mod(
                    &client,
                    &mod_name,
                    &manifest.download_url,
                    &manifest.checksums,
                    &download_dir,
                )
                .await
            });

            handles.push(handle);
        } else {
            warn!(
                "Could not find information about the mod '{}'.\n\
                    The modder might have misspelled the name.",
                dependency
            );
        }
    }

    // Collect all errors instead of stopping at the first one
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
fn check_dependencies(download_path: &Path) -> Result<Option<HashSet<String>>, Error> {
    info!("Checking for missing dependencies...");
    // Attempt to read the manifest file. If it doesn't exist, return an error.
    let buffer = read_manifest_file_from_zip(download_path)?
        .ok_or_else(|| Error::MissingManifestFile(download_path.to_path_buf()))?;

    // Parse the manifest file
    let manifest = ModManifest::parse_mod_manifest_from_yaml(&buffer)?;
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
