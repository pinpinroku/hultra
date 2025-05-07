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
    local::{Generatable, LocalMod},
    mod_registry::{ModRegistryQuery, RemoteModRegistry},
};

/// Update information about the mod
#[derive(Debug, Clone)]
pub struct AvailableUpdate {
    /// The Mod name
    name: String,
    /// Download URL of the Mod
    url: String,
    /// xxHashes of the file
    hashes: Vec<String>,
    /// Path to the current version of the mod
    existing_path: PathBuf,
}

/// Checks for an available update for a mod.
///
/// This function compares the local mod's checksum with the checksum provided by the remote mod registry.
/// If the checksums differ, it indicates that an update is available. If the remote registry does not contain information
/// for the mod, or the checksums match, no update is reported.
///
/// # Arguments
/// * `local_mod` - Information about the locally installed mod, including its current version and checksum.
/// * `mod_registry` - An atomic reference counter of the mod registry that holds remote mod information.
///
/// # Returns
/// * `Ok(Some(AvailableUpdate))` if an update is available, containing update details.
/// * `Ok(None)` if no update is available (either because the mod is up-to-date or remote info is missing).
/// * `Err(Error)` if there is an error computing the mod's checksum (originally the error caused by opening or reading file).
async fn check_update(
    local_mod: LocalMod,
    mod_registry: Arc<RemoteModRegistry>,
) -> Result<Option<AvailableUpdate>, Error> {
    let manifest = local_mod.manifest();

    // Look up remote mod info
    let Some(remote_mod) = mod_registry.get_mod_info_by_name(&manifest.name) else {
        tracing::debug!(
            "No remote info for mod: {}, skipping update check",
            manifest.name
        );
        return Ok(None);
    };

    // Compute checksum - only if needed
    let computed_hash = local_mod.checksum().await?;

    // Skip if hash matches (no update needed)
    if remote_mod.has_matching_hash(computed_hash) {
        return Ok(None);
    }

    tracing::info!(
        "Update available for '{}': {} -> {}",
        manifest.name,
        manifest.version,
        remote_mod.version
    );

    // Update is available
    Ok(Some(AvailableUpdate {
        name: manifest.name.to_string(),
        url: remote_mod.download_url.clone(),
        hashes: remote_mod.checksums.clone(),
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
pub async fn check_updates(
    installed_mods: Vec<LocalMod>,
    mod_registry: RemoteModRegistry,
) -> Result<Vec<AvailableUpdate>, Error> {
    let start = std::time::Instant::now();
    tracing::info!("Starting update check for {} mods", installed_mods.len());

    let mod_registry = Arc::new(mod_registry);
    let semaphore = Arc::new(Semaphore::new(64)); // Optimal limits for modern linux system

    let mut handles = Vec::with_capacity(installed_mods.len());
    for local_mod in installed_mods {
        let semaphore = Arc::clone(&semaphore);
        let mod_registry = Arc::clone(&mod_registry);
        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await?;
            let result = check_update(local_mod, mod_registry).await?;
            drop(_permit);
            Ok(result)
        });
        handles.push(handle);
    }

    let mut updates = Vec::with_capacity(handles.len());
    let mut errors = Vec::new();

    for handle in handles {
        match handle.await {
            Ok(Ok(available_or_not)) => {
                if let Some(available) = available_or_not {
                    updates.push(available)
                }
            } // Task completed successfully
            Ok(Err(err)) => errors.push(err), // Task returned an error
            Err(err) => errors.push(Error::TaskJoin(err)), // Task panicked
        }
    }

    if errors.is_empty() {
        tracing::info!(
            "Completed update check in {:?}. Found {} updates.",
            start.elapsed(),
            updates.len()
        );
    } else {
        // Log all errors
        for (i, err) in errors.iter().enumerate() {
            error!("Error {}: {}", i + 1, err);
        }
        // Return update check errors
        return Err(Error::UpdateCheck(errors));
    }

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
