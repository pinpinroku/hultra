//! Service for resolving installed mods.
use std::{io, path::Path};

use rayon::prelude::*;
use tracing::instrument;

use crate::{
    core::{LocalMod, ModFile, ModsDirectoryScanner, manifest::Manifest},
    log::anonymize,
};

/// Scans installed mods.
#[instrument(skip_all, fields(mods_dir = %anonymize(mods_dir)))]
pub fn scan_mods(
    scanner: &impl ModsDirectoryScanner,
    mods_dir: &Path,
) -> io::Result<Vec<LocalMod>> {
    let files = scanner.scan(mods_dir)?;
    Ok(ModResolver::resolve(&files))
}

struct ModResolver;

impl ModResolver {
    fn resolve_manifest(path: &Path) -> anyhow::Result<Manifest> {
        let bytes = zip_finder::extract_file_from_zip(path, b"everest.yaml", Some(b"everest.yml"))?;
        let manifest = Manifest::parse(&bytes)?;
        Ok(manifest)
    }

    /// Resolves a list of installed mods.
    #[instrument(skip_all)]
    fn resolve(files: &[ModFile]) -> Vec<LocalMod> {
        files
            .into_par_iter()
            .filter_map(|file| {
                let manifest = Self::resolve_manifest(file.path()).ok()?;
                Some(LocalMod::new(file.clone(), manifest.name, manifest.version))
            })
            .collect()
    }
}
