use std::path::Path;

use reqwest::Client;
use tracing::info;

use crate::{download, error::Error, mod_registry::RemoteModInfo};

/// MODの新規インストールを行う
pub async fn install(
    client: &Client,
    (name, manifest): (&str, &RemoteModInfo),
    download_dir: &Path,
) -> Result<(), Error> {
    download::download_mod(
        client,
        name,
        &manifest.download_url,
        &manifest.checksums,
        download_dir,
    )
    .await?;

    info!("[{}] installation complete.", name);

    Ok(())
}
