use std::{collections::HashSet, ffi::OsString, io, path::Path};

/// Returns blacklisted mods for update.
pub fn fetch_updater_blacklist(mods_dir: &Path) -> io::Result<HashSet<OsString>> {
    let path = mods_dir.join("updaterblacklist.txt");
    let content = std::fs::read_to_string(&path).or_else(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            Ok(String::new())
        } else {
            Err(e)
        }
    })?;

    Ok(parse_blacklist(&content))
}

fn parse_blacklist(content: &str) -> HashSet<OsString> {
    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(OsString::from)
        .collect()
}
