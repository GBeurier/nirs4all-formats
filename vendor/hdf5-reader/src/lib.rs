pub mod checksum;
pub mod error;
pub mod io;

// Level 0 — File Metadata
pub mod superblock;

// Level 1 — File Infrastructure
pub mod btree_v1;
pub mod btree_v2;
pub mod chunk_index;
pub mod extensible_array;
pub mod fixed_array;
pub mod fractal_heap;
pub mod global_heap;
pub mod local_heap;
pub mod shared_message_table;
pub mod symbol_table;

// Level 2 — Data Objects
pub mod messages;
pub mod object_header;

// High-level API
pub mod attribute_api;
pub mod dataset;
pub mod datatype_api;
pub mod group;
pub mod reference;
pub mod storage;

// Filters
pub mod filters;

// Utilities
pub mod cache;

use std::collections::HashMap;
#[cfg(unix)]
use std::ffi::{CString, OsStr};
use std::fs::File;
use std::io::ErrorKind;
#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, OnceLock};

use memmap2::Mmap;
// parking_lot::Mutex used via fully-qualified paths in HeaderCache and constructors.

use cache::ChunkCache;
use error::{Error, Result};
use group::Group;
use messages::HdfMessage;
use object_header::ObjectHeader;
use shared_message_table::SharedMessageTableRef;
use storage::DynStorage;
use superblock::Superblock;

// Re-exports
pub use attribute_api::Attribute;
pub use cache::ChunkCacheStats;
use dataset::DatasetTemplate;
pub use dataset::{Dataset, DatasetChunk, DatasetChunkIterator, SliceInfo, SliceInfoElem};
pub use datatype_api::{
    dtype_element_size, CompoundField, EnumMember, H5Type, ReferenceType, StringEncoding,
    StringPadding, StringSize, VarLenKind,
};
pub use error::ByteOrder;
pub use filters::FilterRegistry;
pub use messages::datatype::Datatype;
pub use storage::{
    BlockCacheStats, BlockCacheStorage, BytesStorage, FileStorage, MmapStorage,
    RangeRequestStorage, Storage, StorageBuffer,
};

/// Configuration options for opening an HDF5 file.
pub struct OpenOptions {
    /// Maximum bytes for the chunk cache. Default: 64 MiB.
    pub chunk_cache_bytes: usize,
    /// Maximum number of chunk cache slots. Default: 521.
    pub chunk_cache_slots: usize,
    /// Custom filter registry. If `None`, the default built-in filters are used.
    pub filter_registry: Option<FilterRegistry>,
    /// Resolver for HDF5 external raw data files. If `None`, external raw data
    /// files are not resolved.
    pub external_file_resolver: Option<Arc<dyn ExternalFileResolver>>,
    /// Optional resolver for HDF5 external links.
    pub external_link_resolver: Option<Arc<dyn ExternalLinkResolver>>,
}

impl Default for OpenOptions {
    fn default() -> Self {
        OpenOptions {
            chunk_cache_bytes: 64 * 1024 * 1024,
            chunk_cache_slots: 521,
            filter_registry: None,
            external_file_resolver: None,
            external_link_resolver: None,
        }
    }
}

/// Resolves file names from HDF5 External Data Files messages to storage.
///
/// Implementations are responsible for their own path security policy. The
/// built-in [`FilesystemExternalFileResolver`] confines normal paths to a base
/// directory and, on Unix, opens paths through an anchored directory handle
/// without following symlinks. On non-Unix platforms it falls back to
/// canonicalize-then-open, so attacker-writable resolver roots are out of
/// scope there.
pub trait ExternalFileResolver: Send + Sync {
    fn resolve_external_file(&self, filename: &str) -> Result<Option<DynStorage>>;
}

