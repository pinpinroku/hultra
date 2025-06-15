use std::sync::Arc;

use anyhow::Result;
use indicatif::{MultiProgress, ProgressBar};
use reqwest::Client;
use tokio::sync::Semaphore;

use crate::{
    config::Config,
    download,
    local::{Generatable, LocalMod},
    mod_registry::{RemoteModInfo, RemoteModRegistry},
};

pub async fn check_updates(
    local_mods: &[LocalMod],
    mod_registry: Arc<RemoteModRegistry>,
    semaphore: Arc<Semaphore>,
) -> Result<Vec<(String, RemoteModInfo)>> {
    let tasks: Vec<_> = local_mods
        .iter()
        .map(|local_mod| {
            let registry = mod_registry.clone();
            let local_mod = local_mod.clone();
            let semaphore = semaphore.clone();

            tokio::spawn(async move {
                let _permit = semaphore.acquire().await?;

                let name = &local_mod.manifest.name;
                let Some((name, remote_mod)) = registry.get_key_value(name) else {
                    return Ok(None); // Local-only mod, ignore.
                };

                let computed_hash = local_mod.checksum().await?;
                if remote_mod.has_matching_hash(computed_hash) {
                    Ok(None) // No update needed.
                } else {
                    println!(
                        "Update available for '{}': {} -> {}",
                        name, local_mod.manifest.version, remote_mod.version
                    );
                    Ok(Some((name.clone(), remote_mod.clone())))
                }
            })
        })
        .collect();

    let mut updates = Vec::new();
    let mut errors = Vec::new();

    for task in tasks {
        match task.await {
            Ok(Ok(Some(update))) => updates.push(update),
            Ok(Ok(None)) => { /* No update needed */ }
            Ok(Err(e)) => errors.push(e),
            Err(e) => errors.push(anyhow::anyhow!(e)),
        }
    }

    if errors.is_empty() {
        tracing::info!("Completed update check. Found {} updates.", updates.len());
    } else {
        for (i, err) in errors.iter().enumerate() {
            tracing::error!("Error {}: {}", i + 1, err);
        }
        anyhow::bail!("Failed to check updates: {:?}", errors)
    }

    Ok(updates)
}

pub async fn install_updates(
    client: &Client,
    config: Arc<Config>,
    available_updates: &[(String, RemoteModInfo)],
) -> Result<()> {
    const CONCURRENT_LIMIT: usize = 6;
    let semaphore = Arc::new(Semaphore::new(CONCURRENT_LIMIT));
    let mp = MultiProgress::new();

    let mut handles = Vec::with_capacity(available_updates.len());

    for (name, remote_mod) in available_updates {
        let name = name.to_owned();
        let remote_mod = remote_mod.clone();

        let semaphore = semaphore.clone();
        let config = config.clone();
        let client = client.clone();

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
        handles.push(handle);
    }

    let mut errors = Vec::with_capacity(available_updates.len());

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
