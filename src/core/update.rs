use std::{fmt::Display, str::FromStr};

use tracing::debug;

use crate::core::{
    Checksum, Checksums, ParseChecksumError,
    cache::FileCacheDb,
    network::downloader::{DownloadFile, ParseDownloadFileError},
    registry::Entry,
};

/// Identifies required updates by comparing file checksums.
pub fn scan_updates<'a>(
    cache_db: &FileCacheDb,
    contexts: &'a [UpdateContext],
) -> Result<UpdateReport<'a>, ParseDownloadFileError> {
    let mut updates = Vec::new();
    let mut download_files = Vec::new();

    for ctx in contexts {
        let is_valid = cache_db.is_cache_valid(&ctx.inode, &ctx.checksums);

        debug!(
            mod=ctx.name,
            cache_valid=is_valid,
            current_version=ctx.current_version,
            available_version=ctx.available_version
        );

        if !is_valid {
            let update_info =
                UpdateInfo::new(&ctx.name, &ctx.current_version, &ctx.available_version);
            let download_task = DownloadFile::try_from(ctx)?;

            updates.push(update_info);
            download_files.push(download_task);
        }
    }
    Ok(UpdateReport {
        download_files,
        updates,
    })
}

#[derive(Debug)]
pub struct UpdateContext {
    current_version: String,
    available_version: String,
    inode: u64,
    name: String,
    url: String,
    size: u64,
    checksums: Checksums,
}

impl UpdateContext {
    pub fn new(
        current_version: &str,
        inode: u64,
        name: String,
        entry: Entry,
    ) -> Result<Self, ParseChecksumError> {
        let checksums = entry
            .checksums()
            .iter()
            .map(|s| Checksum::from_str(s))
            .collect::<Result<Checksums, _>>()?;

        Ok(Self {
            current_version: current_version.to_string(),
            available_version: entry.version().to_string(),
            inode,
            name,
            url: entry.url().to_string(),
            size: entry.file_size(),
            checksums,
        })
    }
    #[cfg(test)]
    pub fn inode(&self) -> u64 {
        self.inode
    }
    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn checksums(&self) -> &Checksums {
        &self.checksums
    }
}

/// Result of scanning mods for update.
#[derive(Debug)]
pub struct UpdateReport<'a> {
    /// Files to download.
    pub download_files: Vec<DownloadFile>,
    /// A list of mod information to display.
    pub updates: Vec<UpdateInfo<'a>>,
}

/// Update information to display.
#[derive(Debug)]
pub struct UpdateInfo<'a> {
    name: &'a str,
    current_version: &'a str,
    available_version: &'a str,
}

impl<'a> UpdateInfo<'a> {
    fn new(name: &'a str, current_version: &'a str, available_version: &'a str) -> Self {
        Self {
            name,
            current_version,
            available_version,
        }
    }
}

impl<'a> Display for UpdateInfo<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "* {}: {} -> {}",
            self.name, self.current_version, self.available_version
        )
    }
}
