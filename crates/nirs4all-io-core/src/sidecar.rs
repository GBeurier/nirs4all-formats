//! Sidecar resolution contract.
//!
//! Many spectroscopy formats reference additional files alongside their
//! primary payload: ENVI `.hdr` headers, ERDAS LAN `.spc`/`.GIS` companions,
//! the FGI XML/HDF5 pairing, the ARM MFRSR YAML quality-control sidecar.
//! The [`SidecarResolver`] trait lets readers stay agnostic about whether
//! those companions live on a real filesystem (`FsSidecars`) or in an
//! in-memory map (`InMemorySidecars`).
//!
//! The runtime-level implementations live in the `nirs4all-io` crate; this
//! module only defines the trait so readers and bindings can depend on it
//! without pulling in the registry.

use std::path::{Path, PathBuf};

use crate::Result;

/// Resolve files that a reader needs in addition to its primary payload.
///
/// `relative` paths are interpreted relative to the primary file's logical
/// parent directory. Implementations are free to keep them case-sensitive;
/// case fallback is the responsibility of individual readers (only ENVI
/// has been observed to need it).
pub trait SidecarResolver: Send + Sync {
    /// Return the raw bytes for a sidecar referenced by `relative`.
    fn read(&self, relative: &Path) -> Result<Vec<u8>>;

    /// Return `true` if and only if [`read`] would succeed for `relative`.
    /// Used by readers to probe optional sidecars (e.g. AVIRIS `.GIS`).
    fn contains(&self, relative: &Path) -> bool;

    /// Optional listing of every known sidecar key. Empty by default; used
    /// by debugging tools and CLI dumps.
    fn list(&self) -> Vec<PathBuf> {
        Vec::new()
    }
}