/// Resolves HDF5 external links to another opened file.
///
/// Implementations are responsible for their own path security policy. The
/// built-in [`FilesystemExternalLinkResolver`] confines normal paths to a base
/// directory and, on Unix, opens paths through an anchored directory handle
/// without following symlinks. On non-Unix platforms it falls back to
/// canonicalize-then-open, so attacker-writable resolver roots are out of
/// scope there.
pub trait ExternalLinkResolver: Send + Sync {
    fn resolve_external_link(&self, filename: &str) -> Result<Option<Hdf5File>>;
}

fn normalize_resolver_path(filename: &str, description: &str) -> Result<PathBuf> {
    let path = Path::new(filename);
    if path.as_os_str().is_empty() {
        return Err(Error::InvalidData(format!("{description} path is empty")));
    }

    if path.is_absolute() {
        return Err(Error::InvalidData(format!(
            "{description} path must be relative: {filename}"
        )));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(name) => normalized.push(name),
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return Err(Error::InvalidData(format!(
                    "{description} path must stay within the resolver base directory: {filename}"
                )));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(Error::InvalidData(format!("{description} path is empty")));
    }

    Ok(normalized)
}

#[cfg(not(unix))]
fn open_external_file_within_base(
    base_dir: &Path,
    relative_path: &Path,
    description: &str,
    filename: &str,
) -> Result<Option<File>> {
    let base = match base_dir.canonicalize() {
        Ok(path) => path,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let candidate = base.join(relative_path);
    let resolved = match candidate.canonicalize() {
        Ok(path) => path,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };

    if !resolved.starts_with(&base) {
        return Err(Error::InvalidData(format!(
            "{description} path escapes the resolver base directory: {filename}"
        )));
    }

    Ok(Some(File::open(resolved)?))
}

#[cfg(unix)]
fn open_external_file_within_base(
    base_dir: &Path,
    relative_path: &Path,
    description: &str,
    filename: &str,
) -> Result<Option<File>> {
    let mut dir = match open_unix_path(
        base_dir,
        libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
    ) {
        Ok(file) => file,
        Err(err) if path_lookup_is_missing(&err) => return Ok(None),
        Err(err) if path_lookup_is_symlink(&err) => {
            return Err(Error::InvalidData(format!(
                "{description} resolver base directory must not be a symlink"
            )));
        }
        Err(err) => return Err(err.into()),
    };
    if !dir.metadata()?.is_dir() {
        return Ok(None);
    }

    let parts: Vec<&OsStr> = relative_path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();

    let Some((leaf, parents)) = parts.split_last() else {
        return Err(Error::InvalidData(format!("{description} path is empty")));
    };

    for parent in parents {
        dir = match open_unix_child(
            &dir,
            parent,
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        ) {
            Ok(file) => file,
            Err(err) if path_lookup_is_missing(&err) => return Ok(None),
            Err(err) if path_lookup_is_symlink(&err) => {
                return Err(symlink_resolver_error(description, filename));
            }
            Err(err) => return Err(err.into()),
        };
        if !dir.metadata()?.is_dir() {
            return Ok(None);
        }
    }

    let file = match open_unix_child(
        &dir,
        leaf,
        libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
    ) {
        Ok(file) => file,
        Err(err) if path_lookup_is_missing(&err) => return Ok(None),
        Err(err) if path_lookup_is_symlink(&err) => {
            return Err(symlink_resolver_error(description, filename));
        }
        Err(err) => return Err(err.into()),
    };

    if file.metadata()?.is_dir() {
        return Err(Error::InvalidData(format!(
            "{description} path resolves to a directory: {filename}"
        )));
    }

    Ok(Some(file))
}

#[cfg(unix)]
fn open_unix_path(path: &Path, flags: libc::c_int) -> std::io::Result<File> {
    let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "filesystem path contains an interior NUL byte",
        )
    })?;
    // SAFETY: `path` is NUL-terminated, `flags` is passed through unchanged,
    // and the returned descriptor is checked before ownership is assumed.
    let fd = unsafe { libc::open(path.as_ptr(), flags) };
    file_from_unix_fd(fd)
}

