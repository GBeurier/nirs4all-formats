use std::fs::File;
use std::num::NonZeroUsize;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use lru::LruCache;
use memmap2::Mmap;
use parking_lot::Mutex;

use crate::error::{Error, Result};

#[derive(Clone)]
enum StorageBacking {
    Bytes(Arc<[u8]>),
    Mmap(Arc<Mmap>),
}

/// An immutable byte range returned by a storage backend.
#[derive(Clone)]
pub struct StorageBuffer {
    backing: StorageBacking,
    start: usize,
    len: usize,
}

impl StorageBuffer {
    pub fn from_vec(bytes: Vec<u8>) -> Self {
        let len = bytes.len();
        Self {
            backing: StorageBacking::Bytes(Arc::<[u8]>::from(bytes)),
            start: 0,
            len,
        }
    }

    pub(crate) fn from_arc_bytes(bytes: Arc<[u8]>, start: usize, len: usize) -> Self {
        Self {
            backing: StorageBacking::Bytes(bytes),
            start,
            len,
        }
    }

    pub(crate) fn from_arc_mmap(mmap: Arc<Mmap>, start: usize, len: usize) -> Self {
        Self {
            backing: StorageBacking::Mmap(mmap),
            start,
            len,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl AsRef<[u8]> for StorageBuffer {
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl Deref for StorageBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match &self.backing {
            StorageBacking::Bytes(bytes) => &bytes[self.start..self.start + self.len],
            StorageBacking::Mmap(mmap) => &mmap[self.start..self.start + self.len],
        }
    }
}

/// Random-access, immutable byte storage for HDF5 parsing and reads.
pub trait Storage: Send + Sync {
    /// Total length in bytes.
    fn len(&self) -> u64;

    /// Returns `true` if the storage is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Read a byte range from `offset..offset+len`.
    fn read_range(&self, offset: u64, len: usize) -> Result<StorageBuffer>;
}

pub type DynStorage = Arc<dyn Storage>;

fn check_storage_range(total_len: u64, offset: u64, len: usize) -> Result<u64> {
    let needed = u64::try_from(len).map_err(|_| Error::OffsetOutOfBounds(offset))?;
    let end = offset
        .checked_add(needed)
        .ok_or(Error::OffsetOutOfBounds(offset))?;
    if end > total_len {
        return Err(Error::UnexpectedEof {
            offset,
            needed,
            available: total_len.saturating_sub(offset),
        });
    }
    Ok(end)
}

/// In-memory storage backed by owned bytes.
pub struct BytesStorage {
    data: Arc<[u8]>,
}

impl BytesStorage {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data: Arc::<[u8]>::from(data),
        }
    }
}

impl Storage for BytesStorage {
    fn len(&self) -> u64 {
        self.data.len() as u64
    }

    fn read_range(&self, offset: u64, len: usize) -> Result<StorageBuffer> {
        let start = usize::try_from(offset).map_err(|_| Error::OffsetOutOfBounds(offset))?;
        let end = start
            .checked_add(len)
            .ok_or(Error::OffsetOutOfBounds(offset))?;
        if end > self.data.len() {
            return Err(Error::UnexpectedEof {
                offset,
                needed: len as u64,
                available: self.len().saturating_sub(offset),
            });
        }
        Ok(StorageBuffer::from_arc_bytes(self.data.clone(), start, len))
    }
}

/// In-memory storage backed by a read-only memory map.
pub struct MmapStorage {
    mmap: Arc<Mmap>,
}

impl MmapStorage {
    pub fn new(mmap: Mmap) -> Self {
        Self {
            mmap: Arc::new(mmap),
        }
    }
}

impl Storage for MmapStorage {
    fn len(&self) -> u64 {
        self.mmap.len() as u64
    }

