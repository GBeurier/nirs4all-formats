//! Concrete [`SidecarResolver`] implementations.
//!
//! - [`FsSidecars`] reads sidecars from a real directory.
//! - [`InMemorySidecars`] serves sidecars from an owned byte map.
//! - [`NoSidecars`] errors on every call; used as a default for entry
//!   points that don't pass a resolver (`open_bytes`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nirs4all_io_core::{Error, Result, SidecarResolver};

/// Filesystem-backed resolver.
#[derive(Clone, Debug)]
pub struct FsSidecars {
    base: PathBuf,
}

impl FsSidecars {
    pub fn new(base: impl Into<PathBuf>) -> Self {
        Self { base: base.into() }
    }

    fn resolve(&self, relative: &Path) -> PathBuf {
        if relative.is_absolute() {
            relative.to_path_buf()
        } else {
            self.base.join(relative)
        }
    }
}

impl SidecarResolver for FsSidecars {
    fn read(&self, relative: &Path) -> Result<Vec<u8>> {
        let target = self.resolve(relative);
        std::fs::read(&target).map_err(|source| Error::Io {
            path: target,
            source,
        })
    }

    fn contains(&self, relative: &Path) -> bool {
        self.resolve(relative).exists()
    }
}

/// In-memory resolver. Keys are interpreted as relative paths.
#[derive(Clone, Debug, Default)]
pub struct InMemorySidecars {
    map: BTreeMap<PathBuf, Vec<u8>>,
}

impl InMemorySidecars {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, path: impl Into<PathBuf>, bytes: impl Into<Vec<u8>>) -> Self {
        self.insert(path, bytes);
        self
    }

    pub fn insert(&mut self, path: impl Into<PathBuf>, bytes: impl Into<Vec<u8>>) {
        self.map.insert(path.into(), bytes.into());
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }
}

impl SidecarResolver for InMemorySidecars {
    fn read(&self, relative: &Path) -> Result<Vec<u8>> {
        self.map
            .get(relative)
            .cloned()
            .ok_or_else(|| Error::UnsupportedSidecar {
                path: relative.to_path_buf(),
                reason: "key not present in the in-memory sidecar map".to_string(),
            })
    }

    fn contains(&self, relative: &Path) -> bool {
        self.map.contains_key(relative)
    }

    fn list(&self) -> Vec<PathBuf> {
        self.map.keys().cloned().collect()
    }
}

/// Resolver that refuses every lookup. Used by `open_bytes` so readers that
/// declare a sidecar requirement get a clean, descriptive error instead of
/// reading from an arbitrary working directory.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoSidecars;

impl SidecarResolver for NoSidecars {
    fn read(&self, relative: &Path) -> Result<Vec<u8>> {
        Err(Error::UnsupportedSidecar {
            path: relative.to_path_buf(),
            reason: "no sidecar resolver was supplied; call open_with_sidecars".to_string(),
        })
    }

    fn contains(&self, _relative: &Path) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_resolver_round_trip() {
        let resolver = InMemorySidecars::new()
            .with("foo.hdr", b"header bytes".to_vec())
            .with("foo.dat", b"\x00\x01\x02".to_vec());
        assert!(resolver.contains(Path::new("foo.hdr")));
        assert!(resolver.contains(Path::new("foo.dat")));
        assert!(!resolver.contains(Path::new("missing")));
        assert_eq!(
            resolver.read(Path::new("foo.hdr")).unwrap(),
            b"header bytes"
        );
        assert_eq!(resolver.read(Path::new("foo.dat")).unwrap(), vec![0, 1, 2]);
        let mut keys: Vec<_> = resolver
            .list()
            .into_iter()
            .map(|p| p.display().to_string())
            .collect();
        keys.sort();
        assert_eq!(keys, vec!["foo.dat".to_string(), "foo.hdr".to_string()]);
    }

    #[test]
    fn no_sidecars_errors_with_path() {
        let resolver = NoSidecars;
        let err = resolver.read(Path::new("anything.hdr")).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("anything.hdr"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn fs_sidecars_resolves_relative_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("a.hdr"), b"x").expect("write");
        let resolver = FsSidecars::new(dir.path());
        assert!(resolver.contains(Path::new("a.hdr")));
        assert_eq!(resolver.read(Path::new("a.hdr")).unwrap(), b"x");
    }
}