#[cfg(unix)]
fn open_unix_child(dir: &File, name: &OsStr, flags: libc::c_int) -> std::io::Result<File> {
    let name = CString::new(name.as_bytes()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "filesystem path contains an interior NUL byte",
        )
    })?;
    // SAFETY: `dir` owns a live descriptor, `name` is NUL-terminated, and the
    // returned descriptor is checked before ownership is assumed.
    let fd = unsafe { libc::openat(dir.as_raw_fd(), name.as_ptr(), flags) };
    file_from_unix_fd(fd)
}

#[cfg(unix)]
fn file_from_unix_fd(fd: libc::c_int) -> std::io::Result<File> {
    if fd < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        // SAFETY: a successful `open`/`openat` returned an owned descriptor,
        // and this transfers that ownership exactly once to `File`.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

#[cfg(unix)]
fn path_lookup_is_missing(err: &std::io::Error) -> bool {
    err.kind() == ErrorKind::NotFound
        || matches!(err.raw_os_error(), Some(code) if code == libc::ENOTDIR)
}

#[cfg(unix)]
fn path_lookup_is_symlink(err: &std::io::Error) -> bool {
    matches!(err.raw_os_error(), Some(code) if code == libc::ELOOP)
}

#[cfg(unix)]
fn symlink_resolver_error(description: &str, filename: &str) -> Error {
    Error::InvalidData(format!(
        "{description} path escapes the resolver base directory or uses a symlink: {filename}"
    ))
}

/// Filesystem resolver for external raw data files.
///
/// The resolver rejects absolute paths and `..` components. On Unix, it opens
/// paths relative to `base_dir` using `openat` and `O_NOFOLLOW`, so symlinks
/// are rejected instead of being followed. On non-Unix platforms,
/// attacker-writable resolver roots are out of scope.
#[derive(Debug, Clone)]
pub struct FilesystemExternalFileResolver {
    base_dir: PathBuf,
}

impl FilesystemExternalFileResolver {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    fn relative_path_for(&self, filename: &str) -> Result<PathBuf> {
        normalize_resolver_path(filename, "external raw data file")
    }
}

impl ExternalFileResolver for FilesystemExternalFileResolver {
    fn resolve_external_file(&self, filename: &str) -> Result<Option<DynStorage>> {
        let relative_path = self.relative_path_for(filename)?;
        let Some(file) = open_external_file_within_base(
            &self.base_dir,
            &relative_path,
            "external raw data file",
            filename,
        )?
        else {
            return Ok(None);
        };
        Ok(Some(Arc::new(FileStorage::from_file(file)?)))
    }
}

/// Filesystem resolver for external links. Linked files are cached after the
/// first successful open.
///
/// The resolver rejects absolute paths and `..` components. On Unix, it opens
/// paths relative to `base_dir` using `openat` and `O_NOFOLLOW`, so symlinks
/// are rejected instead of being followed. On non-Unix platforms,
/// attacker-writable resolver roots are out of scope.
pub struct FilesystemExternalLinkResolver {
    base_dir: PathBuf,
    cache: parking_lot::Mutex<HashMap<PathBuf, Hdf5File>>,
}

impl FilesystemExternalLinkResolver {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
            cache: parking_lot::Mutex::new(HashMap::new()),
        }
    }

    fn relative_path_for(&self, filename: &str) -> Result<PathBuf> {
        normalize_resolver_path(filename, "external link")
    }
}

impl ExternalLinkResolver for FilesystemExternalLinkResolver {
    fn resolve_external_link(&self, filename: &str) -> Result<Option<Hdf5File>> {
        let relative_path = self.relative_path_for(filename)?;

        if let Some(file) = self.cache.lock().get(&relative_path).cloned() {
            return Ok(Some(file));
        }

        let Some(opened) = open_external_file_within_base(
            &self.base_dir,
            &relative_path,
            "external link",
            filename,
        )?
        else {
            return Ok(None);
        };

        let file = Hdf5File::from_storage(Arc::new(FileStorage::from_file(opened)?))?;
        self.cache.lock().insert(relative_path, file.clone());
        Ok(Some(file))
    }
}

