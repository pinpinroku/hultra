use reqwest::Client;
use std::path::Path;
use tracing::{error, info};

use crate::{download, error::Error, installed_mods::AvailableUpdateInfo};

/// MODの更新を行う
pub async fn update(
    client: &Client,
    update_info: &AvailableUpdateInfo,
    download_dir: &Path,
) -> Result<(), Error> {
    download::download_mod(
        client,
        &update_info.name,
        &update_info.url,
        &update_info.hash,
        download_dir,
    )
    .await?;

    // ファイルダウンロードに成功したら既存ファイルを削除する
    if update_info.existing_path.exists() {
        tokio::fs::remove_file(&update_info.existing_path).await?;
    }

    // その他のアップデート固有の処理を行う
    info!(
        "Updated {} to version {}\n",
        update_info.name, update_info.available_version
    );

    Ok(())
}

/// 全てのMODをまとめて更新する
pub async fn download_all(
    client: &Client,
    download_dir: &Path,
    updates: Vec<AvailableUpdateInfo>,
) -> Result<(), Error> {
    let mut handles = Vec::new();

    for update_info in updates {
        let client = client.clone();
        let download_dir = download_dir.to_path_buf();

        let handle =
            tokio::spawn(async move { update(&client, &update_info, &download_dir).await });

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
