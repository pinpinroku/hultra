use std::{borrow::Cow, fmt};

use mod_file::ModFile;

mod manifest;
pub mod mod_file;
mod resolver;

pub use resolver::scan_mods;

/// Information of installed mod.
#[derive(Debug, Clone)]
pub struct LocalMod {
    /// Full path to the ZIP archive of the mod.
    file: ModFile,
    /// Mod name.
    name: String,
    /// Version label of the mod to display.
    version: DisplayVersion,
}

#[derive(Debug, Clone)]
struct DisplayVersion(String);

impl std::fmt::Display for DisplayVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "v{}", self.0)
    }
}

impl LocalMod {
    pub fn new(file: ModFile, name: String, version: String) -> Self {
        Self {
            file,
            name,
            version: DisplayVersion(version),
        }
    }

    pub fn file(&self) -> &ModFile {
        &self.file
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version.0
    }
}

impl fmt::Display for LocalMod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self
            .file()
            .path()
            .file_stem()
            .is_some_and(|name| name.eq_ignore_ascii_case(self.name()))
        {
            write!(f, "{} (v{})", self.name(), self.version())?;
        } else {
            let filename = self
                .file()
                .path()
                .file_name()
                .map(|name| name.to_string_lossy())
                .unwrap_or(Cow::Borrowed("unknown"));

            write!(f, "*{} (v{}) [{}]", self.name(), self.version(), filename)?;
        }
        Ok(())
    }
}