/// Cache for parsed object headers, keyed by file address.
pub type HeaderCache = Arc<parking_lot::Mutex<HashMap<u64, Arc<ObjectHeader>>>>;

/// An opened HDF5 file.
///
/// This is the main entry point for reading HDF5 files. Storage is random-
/// access and range-based, so metadata and data reads do not require an eager
/// whole-file mapping.
#[derive(Clone)]
pub struct Hdf5File {
    context: Arc<FileContext>,
}

pub(crate) struct FileContext {
    pub(crate) storage: DynStorage,
    pub(crate) superblock: Superblock,
    pub(crate) chunk_cache: Arc<ChunkCache>,
    pub(crate) header_cache: HeaderCache,
    pub(crate) dataset_path_cache: Arc<parking_lot::Mutex<HashMap<String, Arc<DatasetTemplate>>>>,
    pub(crate) filter_registry: Arc<FilterRegistry>,
    pub(crate) external_file_resolver: Option<Arc<dyn ExternalFileResolver>>,
    pub(crate) external_link_resolver: Option<Arc<dyn ExternalLinkResolver>>,
    pub(crate) external_file_cache: parking_lot::Mutex<HashMap<String, DynStorage>>,
    sohm_table: OnceLock<std::result::Result<Option<SharedMessageTableRef>, String>>,
    full_file_cache: OnceLock<StorageBuffer>,
}

impl FileContext {
    pub(crate) fn read_range(&self, offset: u64, len: usize) -> Result<StorageBuffer> {
        self.storage.read_range(offset, len)
    }

    pub(crate) fn full_file_data(&self) -> Result<StorageBuffer> {
        if let Some(buffer) = self.full_file_cache.get() {
            return Ok(buffer.clone());
        }

        let len = usize::try_from(self.storage.len()).map_err(|_| {
            Error::InvalidData("file size exceeds platform usize capacity".to_string())
        })?;
        let buffer = self.storage.read_range(0, len)?;
        let _ = self.full_file_cache.set(buffer);
        Ok(self
            .full_file_cache
            .get()
            .expect("full-file buffer must exist after successful initialization")
            .clone())
    }

    pub(crate) fn get_or_parse_header(&self, addr: u64) -> Result<Arc<ObjectHeader>> {
        {
            let cache = self.header_cache.lock();
            if let Some(hdr) = cache.get(&addr) {
                return Ok(Arc::clone(hdr));
            }
        }

        let mut hdr = ObjectHeader::parse_at_storage(
            self.storage.as_ref(),
            addr,
            self.superblock.offset_size,
            self.superblock.length_size,
        )?;
        hdr.resolve_shared_messages_storage_with_sohm(
            self.storage.as_ref(),
            self.superblock.offset_size,
            self.superblock.length_size,
            |heap_id, message_type| self.resolve_sohm_message(heap_id, message_type),
        )?;
        let arc = Arc::new(hdr);
        let mut cache = self.header_cache.lock();
        cache.insert(addr, Arc::clone(&arc));
        Ok(arc)
    }

    fn resolve_sohm_message(
        &self,
        heap_id: &[u8],
        message_type: u16,
    ) -> Result<Option<HdfMessage>> {
        let Some(table) = self.sohm_table()? else {
            return Ok(None);
        };
        table.resolve_heap_message(
            heap_id,
            message_type,
            self.storage.as_ref(),
            self.superblock.offset_size,
            self.superblock.length_size,
            Some(self.filter_registry.as_ref()),
        )
    }

    fn sohm_table(&self) -> Result<Option<SharedMessageTableRef>> {
        let cached = self
            .sohm_table
            .get_or_init(|| self.load_sohm_table().map_err(|err| err.to_string()));
        match cached {
            Ok(table) => Ok(table.clone()),
            Err(message) => Err(Error::InvalidData(format!(
                "failed to load SOHM table: {message}"
            ))),
        }
    }

