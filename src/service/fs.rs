use std::{collections::HashSet, fs, io, path::Path};

use tracing::instrument;

use crate::{core::mod_file::ModFile, log::anonymize};

/// Returns blacklisted mods for update.
#[instrument(skip_all, fields(mods_dir = %anonymize(mods_dir)), ret(Debug))]
pub fn fetch_updater_blacklist(mods_dir: &Path) -> io::Result<HashSet<String>> {
    let path = mods_dir.join("updaterblacklist.txt");
    let content = fs::read_to_string(&path).or_else(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            Ok(String::new())
        } else {
            Err(e)
        }
    })?;

    Ok(parse_blacklist(&content))
}

fn parse_blacklist(content: &str) -> HashSet<String> {
    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(String::from)
        .collect()
}

pub struct ModsDirectoryScanner;

impl ModsDirectoryScanner {
    /// Scans mods directory to collect the path of ZIP archives.
    pub fn scan(mods_dir: &Path) -> io::Result<Vec<ModFile>> {
        let found_paths: Vec<_> = fs::read_dir(mods_dir)?
            .filter_map(|res| res.ok())
            .filter_map(|e| ModFile::try_from_path(e.path()))
            .collect();
        Ok(found_paths)
    }
}
