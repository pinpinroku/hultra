use std::{collections::HashSet, fs, io, path::Path};
use tracing::instrument;

use crate::log::anonymize;

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