    fn load_sohm_table(&self) -> Result<Option<SharedMessageTableRef>> {
        let Some(extension_address) = self.superblock.extension_address else {
            return Ok(None);
        };
        let extension = ObjectHeader::parse_at_storage(
            self.storage.as_ref(),
            extension_address,
            self.superblock.offset_size,
            self.superblock.length_size,
        )?;

        let shared_table = extension.messages.iter().find_map(|message| match message {
            HdfMessage::SharedTable(table) => Some(table),
            _ => None,
        });
        let Some(shared_table) = shared_table else {
            return Ok(None);
        };

        let table = crate::shared_message_table::SharedMessageTable::parse_at_storage(
            self.storage.as_ref(),
            shared_table.table_address,
            shared_table.num_indices,
            self.superblock.offset_size,
        )?;
        Ok(Some(Arc::new(table)))
    }

    pub(crate) fn resolve_external_file(&self, filename: &str) -> Result<Option<DynStorage>> {
        if let Some(storage) = self.external_file_cache.lock().get(filename).cloned() {
            return Ok(Some(storage));
        }

        let Some(resolver) = self.external_file_resolver.as_ref() else {
            return Ok(None);
        };
        let Some(storage) = resolver.resolve_external_file(filename)? else {
            return Ok(None);
        };
        self.external_file_cache
            .lock()
            .insert(filename.to_string(), storage.clone());
        Ok(Some(storage))
    }
}

impl Hdf5File {
    fn from_storage_impl(storage: DynStorage, options: OpenOptions) -> Result<Self> {
        let superblock = Superblock::parse_from_storage(storage.as_ref())?;
        let cache = Arc::new(ChunkCache::new(
            options.chunk_cache_bytes,
            options.chunk_cache_slots,
        ));
        let registry = options.filter_registry.unwrap_or_default();
        let external_file_resolver = options.external_file_resolver;
        let external_link_resolver = options.external_link_resolver;

        Ok(Hdf5File {
            context: Arc::new(FileContext {
                storage,
                superblock,
                chunk_cache: cache,
                header_cache: Arc::new(parking_lot::Mutex::new(HashMap::new())),
                dataset_path_cache: Arc::new(parking_lot::Mutex::new(HashMap::new())),
                filter_registry: Arc::new(registry),
                external_file_resolver,
                external_link_resolver,
                external_file_cache: parking_lot::Mutex::new(HashMap::new()),
                sohm_table: OnceLock::new(),
                full_file_cache: OnceLock::new(),
            }),
        })
    }

