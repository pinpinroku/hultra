use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ffi::OsString,
    fmt::Display,
    fs::File,
    io::{self, BufRead, BufReader},
    path::Path,
};

use tracing::{instrument, warn};

use crate::{
    cache::CacheEntry,
    core::network::downloader::DownloadTask,
    local_mods::{FileSystemExt, LocalMod},
    registry::RemoteMod,
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
    registry: HashMap<String, RemoteMod>,
}

impl UpdateScanner {
    pub fn new(cache_db: BTreeMap<u64, CacheEntry>, registry: HashMap<String, RemoteMod>) -> Self {
        Self { cache_db, registry }
    }

    /// Identifies required updates by comparing local mods with the remote registry.
    #[instrument(skip_all)]
    pub fn scan(mut self, local_mods: &[LocalMod]) -> UpdateReport {
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

            // check if an update is required
            let is_update_needed = self
                .cache_db
                .get(&inode)
                .map(|entry| !remote_mod.checksums.contains(entry.hash()))
                .unwrap_or(false);

            // extract the metadata from the remote registry if an update is required
            if is_update_needed && let Some(reg) = self.registry.remove_entry(name) {
                let update_info = UpdateInfo::from(&reg);
                available_info.push(update_info);
                available_mods.push(DownloadTask::from(&reg));
            }
        }

        UpdateReport {
            download_tasks: available_mods,
            updates: available_info,
        }
    }
}

#[derive(Debug)]
pub struct UpdateInfo {
    name: String,
    current_version: String,
    available_version: String,
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

impl From<&(String, RemoteMod)> for UpdateInfo {
    /// Converts HashMap<String, RemoteMod> into this type.
    fn from((name, remote): &(String, RemoteMod)) -> Self {
        Self {
            name: name.to_string(),
            current_version: remote.version().to_string(),
            available_version: remote.version().to_string(),
        }
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
