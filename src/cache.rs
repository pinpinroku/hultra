use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::{self, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

use rkyv::{Archive, Deserialize, Serialize, deserialize, rancor};
use tracing::{info, instrument};
use xxhash_rust::xxh64::Xxh64;

use crate::config::AppConfig;

#[derive(thiserror::Error, Debug)]
pub enum CacheError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Archive(#[from] rancor::Error),
}

/// Represents database of file cache.
#[derive(Archive, Deserialize, Serialize, Debug, Default)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct FileCacheDB {
    pub entries: BTreeMap<u64, CacheEntry>,
}

/// Snapshot of the file when it was last hashed.
#[derive(Archive, Deserialize, Serialize, Debug)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct CacheEntry {
    file_name: String, // for DEBUG purpose
    mtime: i64,
    size: u64,
    hash: u64, // XXH64
}

impl CacheEntry {
    pub fn new(file_name: &str, mtime: i64, size: u64, hash: u64) -> Self {
        Self {
            file_name: file_name.to_string(),
            mtime,
            size,
            hash,
        }
    }

    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    pub fn hash(&self) -> &u64 {
        &self.hash
    }
}

/// Gets up-to-date file cache.
#[instrument(skip_all)]
pub fn sync(config: &AppConfig) -> Result<BTreeMap<u64, CacheEntry>, CacheError> {
    // Load existing cache database
    let mut cache = load_cache_db(config.cache_db_path()).unwrap_or_default();

    if update_cache(&mut cache, config.mods_dir())? {
        save_cache_db(&cache, config.cache_db_path())?;
    }

    Ok(cache.entries)
}

/// Updates cache entries based on current filesystem state.
fn update_cache(cache: &mut FileCacheDB, mods_dir: &Path) -> io::Result<bool> {
    let mut current_keys = HashSet::new();
    let mut updated = false;

    for entry in (mods_dir.read_dir()?).flatten() {
        // Skip anything that isn’t a regular file *or* isn’t a `.zip`
        if !entry.path().is_file()
            || !entry
                .path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            continue;
        }

        // Get file metadata
        if let Ok(meta) = entry.metadata() {
            let key = meta.ino();
            current_keys.insert(key);

            let (path, mtime, size) = (entry.path(), meta.mtime(), meta.size());

            if should_rehash(&cache.entries, &key, mtime, size) {
                let hash = hash_file(&path)?;

                // NOTE we only need file name since mods directory is fixed
                let file_name = path
                    .file_name()
                    .map(|name| name.to_string_lossy())
                    .unwrap_or_else(|| path.to_string_lossy());

                // Create new cache entry
                let cache_entry = CacheEntry::new(&file_name, mtime, size, hash);
                info!(?cache_entry, "new entry created");
                cache.entries.insert(key, cache_entry);
                updated = true;
            }
        }
    }

    // Remove stale cache entries (files that no longer exist)
    let stale_count = cache.entries.len();
    cache.entries.retain(|key, _| current_keys.contains(key));
    updated |= cache.entries.len() != stale_count;

    Ok(updated)
}

/// Checks if cache entry exists and is still valid.
///
/// * `true` means no cache, or contents are modified
/// * `false` means the entry is still valid
#[inline]
fn should_rehash(entries: &BTreeMap<u64, CacheEntry>, key: &u64, mtime: i64, size: u64) -> bool {
    entries
        .get(key)
        .is_none_or(|cached| cached.mtime != mtime || cached.size != size)
}

/// Loads cache database from disk using rkyv.
fn load_cache_db(cache_path: &Path) -> Result<FileCacheDB, CacheError> {
    let bytes = fs::read(cache_path)?;
    let archived = rkyv::access::<ArchivedFileCacheDB, rancor::Error>(&bytes)?;
    let cache = deserialize::<FileCacheDB, rancor::Error>(archived)?;
    Ok(cache)
}

/// Saves cache database to disk using rkyv.
fn save_cache_db(cache: &FileCacheDB, cache_path: &Path) -> Result<(), CacheError> {
    let bytes = rkyv::to_bytes::<rancor::Error>(cache)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(cache_path)?;
    file.write_all(&bytes)?;
    Ok(())
}

/// Returns digest of xxhash by calculating given file.
fn hash_file(file_path: &Path) -> io::Result<u64> {
    let mut reader = File::open(file_path)?;

    // NOTE use boxed slice to avoid stack overflow
    let mut buffer = vec![0u8; 64 * 1024].into_boxed_slice();
    let mut hasher = Xxh64::new(0);

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hasher.digest())
}
