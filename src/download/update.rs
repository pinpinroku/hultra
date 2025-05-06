use indicatif::{MultiProgress, ProgressBar};
use reqwest::Client;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Semaphore;
use tracing::{debug, error, info};

use crate::{
    download,
    error::Error,
    fileutil,
    local::Generatable,
    mod_registry::{ModRegistryQuery, RemoteModRegistry},
};

/// Update information about the mod
#[derive(Debug, Clone)]
pub struct AvailableUpdate {
    /// The Mod name
    pub name: String,
    /// Current version (from LocalMod)
    pub current_version: String,
    /// Available version (from RemoteMod)
    pub available_version: String,
    /// Download URL of the Mod
    pub url: String,
    /// xxHashes of the file
    pub hashes: Vec<String>,
    /// Path to the current version of the mod
    pub existing_path: PathBuf,
}

/// Checks for an available update for a mod.
///
/// This function compares the local mod's checksum with the checksum provided by the remote mod registry.
/// If the checksums differ, it indicates that an update is available. If the remote registry does not contain information
/// for the mod, or the checksums match, no update is reported.
///
/// # Arguments
/// * `local_mod` - Information about the locally installed mod, including its current version and checksum.
/// * `mod_registry` - A reference to the mod registry that holds remote mod information.
///
/// # Returns
/// * `Ok(Some(AvailableUpdate))` if an update is available, containing update details.
/// * `Ok(None)` if no update is available (either because the mod is up-to-date or remote info is missing).
/// * `Err(Error)` if there is an error computing the mod's checksum.
fn check_update<G: Generatable>(
    mut local_mod: G,
    mod_registry: &RemoteModRegistry,
) -> Result<Option<AvailableUpdate>, Error> {
    // Look up remote mod info
    let manifest = local_mod.manifest();
    let remote_mod = match mod_registry.get_mod_info_by_name(&manifest.name) {
        Some(info) => info,
        None => return Ok(None), // No remote info, skip update check.
    };

    // Compute checksum
    let computed_hash = local_mod.checksum()?;

    // Continue only if the hash doesn't match
    if remote_mod.has_matching_hash(computed_hash) {
        return Ok(None);
    }

    let remote_mod = remote_mod.clone();
    let manifest = local_mod.manifest();

    Ok(Some(AvailableUpdate {
        name: manifest.name.to_string(),
        current_version: manifest.version.to_string(),
        available_version: remote_mod.version,
        url: remote_mod.download_url,
        hashes: remote_mod.checksums,
        existing_path: local_mod.file_path().to_path_buf(),
    }))
}

/// Check available updates for all installed mods.
///
/// # Arguments
/// * `installed_mods` - A list of information about installed mods.
/// * `mod_registry` - Registry containing remote mod information.
///
/// # Returns
/// * `Ok(Vec<AvailableUpdateInfo>)` - List of available updates for mods.
/// * `Err(Error)` - If there are issues fetching or computing update information.
pub fn check_updates<G: Generatable>(
    installed_mods: Vec<G>,
    mod_registry: &RemoteModRegistry,
) -> Result<Vec<AvailableUpdate>, Error> {
    // Use iterator combinators to process each mod gracefully.
    let updates = installed_mods
        .into_iter()
        .map(|local_mod| check_update(local_mod, mod_registry))
        .collect::<Result<Vec<Option<AvailableUpdate>>, Error>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    Ok(updates)
}

/// Updates a mod, deletes the previous version of the file if the update succeeds.
async fn update(
    client: &Client,
    update_info: &AvailableUpdate,
    download_dir: &Path,
    pb: &ProgressBar,
) -> Result<(), Error> {
    download::download_mod(
        client,
        &update_info.name,
        &update_info.url,
        &update_info.hashes,
        download_dir,
        pb,
    )
    .await?;

    if update_info.existing_path.exists() {
        tokio::fs::remove_file(&update_info.existing_path).await?;
        debug!(
            "🗑️ The previous version has been deleted. {}",
            fileutil::replace_home_dir_with_tilde(&update_info.existing_path)
        );
    }

    debug!(
        "Updated {} to version {}\n",
        update_info.name, update_info.available_version
    );

    Ok(())
}

/// Updates all mods that can be updated concurrently.
pub async fn update_multiple_mods(
    client: &Client,
    download_dir: &Path,
    available_updates: Vec<AvailableUpdate>,
) -> Result<(), Error> {
    let mp = MultiProgress::new();
    let style = super::pb_style::new();

    let semaphore = Arc::new(Semaphore::new(6));
    let mut handles = Vec::new();

    for available_update in available_updates {
        let semaphore = Arc::clone(&semaphore);

        let mp = mp.clone();

        let client = client.clone();
        let download_dir = download_dir.to_path_buf();
        let style = style.clone();

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await?;

            let total_size = super::get_file_size(&client, &available_update.url).await?;
            let pb = mp.add(ProgressBar::new(total_size));
            pb.set_style(style);

            let msg = super::pb_style::truncate_msg(&available_update.name);
            pb.set_message(msg.to_string());

            update(&client, &available_update, &download_dir, &pb).await?;

            drop(_permit);

            Ok(())
        });

        handles.push(handle);
    }

    // Collects all errors instead of stopping at the first one.
    let mut errors = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok(())) => (),                              // Task completed successfully
            Ok(Err(err)) => errors.push(err),              // Task returned an error
            Err(err) => errors.push(Error::TaskJoin(err)), // Task panicked
        }
    }

    // Actually handle the errors
    if errors.is_empty() {
        info!("\nAll updates installed successfully!");
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