    /// Open an HDF5 file with default options.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_options(path, OpenOptions::default())
    }

    /// Open an HDF5 file with custom options.
    pub fn open_with_options(path: impl AsRef<Path>, options: OpenOptions) -> Result<Self> {
        let path = path.as_ref();
        Self::from_storage_with_options(Arc::new(FileStorage::open(path)?), options)
    }

    /// Open an HDF5 file from an in-memory byte slice.
    ///
    /// The data is copied into an owned buffer.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        Self::from_bytes_with_options(data, OpenOptions::default())
    }

    /// Open an HDF5 file from an in-memory byte slice with custom options.
    ///
    /// The data is copied into an owned buffer.
    pub fn from_bytes_with_options(data: &[u8], options: OpenOptions) -> Result<Self> {
        Self::from_vec_with_options(data.to_vec(), options)
    }

    /// Open an HDF5 file from an owned byte vector without copying.
    pub fn from_vec(data: Vec<u8>) -> Result<Self> {
        Self::from_vec_with_options(data, OpenOptions::default())
    }

    /// Open an HDF5 file from an owned byte vector with custom options.
    pub fn from_vec_with_options(data: Vec<u8>, options: OpenOptions) -> Result<Self> {
        Self::from_storage_with_options(Arc::new(BytesStorage::new(data)), options)
    }

    /// Open an HDF5 file from an existing memory map with custom options.
    ///
    /// This avoids remapping when the caller already owns a read-only mapping.
    pub fn from_mmap_with_options(mmap: Mmap, options: OpenOptions) -> Result<Self> {
        Self::from_storage_with_options(Arc::new(MmapStorage::new(mmap)), options)
    }

    /// Open an HDF5 file from a custom random-access storage backend.
    pub fn from_storage(storage: DynStorage) -> Result<Self> {
        Self::from_storage_with_options(storage, OpenOptions::default())
    }

    /// Open an HDF5 file from a custom random-access storage backend.
    pub fn from_storage_with_options(storage: DynStorage, options: OpenOptions) -> Result<Self> {
        Self::from_storage_impl(storage, options)
    }

    /// Get the parsed superblock.
    pub fn superblock(&self) -> &Superblock {
        &self.context.superblock
    }

    /// Access the underlying random-access storage backend.
    pub fn storage(&self) -> &dyn Storage {
        self.context.storage.as_ref()
    }

    /// Return current chunk-cache statistics for this file.
    pub fn chunk_cache_stats(&self) -> ChunkCacheStats {
        self.context.chunk_cache.stats()
    }

    /// Look up or parse an object header at the given address.
    ///
    /// Uses the internal cache to avoid re-parsing the same header.
    pub fn get_or_parse_header(&self, addr: u64) -> Result<Arc<ObjectHeader>> {
        self.context.get_or_parse_header(addr)
    }

    /// Get the root group of the file.
    pub fn root_group(&self) -> Result<Group> {
        let addr = self.context.superblock.root_object_header_address()?;

        Ok(Group::new(
            self.context.clone(),
            addr,
            "/".to_string(),
            addr, // root_address = self
        ))
    }

    /// Convenience: get a dataset at a path like "/group1/dataset".
    pub fn dataset(&self, path: &str) -> Result<Dataset> {
        let parts: Vec<&str> = path
            .trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();
        let normalized_path = format!("/{}", parts.join("/"));

        if parts.is_empty() {
            return Err(Error::DatasetNotFound(path.to_string()).with_context(path));
        }

        if let Some(template) = self
            .context
            .dataset_path_cache
            .lock()
            .get(&normalized_path)
            .cloned()
        {
            return Ok(Dataset::from_template(self.context.clone(), template));
        }

        let mut group = self.root_group()?;
        for &part in &parts[..parts.len() - 1] {
            group = group.group(part).map_err(|e| e.with_context(path))?;
        }

        let dataset = group
            .dataset(parts[parts.len() - 1])
            .map_err(|e| e.with_context(path))?;
        if Arc::ptr_eq(&dataset.context, &self.context) {
            self.context
                .dataset_path_cache
                .lock()
                .insert(normalized_path, dataset.template());
        }
        Ok(dataset)
    }

    /// Convenience: get a group at a path like "/group1/subgroup".
    pub fn group(&self, path: &str) -> Result<Group> {
        let parts: Vec<&str> = path
            .trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        let mut group = self.root_group()?;
        for &part in &parts {
            group = group.group(part)?;
        }

        Ok(group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_options_default() {
        let opts = OpenOptions::default();
        assert_eq!(opts.chunk_cache_bytes, 64 * 1024 * 1024);
        assert_eq!(opts.chunk_cache_slots, 521);
        assert!(opts.external_file_resolver.is_none());
    }

    #[test]
    fn invalid_file() {
        let data = b"this is not an HDF5 file";
        let result = Hdf5File::from_bytes(data);
        assert!(result.is_err());
    }

    #[test]
    fn btree_v2_chunked_fixture_uses_btree_v2_index() {
        use crate::messages::layout::{ChunkIndexing, DataLayout};

        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("testdata/hdf5/btree_v2_chunked.h5");
        if !path.exists() {
            eprintln!("SKIPPED: fixture btree_v2_chunked.h5 not found");
            return;
        }

        let file = Hdf5File::open(path).unwrap();
        let dataset = file.dataset("/data").unwrap();
        assert!(matches!(
            dataset.layout,
            DataLayout::Chunked {
                chunk_indexing: Some(ChunkIndexing::BTreeV2),
                ..
            }
        ));
    }

    #[test]
    fn filesystem_external_file_resolver_reads_relative_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("raw.bin");
        std::fs::write(&path, b"abcdef").unwrap();

        let resolver = FilesystemExternalFileResolver::new(dir.path());
        let storage = resolver
            .resolve_external_file("raw.bin")
            .unwrap()
            .expect("raw file should resolve");
        let bytes = storage.read_range(2, 3).unwrap();
        assert_eq!(bytes.as_ref(), b"cde");
    }

    #[test]
    fn filesystem_external_file_resolver_rejects_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("raw.bin");
        std::fs::write(&path, b"abcdef").unwrap();

        let resolver = FilesystemExternalFileResolver::new(dir.path());
        let err = match resolver.resolve_external_file(path.to_str().unwrap()) {
            Ok(_) => panic!("absolute external file path should be rejected"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("must be relative"));
    }

    #[test]
    fn filesystem_external_file_resolver_rejects_parent_component() {
        let dir = tempfile::tempdir().unwrap();
        let resolver = FilesystemExternalFileResolver::new(dir.path());

        let err = match resolver.resolve_external_file("../raw.bin") {
            Ok(_) => panic!("parent external file path should be rejected"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("resolver base directory"));
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_external_file_resolver_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_path = outside.path().join("raw.bin");
        std::fs::write(&outside_path, b"abcdef").unwrap();
        symlink(&outside_path, dir.path().join("raw.bin")).unwrap();

        let resolver = FilesystemExternalFileResolver::new(dir.path());
        let err = match resolver.resolve_external_file("raw.bin") {
            Ok(_) => panic!("symlink escape should be rejected"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("escapes"));
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_external_file_resolver_rejects_symlink_inside_base() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("raw.bin"), b"abcdef").unwrap();
        symlink("raw.bin", dir.path().join("link.bin")).unwrap();

        let resolver = FilesystemExternalFileResolver::new(dir.path());
        let err = match resolver.resolve_external_file("link.bin") {
            Ok(_) => panic!("symlinks should be rejected even when they point inside the base"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("symlink"));
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_external_file_resolver_rejects_symlink_directory_component() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let real_dir = dir.path().join("real");
        std::fs::create_dir(&real_dir).unwrap();
        std::fs::write(real_dir.join("raw.bin"), b"abcdef").unwrap();
        symlink("real", dir.path().join("linkdir")).unwrap();

        let resolver = FilesystemExternalFileResolver::new(dir.path());
        let err = match resolver.resolve_external_file("linkdir/raw.bin") {
            Ok(_) => panic!("symlinked directory components should be rejected"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("symlink"));
    }

    #[test]
    fn filesystem_external_link_resolver_rejects_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("linked.h5");
        std::fs::write(&path, b"not really hdf5").unwrap();

        let resolver = FilesystemExternalLinkResolver::new(dir.path());
        let err = match resolver.resolve_external_link(path.to_str().unwrap()) {
            Ok(_) => panic!("absolute external link path should be rejected"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("must be relative"));
    }

    #[test]
    fn filesystem_external_link_resolver_rejects_parent_component() {
        let dir = tempfile::tempdir().unwrap();
        let resolver = FilesystemExternalLinkResolver::new(dir.path());

        let err = match resolver.resolve_external_link("../linked.h5") {
            Ok(_) => panic!("parent external link path should be rejected"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("resolver base directory"));
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_external_link_resolver_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_path = outside.path().join("linked.h5");
        std::fs::write(&outside_path, b"not really hdf5").unwrap();
        symlink(&outside_path, dir.path().join("linked.h5")).unwrap();

        let resolver = FilesystemExternalLinkResolver::new(dir.path());
        let err = match resolver.resolve_external_link("linked.h5") {
            Ok(_) => panic!("symlink escape should be rejected"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("escapes"));
    }
}
