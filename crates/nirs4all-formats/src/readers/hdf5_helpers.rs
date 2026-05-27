//! HDF5/NetCDF in-memory open helpers that wire the [`SidecarResolver`] into
//! `hdf5-reader`'s `ExternalFileResolver` / `ExternalLinkResolver` slots.
//!
//! Gated by `feature = "fmt-hdf5"`.

use std::path::PathBuf;
use std::sync::Arc;

use hdf5_reader::storage::DynStorage;
use hdf5_reader::{
    BytesStorage, ExternalFileResolver, ExternalLinkResolver, Hdf5File, OpenOptions,
};
use nirs4all_formats_core::{Error, Result, SidecarResolver};

fn into_invalid(format: &str, error: impl std::fmt::Display) -> Error {
    Error::InvalidRecord(format!("{format}: {error}"))
}

fn build_options(sidecars: Arc<dyn SidecarResolver>) -> OpenOptions {
    let adapter = Arc::new(SidecarBackedExternal {
        sidecars: sidecars.clone(),
    });
    OpenOptions {
        external_file_resolver: Some(adapter.clone() as Arc<dyn ExternalFileResolver>),
        external_link_resolver: Some(adapter as Arc<dyn ExternalLinkResolver>),
        ..OpenOptions::default()
    }
}

/// Open an HDF5 file from in-memory bytes, with `sidecars` available for
/// external raw-data files and external links.
pub fn open_hdf5(
    bytes: Vec<u8>,
    sidecars: Arc<dyn SidecarResolver>,
    format_tag: &str,
) -> Result<Hdf5File> {
    let options = build_options(sidecars);
    Hdf5File::from_vec_with_options(bytes, options).map_err(|e| into_invalid(format_tag, e))
}

/// Adapter that lets `hdf5-reader` ask our [`SidecarResolver`] for external
/// files and external links.
///
/// Sidecar misses are mapped to `Ok(None)`; only real parse / I/O errors
/// propagate. This matches the convention used by
/// [`hdf5_reader::FilesystemExternalFileResolver`] when a file is absent.
struct SidecarBackedExternal {
    sidecars: Arc<dyn SidecarResolver>,
}

impl SidecarBackedExternal {
    fn load(&self, filename: &str) -> Option<Vec<u8>> {
        let rel = PathBuf::from(filename);
        if !self.sidecars.contains(&rel) {
            return None;
        }
        self.sidecars.read(&rel).ok()
    }
}

impl ExternalFileResolver for SidecarBackedExternal {
    fn resolve_external_file(
        &self,
        filename: &str,
    ) -> std::result::Result<Option<DynStorage>, hdf5_reader::error::Error> {
        match self.load(filename) {
            Some(bytes) => Ok(Some(Arc::new(BytesStorage::new(bytes)))),
            None => Ok(None),
        }
    }
}

impl ExternalLinkResolver for SidecarBackedExternal {
    fn resolve_external_link(
        &self,
        filename: &str,
    ) -> std::result::Result<Option<Hdf5File>, hdf5_reader::error::Error> {
        let Some(bytes) = self.load(filename) else {
            return Ok(None);
        };
        // Re-inject the same resolver so nested external links keep working.
        let options = build_options(self.sidecars.clone());
        let file = Hdf5File::from_vec_with_options(bytes, options)?;
        Ok(Some(file))
    }
}