    fn read_range(&self, offset: u64, len: usize) -> Result<StorageBuffer> {
        let start = usize::try_from(offset).map_err(|_| Error::OffsetOutOfBounds(offset))?;
        let end = start
            .checked_add(len)
            .ok_or(Error::OffsetOutOfBounds(offset))?;
        if end > self.mmap.len() {
            return Err(Error::UnexpectedEof {
                offset,
                needed: len as u64,
                available: self.len().saturating_sub(offset),
            });
        }
        Ok(StorageBuffer::from_arc_mmap(self.mmap.clone(), start, len))
    }
}

/// File-backed storage that serves explicit byte ranges via positional reads.
pub struct FileStorage {
    file: Arc<File>,
    len: u64,
}

impl FileStorage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path)?;
        Self::from_file(file)
    }

    pub fn from_file(file: File) -> Result<Self> {
        let len = file.metadata()?.len();
        Ok(Self {
            file: Arc::new(file),
            len,
        })
    }
}

impl Storage for FileStorage {
    fn len(&self) -> u64 {
        self.len
    }

    fn read_range(&self, offset: u64, len: usize) -> Result<StorageBuffer> {
        let needed = u64::try_from(len).map_err(|_| Error::OffsetOutOfBounds(offset))?;
        let end = offset
            .checked_add(needed)
            .ok_or(Error::OffsetOutOfBounds(offset))?;
        if end > self.len {
            return Err(Error::UnexpectedEof {
                offset,
                needed,
                available: self.len.saturating_sub(offset),
            });
        }

        let mut buf = vec![0u8; len];
        read_exact_at(self.file.as_ref(), &mut buf, offset)?;
        Ok(StorageBuffer::from_vec(buf))
    }
}

/// Storage backed by a caller-provided byte-range reader.
///
/// This is intended for HTTP range requests, S3/object-store clients, or other
/// remote sources that can return exactly the requested byte range.
pub struct RangeRequestStorage {
    len: u64,
    reader: Arc<RangeReader>,
}

type RangeReader = dyn Fn(u64, usize) -> Result<Vec<u8>> + Send + Sync;

impl RangeRequestStorage {
    /// Create a storage backend from a total length and a range reader.
    pub fn new<F>(len: u64, reader: F) -> Self
    where
        F: Fn(u64, usize) -> Result<Vec<u8>> + Send + Sync + 'static,
    {
        Self {
            len,
            reader: Arc::new(reader),
        }
    }
}

impl Storage for RangeRequestStorage {
    fn len(&self) -> u64 {
        self.len
    }

    fn read_range(&self, offset: u64, len: usize) -> Result<StorageBuffer> {
        check_storage_range(self.len, offset, len)?;
        let bytes = (self.reader)(offset, len)?;
        if bytes.len() != len {
            return Err(Error::UnexpectedEof {
                offset,
                needed: len as u64,
                available: bytes.len() as u64,
            });
        }
        Ok(StorageBuffer::from_vec(bytes))
    }
}

/// Point-in-time statistics for [`BlockCacheStorage`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlockCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub evictions: u64,
    pub current_bytes: usize,
    pub entries: usize,
    pub block_size: usize,
    pub max_blocks: usize,
}

/// Block-aligned LRU cache for any random-access storage backend.
///
/// This is useful when the underlying storage is remote or has high per-request
/// latency. Reads are rounded out to fixed-size blocks and cached by block
/// number; callers still see the normal [`Storage::read_range`] interface.
pub struct BlockCacheStorage {
    inner: DynStorage,
    block_size: NonZeroUsize,
    max_blocks: NonZeroUsize,
    state: Mutex<BlockCacheState>,
}

struct BlockCacheState {
    cache: LruCache<u64, Arc<[u8]>>,
    current_bytes: usize,
    hits: u64,
    misses: u64,
    inserts: u64,
    evictions: u64,
}

