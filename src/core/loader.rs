//! Service for resolving installed mods.
use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

use rayon::prelude::*;
use tracing::instrument;

use crate::{core::local::LocalMod, manifest::Manifest};

pub struct ModResolver;

impl ModResolver {
    fn resolve_manifest(path: &Path) -> anyhow::Result<Manifest> {
        let bytes = zip_finder::extract_file_from_zip(path, b"everest.yaml", Some(b"everest.yml"))?;
        let manifest = Manifest::parse(&bytes)?;
        Ok(manifest)
    }

    /// Resolves a list of installed mods from given paths.
    #[instrument(skip_all)]
    pub fn resolve_from_paths(paths: &[PathBuf]) -> io::Result<Vec<LocalMod>> {
        let mods = paths
            .into_par_iter()
            .filter_map(|path| {
                let manifest = Self::resolve_manifest(path).ok()?;
                LocalMod::new(path, manifest.name, manifest.version).ok()
            })
            .collect();
        Ok(mods)
    }

    /// Resolves a list of installed mod names from given paths.
    #[instrument(skip_all)]
    pub fn resolve_names_from_paths(paths: &[PathBuf]) -> io::Result<HashSet<String>> {
        let names = paths
            .into_par_iter()
            .filter_map(|path| {
                let manifest = Self::resolve_manifest(path).ok()?;
                Some(manifest.name)
            })
            .collect();
        Ok(names)
    }
}

// TODO move this to proper module
pub struct ModsDirectoryScanner;

impl ModsDirectoryScanner {
    /// Scans mods directory to collect the path of ZIP archives.
    pub fn scan(mods_dir: &Path) -> io::Result<Vec<PathBuf>> {
        let found_paths: Vec<_> = fs::read_dir(mods_dir)?
            .filter_map(|res| res.ok())
            .map(|e| e.path())
            .filter(|p| is_mod_archive(p))
            .collect();
        Ok(found_paths)
    }
}

fn is_mod_archive(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
}
