use std::{collections::BTreeMap, fmt::Display};

use tracing::{instrument, warn};

use crate::{
    cache::CacheEntry,
    core::{
        LocalMod,
        mod_file::ModIdentityService,
        network::downloader::{ChecksumError, ChecksumVerifier, Checksums, DownloadTask},
        registry::EverestUpdateYaml,
    },
    service::os::LocalFileSystemService,
};

pub struct UpdateTask {
    /// Key of HashMap
    pub name: String, // used for UpdateInfo

    /// Used for DownloadTask
    pub version: String,
    pub url: String,
    pub size: u64,
    pub checksums: Checksums,
}

/// Result of scanning mods for update.
pub struct UpdateReport {
    /// Tasks to download mods.
    pub download_tasks: Vec<DownloadTask>,
    /// A list of mod information to display.
    pub updates: Vec<UpdateInfo>,
}

/// Mod scanner for update.
pub struct UpdateScanner {
    /// Represents cache for file hash.
    cache_db: BTreeMap<u64, CacheEntry>,
    /// Database of all mods.
    registry: EverestUpdateYaml,
}

impl UpdateScanner {
    pub fn new(cache_db: BTreeMap<u64, CacheEntry>, registry: EverestUpdateYaml) -> Self {
        Self { cache_db, registry }
    }

    /// Identifies required updates by comparing local mods with the remote registry.
    #[instrument(skip_all)]
    pub fn scan(mut self, local_mods: &[LocalMod]) -> Result<UpdateReport, ChecksumError> {
        let mut available_mods = Vec::with_capacity(local_mods.len());
        let mut available_info = Vec::with_capacity(local_mods.len());
        let service = LocalFileSystemService;

        for local_mod in local_mods {
            // verify if the local mod exist in the remote registry
            let Some(result) = self.registry.create_update_task(local_mod.name()) else {
                continue;
            };

            let Ok(task) = result else {
                continue;
            };

            // attempts to retrieve the mod's inode
            let Ok(inode) = service.fetch_id(local_mod.file().path()) else {
                continue;
            };

            // check if an update is required
            let is_update_needed = self
                .cache_db
                .get(&inode)
                .map(|entry| task.checksums.verify(entry.hash()).is_ok())
                .unwrap_or(false);

            // extract the metadata from the remote registry if an update is required
            if is_update_needed {
                let update_info = UpdateInfo::new(&task.name, local_mod.version(), &task.version);
                let download_task = DownloadTask::from(task);

                available_info.push(update_info);
                available_mods.push(download_task);
            }
        }

        Ok(UpdateReport {
            download_tasks: available_mods,
            updates: available_info,
        })
    }
}

#[derive(Debug)]
pub struct UpdateInfo {
    name: String,
    current_version: String,
    available_version: String,
}

impl UpdateInfo {
    fn new(name: &str, current_v: &str, available_v: &str) -> Self {
        Self {
            name: name.to_string(),
            current_version: current_v.to_string(),
            available_version: available_v.to_string(),
        }
    }
}

impl Display for UpdateInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "* {}: {} -> {}",
            self.name, self.current_version, self.available_version
        )
    }
}