impl BlockCacheStorage {
    /// Create a block cache around an existing storage backend.
    ///
    /// `block_size` and `max_blocks` values of zero are normalized to 1.
    pub fn new(inner: DynStorage, block_size: usize, max_blocks: usize) -> Self {
        let block_size = NonZeroUsize::new(block_size).unwrap_or(NonZeroUsize::new(1).unwrap());
        let max_blocks = NonZeroUsize::new(max_blocks).unwrap_or(NonZeroUsize::new(1).unwrap());
        Self {
            inner,
            block_size,
            max_blocks,
            state: Mutex::new(BlockCacheState {
                cache: LruCache::new(max_blocks),
                current_bytes: 0,
                hits: 0,
                misses: 0,
                inserts: 0,
                evictions: 0,
            }),
        }
    }

    /// Create a block cache with conservative defaults: 1 MiB blocks, 128 slots.
    pub fn with_defaults(inner: DynStorage) -> Self {
        Self::new(inner, 1024 * 1024, 128)
    }

    /// Return current cache statistics.
    pub fn stats(&self) -> BlockCacheStats {
        let state = self.state.lock();
        BlockCacheStats {
            hits: state.hits,
            misses: state.misses,
            inserts: state.inserts,
            evictions: state.evictions,
            current_bytes: state.current_bytes,
            entries: state.cache.len(),
            block_size: self.block_size.get(),
            max_blocks: self.max_blocks.get(),
        }
    }

    fn read_block(&self, block_index: u64) -> Result<Arc<[u8]>> {
        {
            let mut state = self.state.lock();
            if let Some(block) = state.cache.get(&block_index).cloned() {
                state.hits += 1;
                return Ok(block);
            }
            state.misses += 1;
        }

        let block_size = self.block_size.get();
        let block_start = block_index
            .checked_mul(block_size as u64)
            .ok_or(Error::OffsetOutOfBounds(u64::MAX))?;
        let remaining = self.inner.len().saturating_sub(block_start);
        let read_len = block_size.min(usize::try_from(remaining).unwrap_or(usize::MAX));
        let bytes = self.inner.read_range(block_start, read_len)?;
        let block = Arc::<[u8]>::from(bytes.as_ref());

        let mut state = self.state.lock();
        if let Some(replaced) = state.cache.peek(&block_index) {
            state.current_bytes = state.current_bytes.saturating_sub(replaced.len());
        } else {
            while state.cache.len() >= self.max_blocks.get() && !state.cache.is_empty() {
                if let Some((_, evicted)) = state.cache.pop_lru() {
                    state.current_bytes = state.current_bytes.saturating_sub(evicted.len());
                    state.evictions += 1;
                }
            }
        }

        state.current_bytes += block.len();
        state.inserts += 1;
        state.cache.put(block_index, block.clone());
        Ok(block)
    }
}

impl Storage for BlockCacheStorage {
    fn len(&self) -> u64 {
        self.inner.len()
    }

    fn read_range(&self, offset: u64, len: usize) -> Result<StorageBuffer> {
        let end = check_storage_range(self.len(), offset, len)?;
        if len == 0 {
            return Ok(StorageBuffer::from_vec(Vec::new()));
        }

        let block_size = self.block_size.get() as u64;
        let first_block = offset / block_size;
        let last_block = (end - 1) / block_size;

        if first_block == last_block {
            let block = self.read_block(first_block)?;
            let block_start = first_block
                .checked_mul(block_size)
                .ok_or(Error::OffsetOutOfBounds(offset))?;
            let start = usize::try_from(offset - block_start)
                .map_err(|_| Error::OffsetOutOfBounds(offset))?;
            return Ok(StorageBuffer::from_arc_bytes(block, start, len));
        }

        let mut output = vec![0u8; len];
        let mut written = 0usize;
        for block_index in first_block..=last_block {
            let block = self.read_block(block_index)?;
            let block_start = block_index
                .checked_mul(block_size)
                .ok_or(Error::OffsetOutOfBounds(offset))?;
            let copy_start = offset.max(block_start);
            let copy_end = end.min(block_start + block.len() as u64);
            let src_start = usize::try_from(copy_start - block_start)
                .map_err(|_| Error::OffsetOutOfBounds(offset))?;
            let copy_len = usize::try_from(copy_end - copy_start)
                .map_err(|_| Error::OffsetOutOfBounds(offset))?;
            output[written..written + copy_len]
                .copy_from_slice(&block[src_start..src_start + copy_len]);
            written += copy_len;
        }

        Ok(StorageBuffer::from_vec(output))
    }
}

