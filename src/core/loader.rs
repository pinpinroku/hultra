//! Service for resolving installed mods.
use std::{io, path::Path};

use rayon::prelude::*;
use tracing::instrument;

use crate::core::{LocalMod, LocalModFileSource, ModFileSource, manifest::Manifest};

/// Scans installed mods.
pub fn scan_mods(mods_dir: &Path) -> io::Result<Vec<LocalMod>> {
    let source = LocalModFileSource::new(mods_dir);
    let resolver = ModResolver::new(source);
    resolver.resolve()
}

struct ModResolver<S: ModFileSource> {
    source: S,
    // R: MetadataReader,
}

impl<S: ModFileSource> ModResolver<S> {
    fn new(source: S) -> Self {
        Self { source }
    }

    /// Resolves a list of installed mods.
    #[instrument(skip_all)]
    fn resolve(self) -> io::Result<Vec<LocalMod>> {
        let files = self.source.fetch_all()?;
        let mods = files
            .into_par_iter()
            .filter_map(|file| {
                let manifest = Self::resolve_manifest(file.path()).ok()?;
                Some(LocalMod::new(file.clone(), manifest.name, manifest.version))
            })
            .collect();
        Ok(mods)
    }

    // TODO should be MetadataReader, then call `reader.read_manifest(&path)`
    fn resolve_manifest(path: &Path) -> anyhow::Result<Manifest> {
        let bytes = zip_finder::extract_file_from_zip(path, b"everest.yaml", Some(b"everest.yml"))?;
        let manifest = Manifest::parse(&bytes)?;
        Ok(manifest)
    }
}
