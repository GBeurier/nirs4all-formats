use std::ops::Deref;
use std::sync::Arc;

use memmap2::Mmap;

use crate::error::{Error, Result};

/// Header prefix read on the first pass for range-backed classic opens.
#[cfg(feature = "netcdf4")]
const INITIAL_HEADER_READ: usize = 64 * 1024;

#[derive(Clone)]
enum ClassicStorageBacking {
    Bytes(Arc<[u8]>),
    Mmap(Arc<Mmap>),
    #[cfg(feature = "netcdf4")]
    Range(hdf5_reader::storage::DynStorage),
}

#[derive(Clone)]
pub(crate) struct ClassicStorage {
    backing: ClassicStorageBacking,
    len: u64,
}

#[cfg(feature = "rayon")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClassicStorageKind {
    Bytes,
    Mmap,
    #[cfg(feature = "netcdf4")]
    Range,
}

#[cfg(feature = "rayon")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ClassicParallelReadPolicy {
    pub(crate) min_bytes: usize,
    pub(crate) target_chunk_bytes: usize,
}

#[cfg(feature = "rayon")]
impl ClassicParallelReadPolicy {
    const LOCAL_MIN_BYTES: usize = 32 * 1024 * 1024;
    const LOCAL_TARGET_CHUNK_BYTES: usize = 8 * 1024 * 1024;
    // Only referenced by the range-backed (HDF5-storage) arm below, which is
    // itself gated on `netcdf4`; without that feature these would be dead code.
    #[cfg(feature = "netcdf4")]
    const RANGE_MIN_BYTES: usize = 1024 * 1024;
    #[cfg(feature = "netcdf4")]
    const RANGE_TARGET_CHUNK_BYTES: usize = 1024 * 1024;

    fn for_kind(kind: ClassicStorageKind) -> Self {
        match kind {
            ClassicStorageKind::Bytes | ClassicStorageKind::Mmap => Self {
                min_bytes: Self::LOCAL_MIN_BYTES,
                target_chunk_bytes: Self::LOCAL_TARGET_CHUNK_BYTES,
            },
            #[cfg(feature = "netcdf4")]
            ClassicStorageKind::Range => Self {
                min_bytes: Self::RANGE_MIN_BYTES,
                target_chunk_bytes: Self::RANGE_TARGET_CHUNK_BYTES,
            },
        }
    }
}

#[derive(Clone)]
pub(crate) enum ClassicBuffer {
    Bytes {
        data: Arc<[u8]>,
        start: usize,
        len: usize,
    },
    Mmap {
        mmap: Arc<Mmap>,
        start: usize,
        len: usize,
    },
    #[cfg(feature = "netcdf4")]
    Range(hdf5_reader::StorageBuffer),
}

impl ClassicStorage {
    pub(crate) fn from_bytes(bytes: Vec<u8>) -> Self {
        let len = bytes.len() as u64;
        Self {
            backing: ClassicStorageBacking::Bytes(Arc::<[u8]>::from(bytes)),
            len,
        }
    }

    pub(crate) fn from_mmap(mmap: Mmap) -> Self {
        let len = mmap.len() as u64;
        Self {
            backing: ClassicStorageBacking::Mmap(Arc::new(mmap)),
            len,
        }
    }

    #[cfg(feature = "netcdf4")]
    pub(crate) fn from_range(storage: hdf5_reader::storage::DynStorage) -> Self {
        let len = storage.len();
        Self {
            backing: ClassicStorageBacking::Range(storage),
            len,
        }
    }

    pub(crate) fn len(&self) -> u64 {
        self.len
    }

    #[cfg(feature = "rayon")]
    pub(crate) fn kind(&self) -> ClassicStorageKind {
        match &self.backing {
            ClassicStorageBacking::Bytes(_) => ClassicStorageKind::Bytes,
            ClassicStorageBacking::Mmap(_) => ClassicStorageKind::Mmap,
            #[cfg(feature = "netcdf4")]
            ClassicStorageBacking::Range(_) => ClassicStorageKind::Range,
        }
    }

    #[cfg(feature = "rayon")]
    pub(crate) fn parallel_read_policy(&self) -> ClassicParallelReadPolicy {
        ClassicParallelReadPolicy::for_kind(self.kind())
    }

    #[cfg(feature = "netcdf4")]
    pub(crate) fn initial_header_len(&self) -> usize {
        usize::try_from(self.len.min(INITIAL_HEADER_READ as u64)).unwrap_or(INITIAL_HEADER_READ)
    }

    pub(crate) fn read_range(&self, offset: u64, len: usize) -> Result<ClassicBuffer> {
        let needed = u64::try_from(len)
            .map_err(|_| Error::InvalidData("classic range length exceeds u64".to_string()))?;
        let end = offset
            .checked_add(needed)
            .ok_or_else(|| Error::InvalidData("classic byte range exceeds u64".to_string()))?;
        if end > self.len {
            return Err(Error::UnexpectedEof {
                offset,
                needed,
                available: self.len.saturating_sub(offset),
            });
        }

        match &self.backing {
            ClassicStorageBacking::Bytes(data) => {
                let start = usize::try_from(offset).map_err(|_| {
                    Error::InvalidData("classic byte offset exceeds platform usize".to_string())
                })?;
                Ok(ClassicBuffer::Bytes {
                    data: data.clone(),
                    start,
                    len,
                })
            }
            ClassicStorageBacking::Mmap(mmap) => {
                let start = usize::try_from(offset).map_err(|_| {
                    Error::InvalidData("classic byte offset exceeds platform usize".to_string())
                })?;
                Ok(ClassicBuffer::Mmap {
                    mmap: mmap.clone(),
                    start,
                    len,
                })
            }
            #[cfg(feature = "netcdf4")]
            ClassicStorageBacking::Range(storage) => {
                Ok(ClassicBuffer::Range(storage.read_range(offset, len)?))
            }
        }
    }

    #[cfg(feature = "netcdf4")]
    pub(crate) fn read_header_prefix(&self, len: usize) -> Result<ClassicBuffer> {
        let capped = len.min(usize::try_from(self.len).unwrap_or(usize::MAX));
        self.read_range(0, capped)
    }
}

impl ClassicBuffer {
    #[cfg(feature = "netcdf4")]
    pub(crate) fn len(&self) -> usize {
        match self {
            ClassicBuffer::Bytes { len, .. } | ClassicBuffer::Mmap { len, .. } => *len,
            #[cfg(feature = "netcdf4")]
            ClassicBuffer::Range(buffer) => buffer.len(),
        }
    }
}

impl AsRef<[u8]> for ClassicBuffer {
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl Deref for ClassicBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match self {
            ClassicBuffer::Bytes { data, start, len } => &data[*start..*start + *len],
            ClassicBuffer::Mmap { mmap, start, len } => &mmap[*start..*start + *len],
            #[cfg(feature = "netcdf4")]
            ClassicBuffer::Range(buffer) => buffer.as_ref(),
        }
    }
}
