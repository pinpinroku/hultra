use std::{
    collections::{BTreeMap, HashSet},
    ffi::OsString,
    fmt::Display,
    fs::File,
    io::{self, BufRead, BufReader},
    path::Path,
};

use tracing::{instrument, warn};

use crate::{
    cache::CacheEntry,
    core::{
        local::{FileSystemExt, LocalMod},
        network::downloader::DownloadTask,
        registry::{Entry, EverestUpdateYaml},
    },
    utils::{self, ChecksumError},
};

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

        for local_mod in local_mods {
            let name = local_mod.name();

            // verify if the local mod exist in the remote registry
            let Some(remote_mod) = self.registry.get(name) else {
                continue;
            };

            // attempts to retrieve the mod's inode
            let Ok(inode) = local_mod.fetch_inode() else {
                continue;
            };

            let digests = remote_mod
                .checksums
                .iter()
                .map(|s| utils::from_str_digest(s))
                .collect::<Result<Vec<u64>, _>>()?;

            // check if an update is required
            let is_update_needed = self
                .cache_db
                .get(&inode)
                .map(|entry| !digests.contains(entry.hash()))
                .unwrap_or(false);

            // extract the metadata from the remote registry if an update is required
            if is_update_needed && let Some((n, e)) = self.registry.remove_entry(name) {
                let update_info = UpdateInfo::new(&n, local_mod, &e);
                available_info.push(update_info);
                available_mods.push(DownloadTask::try_from((n, e))?);
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
    pub fn new(name: &str, local_mod: &LocalMod, remote_mod: &Entry) -> Self {
        Self {
            name: name.to_string(),
            current_version: local_mod.version().to_string(),
            available_version: remote_mod.version.to_string(),
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

/// Returns blacklisted mods for update.
pub fn fetch_updater_blacklist(mods_dir: &Path) -> io::Result<HashSet<OsString>> {
    let path = mods_dir.join("updaterblacklist.txt");
    let file = match File::open(&path) {
        Ok(f) => f,
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => return Ok(HashSet::new()),
        Err(e) => return Err(e),
    };

    let mut blacklist = HashSet::new();
    // NOTE The default 8KiB buffer is overkill for small text files.
    let reader = BufReader::with_capacity(1024, &file);

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if !line.starts_with('#') && !line.is_empty() {
            warn!("'{}' will be excluded from updates", line);
            blacklist.insert(OsString::from(line));
        }
    }

    Ok(blacklist)
}
