use std::{
    collections::HashSet,
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct ModFile(PathBuf);

impl ModFile {
    pub fn try_from_path(path: PathBuf) -> Option<Self> {
        if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            Some(Self(path))
        } else {
            None
        }
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    #[cfg(test)]
    pub fn new_unchecked(path: PathBuf) -> Self {
        Self(path)
    }
}

impl ModFile {
    pub fn is_blacklisted(&self, blacklist: &HashSet<String>) -> bool {
        self.0
            .file_name()
            .and_then(|n| n.to_str())
            .map(|name| blacklist.contains(name))
            .unwrap_or(false)
    }
}

pub trait ModIdentityService {
    fn fetch_id(&self, path: &Path) -> io::Result<u64>;
}