#[cfg(unix)]
fn read_exact_at(file: &File, mut buf: &mut [u8], mut offset: u64) -> std::io::Result<()> {
    use std::os::unix::fs::FileExt;

    while !buf.is_empty() {
        let n = file.read_at(buf, offset)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "failed to fill whole buffer",
            ));
        }
        offset += n as u64;
        buf = &mut buf[n..];
    }
    Ok(())
}

#[cfg(windows)]
fn read_exact_at(file: &File, mut buf: &mut [u8], mut offset: u64) -> std::io::Result<()> {
    use std::os::windows::fs::FileExt;

    while !buf.is_empty() {
        let n = file.seek_read(buf, offset)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "failed to fill whole buffer",
            ));
        }
        offset += n as u64;
        buf = &mut buf[n..];
    }
    Ok(())
}

/// Fallback for targets without a positional file-read syscall (in
/// practice `wasm32-unknown-unknown` and similar no-OS targets).
/// `FileStorage` is never instantiated there because there is no
/// `std::fs::File`-backed flow; this stub keeps the crate linkable
/// when only `BytesStorage` / `MmapStorage` are used.
#[cfg(not(any(unix, windows)))]
fn read_exact_at(_file: &File, _buf: &mut [u8], _offset: u64) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "FileStorage is unavailable on this target; use BytesStorage or Hdf5File::from_bytes",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex as StdMutex;

    #[test]
    fn range_request_storage_reads_exact_ranges() {
        let data: Arc<[u8]> = Arc::from((0u8..32).collect::<Vec<_>>());
        let storage = RangeRequestStorage::new(data.len() as u64, {
            let data = data.clone();
            move |offset, len| {
                let start = offset as usize;
                Ok(data[start..start + len].to_vec())
            }
        });

        let bytes = storage.read_range(4, 6).unwrap();
        assert_eq!(bytes.as_ref(), &[4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn range_request_storage_rejects_short_reads() {
        let storage = RangeRequestStorage::new(16, |_offset, _len| Ok(vec![1, 2]));
        let err = match storage.read_range(0, 4) {
            Ok(_) => panic!("short range read should fail"),
            Err(err) => err,
        };
        assert!(matches!(err, Error::UnexpectedEof { .. }));
    }

    #[test]
    fn block_cache_storage_reuses_aligned_blocks() {
        let data: Arc<[u8]> = Arc::from((0u8..64).collect::<Vec<_>>());
        let reads = Arc::new(StdMutex::new(Vec::new()));
        let inner = Arc::new(RangeRequestStorage::new(data.len() as u64, {
            let data = data.clone();
            let reads = reads.clone();
            move |offset, len| {
                reads.lock().unwrap().push((offset, len));
                let start = offset as usize;
                Ok(data[start..start + len].to_vec())
            }
        }));
        let storage = BlockCacheStorage::new(inner, 8, 2);

        assert_eq!(storage.read_range(2, 4).unwrap().as_ref(), &[2, 3, 4, 5]);
        assert_eq!(storage.read_range(4, 2).unwrap().as_ref(), &[4, 5]);
        assert_eq!(
            storage.read_range(6, 6).unwrap().as_ref(),
            &[6, 7, 8, 9, 10, 11]
        );
        assert_eq!(storage.read_range(18, 2).unwrap().as_ref(), &[18, 19]);

        assert_eq!(*reads.lock().unwrap(), vec![(0, 8), (8, 8), (16, 8)]);
        let stats = storage.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 3);
        assert_eq!(stats.inserts, 3);
        assert_eq!(stats.evictions, 1);
        assert_eq!(stats.entries, 2);
    }
}
