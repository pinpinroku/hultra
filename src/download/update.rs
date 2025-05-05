use indicatif::{MultiProgress, ProgressBar};
use reqwest::Client;
use std::{path::Path, sync::Arc};
use tokio::sync::Semaphore;
use tracing::{debug, error, info};

use crate::{
    download, error::Error, fileutil::replace_home_dir_with_tilde,
    installed_mods::AvailableUpdateInfo,
};

/// Update a mod
async fn update(
    client: &Client,
    update_info: &AvailableUpdateInfo,
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
            replace_home_dir_with_tilde(&update_info.existing_path)
        );
    }

    debug!(
        "Updated {} to version {}\n",
        update_info.name, update_info.available_version
    );

    Ok(())
}

/// Update multiple mods concurrently
pub async fn update_multiple_mods(
    client: &Client,
    download_dir: &Path,
    updates: Vec<AvailableUpdateInfo>,
) -> Result<(), Error> {
    let mp = MultiProgress::new();
    let style = super::pb_style::new();

    let semaphore = Arc::new(Semaphore::new(6));
    let mut handles = Vec::new();

    for update_info in updates {
        let semaphore = Arc::clone(&semaphore);

        let mp = mp.clone();

        let client = client.clone();
        let download_dir = download_dir.to_path_buf();
        let style = style.clone();

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await?;

            let total_size = super::get_file_size(&client, &update_info.url).await?;
            let pb = mp.add(ProgressBar::new(total_size));
            pb.set_style(style);

            let msg = super::pb_style::truncate_msg(&update_info.name);
            pb.set_message(msg.to_string());

            update(&client, &update_info, &download_dir, &pb).await?;

            drop(_permit);

            Ok(())
        });

        handles.push(handle);
    }

    // Collect all errors instead of stopping at the first one
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
