//! HDF5 Fractal Heap (FRHP).
//!
//! Fractal heaps are the storage mechanism for link messages and attribute
//! messages in new-style (v2) groups and datasets. They use a doubling-table
//! scheme with direct and indirect blocks. Objects are addressed by a
//! heap ID that encodes the offset and length within the heap.
//!
//! This module parses the heap header and provides object extraction for
//! managed, tiny, and unfiltered huge object IDs.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::checksum::jenkins_lookup3;
use crate::error::{Error, Result};
use crate::filters::{self, FilterRegistry};
use crate::io::Cursor;
use crate::messages::filter_pipeline::FilterPipelineMessage;
use crate::storage::Storage;

/// Signature bytes for a fractal heap header: ASCII `FRHP`.
const FRHP_SIGNATURE: [u8; 4] = *b"FRHP";

/// Signature bytes for a direct block: ASCII `FHDB`.
const _FHDB_SIGNATURE: [u8; 4] = *b"FHDB";

/// Signature bytes for an indirect block: ASCII `FHIB`.
const FHIB_SIGNATURE: [u8; 4] = *b"FHIB";
const MAX_FRACTAL_HEAP_INDIRECT_DEPTH: usize = 64;
const MAX_FRACTAL_HEAP_INDIRECT_ROWS: u16 = 64;

/// Parsed fractal heap header.
#[derive(Debug, Clone)]
pub struct FractalHeap {
    /// Size in bytes of heap IDs used to reference objects.
    pub heap_id_len: u16,
    /// Size in bytes of I/O filter info (0 if none).
    pub io_filters_len: u16,
    /// Heap status flags.
    pub flags: u8,
    /// Maximum size of a managed object (larger objects become "huge").
    pub max_managed_object_size: u64,
    /// Next huge object ID to assign.
    pub next_huge_id: u64,
    /// Address of the B-tree v2 used for huge objects.
    pub btree_huge_objects_address: u64,
    /// Address of the free-space manager for managed blocks.
    pub free_space_managed_address: u64,
    /// Total managed space in bytes.
    pub managed_space_amount: u64,
    /// Total managed allocated space in bytes.
    pub managed_alloc_amount: u64,
    /// Iterator offset for managed free-space.
    pub managed_iter_offset: u64,
    /// Number of managed objects.
    pub managed_objects_count: u64,
    /// Total size of huge objects in bytes.
    pub huge_objects_size: u64,
    /// Number of huge objects.
    pub huge_objects_count: u64,
    /// Total size of tiny objects in bytes.
    pub tiny_objects_size: u64,
    /// Number of tiny objects.
    pub tiny_objects_count: u64,
    /// Width of the doubling table (number of direct blocks per row).
    pub table_width: u16,
    /// Size in bytes of the starting (smallest) direct block.
    pub starting_block_size: u64,
    /// Maximum direct block size before switching to indirect blocks.
    pub max_direct_block_size: u64,
    /// Log2 of the maximum managed heap size (used for heap ID encoding).
    pub max_heap_size: u16,
    /// Starting row of the root indirect block (for doubling table).
    pub starting_row_root_indirect: u16,
    /// Address of the root block (direct or indirect).
    pub root_block_address: u64,
    /// Current number of rows in the root indirect block.
    pub current_rows_in_root_indirect: u16,
    /// Filtered root direct block size (present only when io_filters_len > 0).
    pub io_filter_size: Option<u64>,
    /// Filter mask for root direct block (present only when io_filters_len > 0).
    pub io_filter_mask: Option<u32>,
    /// Encoded filter pipeline for heap blocks/huge objects.
    pub io_filter_info: Vec<u8>,
}

/// Cache of verified direct blocks for repeated object lookups in one heap.
#[derive(Debug, Default)]
pub struct FractalHeapDirectBlockCache {
    blocks: HashMap<DirectBlockCacheKey, Arc<Vec<u8>>>,
}

type DirectBlockCacheKey = (u64, u64, Option<u64>, u32);

#[derive(Debug, Clone, Copy)]
struct DirectBlockLocation {
    address: u64,
    block_offset_in_heap: u64,
    block_size: u64,
    filtered_size: Option<u64>,
    filter_mask: u32,
}

#[derive(Debug, Clone, Copy)]
struct HugeObjectLocation {
    address: u64,
    disk_length: u64,
    filter_mask: u32,
    memory_length: Option<u64>,
}

impl FractalHeapDirectBlockCache {
    fn get_verified_block_storage(
        &mut self,
        heap: &FractalHeap,
        location: DirectBlockLocation,
        storage: &dyn Storage,
        offset_size: u8,
        filter_registry: Option<&FilterRegistry>,
    ) -> Result<Arc<Vec<u8>>> {
        let key = (
            location.address,
            location.block_size,
            location.filtered_size,
            location.filter_mask,
        );
        if let Some(block) = self.blocks.get(&key) {
            return Ok(block.clone());
        }

        let block =
            heap.load_direct_block_storage(location, storage, offset_size, filter_registry)?;
        heap.verify_direct_block_bytes(&block, offset_size)?;
        let block = Arc::new(block);
        self.blocks.insert(key, block.clone());
        Ok(block)
    }
}

impl FractalHeap {
    /// Parse a fractal heap header at the current cursor position.
    ///
    /// Format:
    /// - Signature: `FRHP` (4 bytes)
    /// - Version: 0 (1 byte)
    /// - Heap ID length (u16 LE)
    /// - I/O filters encoded length (u16 LE)
    /// - Flags (u8)
    /// - Max managed object size (u32 LE)
    /// - Next huge object ID (`length_size` bytes)
    /// - B-tree address for huge objects (`offset_size` bytes)
    /// - Managed free-space amount (`length_size` bytes)
    /// - Free-space manager address (`offset_size` bytes)
    /// - Managed space amount (`length_size` bytes)
    /// - Managed alloc amount (`length_size` bytes)
    /// - Managed free-space iterator offset (`length_size` bytes)
    /// - Managed objects count (`length_size` bytes)
    /// - Huge objects size (`length_size` bytes)
    /// - Huge objects count (`length_size` bytes)
    /// - Tiny objects size (`length_size` bytes)
    /// - Tiny objects count (`length_size` bytes)
    /// - Table width (u16 LE)
    /// - Starting block size (`length_size` bytes)
    /// - Maximum direct block size (`length_size` bytes)
    /// - Max heap size (u16 LE)
    /// - Starting row of root indirect block (u16 LE)
    /// - Root block address (`offset_size` bytes)
    /// - Current rows in root indirect block (u16 LE)
    /// - If io_filters_len > 0: filtered root direct block size (`length_size`), filter mask (u32 LE)
    /// - Checksum (u32 LE)
    pub fn parse(cursor: &mut Cursor, offset_size: u8, length_size: u8) -> Result<Self> {
        let start = cursor.position();

        let sig = cursor.read_bytes(4)?;
        if sig != FRHP_SIGNATURE {
            return Err(Error::InvalidFractalHeapSignature);
        }

        let version = cursor.read_u8()?;
        if version != 0 {
            return Err(Error::UnsupportedFractalHeapVersion(version));
        }

        let heap_id_len = cursor.read_u16_le()?;
        let io_filters_len = cursor.read_u16_le()?;
        let flags = cursor.read_u8()?;

        let max_managed_object_size = cursor.read_u32_le()? as u64;
        let next_huge_id = cursor.read_length(length_size)?;
        let btree_huge_objects_address = cursor.read_offset(offset_size)?;
        let _managed_free_space_amount = cursor.read_length(length_size)?;
        let free_space_managed_address = cursor.read_offset(offset_size)?;
        let managed_space_amount = cursor.read_length(length_size)?;
        let managed_alloc_amount = cursor.read_length(length_size)?;
        let managed_iter_offset = cursor.read_length(length_size)?;
        let managed_objects_count = cursor.read_length(length_size)?;
        let huge_objects_size = cursor.read_length(length_size)?;
        let huge_objects_count = cursor.read_length(length_size)?;
        let tiny_objects_size = cursor.read_length(length_size)?;
        let tiny_objects_count = cursor.read_length(length_size)?;

        let table_width = cursor.read_u16_le()?;
        let starting_block_size = cursor.read_length(length_size)?;
        let max_direct_block_size = cursor.read_length(length_size)?;
        let max_heap_size = cursor.read_u16_le()?;
        let starting_row_root_indirect = cursor.read_u16_le()?;
        let root_block_address = cursor.read_offset(offset_size)?;
        let current_rows_in_root_indirect = cursor.read_u16_le()?;

        let (io_filter_size, io_filter_mask) = if io_filters_len > 0 {
            let size = cursor.read_length(length_size)?;
            let mask = cursor.read_u32_le()?;
            (Some(size), Some(mask))
        } else {
            (None, None)
        };
        let io_filter_info = if io_filters_len > 0 {
            cursor.read_bytes(usize::from(io_filters_len))?.to_vec()
        } else {
            Vec::new()
        };

        // Verify checksum.
        let checksum_end = cursor.position();
        let stored_checksum = cursor.read_u32_le()?;
        let computed = jenkins_lookup3(&cursor.data()[start as usize..checksum_end as usize]);
        if computed != stored_checksum {
            return Err(Error::ChecksumMismatch {
                expected: stored_checksum,
                actual: computed,
            });
        }

        Ok(FractalHeap {
            heap_id_len,
            io_filters_len,
            flags,
            max_managed_object_size,
            next_huge_id,
            btree_huge_objects_address,
            free_space_managed_address,
            managed_space_amount,
            managed_alloc_amount,
            managed_iter_offset,
            managed_objects_count,
            huge_objects_size,
            huge_objects_count,
            tiny_objects_size,
            tiny_objects_count,
            table_width,
            starting_block_size,
            max_direct_block_size,
            max_heap_size,
            starting_row_root_indirect,
            root_block_address,
            current_rows_in_root_indirect,
            io_filter_size,
            io_filter_mask,
            io_filter_info,
        })
    }

    /// Parse a fractal heap header from random-access storage.
    pub fn parse_at_storage(
        storage: &dyn Storage,
        address: u64,
        offset_size: u8,
        length_size: u8,
    ) -> Result<Self> {
        let max_header_len = 256usize;
        let available = storage.len().saturating_sub(address);
        let len = usize::try_from(available.min(max_header_len as u64)).map_err(|_| {
            Error::InvalidData("fractal heap header exceeds platform usize capacity".into())
        })?;
        let bytes = storage.read_range(address, len)?;
        let mut cursor = Cursor::new(bytes.as_ref());
        Self::parse(&mut cursor, offset_size, length_size)
    }

    /// Extract any fractal heap object given a heap ID.
    pub fn get_object(
        &self,
        heap_id: &[u8],
        file_data: &[u8],
        offset_size: u8,
        length_size: u8,
    ) -> Result<Vec<u8>> {
        self.get_object_with_registry(heap_id, file_data, offset_size, length_size, None)
    }

    /// Extract any fractal heap object using a caller-provided filter registry
    /// for filtered managed and huge objects.
    pub fn get_object_with_registry(
        &self,
        heap_id: &[u8],
        file_data: &[u8],
        offset_size: u8,
        length_size: u8,
        filter_registry: Option<&FilterRegistry>,
    ) -> Result<Vec<u8>> {
        match self.heap_id_kind(heap_id)? {
            HeapIdKind::Managed => self.get_managed_object_impl(
                heap_id,
                file_data,
                offset_size,
                length_size,
                filter_registry,
            ),
            HeapIdKind::Huge => self.get_huge_object(
                heap_id,
                file_data,
                offset_size,
                length_size,
                filter_registry,
            ),
            HeapIdKind::Tiny => self.decode_tiny_object(heap_id),
        }
    }

    /// Extract any fractal heap object from random-access storage.
    pub fn get_object_storage(
        &self,
        heap_id: &[u8],
        storage: &dyn Storage,
        offset_size: u8,
        length_size: u8,
    ) -> Result<Vec<u8>> {
        self.get_object_storage_with_registry(heap_id, storage, offset_size, length_size, None)
    }

    /// Extract any fractal heap object from random-access storage using a
    /// caller-provided filter registry for filtered managed and huge objects.
    pub fn get_object_storage_with_registry(
        &self,
        heap_id: &[u8],
        storage: &dyn Storage,
        offset_size: u8,
        length_size: u8,
        filter_registry: Option<&FilterRegistry>,
    ) -> Result<Vec<u8>> {
        let mut cache = FractalHeapDirectBlockCache::default();
        self.get_object_storage_cached_with_registry(
            heap_id,
            storage,
            offset_size,
            length_size,
            &mut cache,
            filter_registry,
        )
    }

    /// Extract any fractal heap object from storage, reusing verified direct
    /// blocks across managed object lookups.
    pub fn get_object_storage_cached(
        &self,
        heap_id: &[u8],
        storage: &dyn Storage,
        offset_size: u8,
        length_size: u8,
        direct_block_cache: &mut FractalHeapDirectBlockCache,
    ) -> Result<Vec<u8>> {
        self.get_object_storage_cached_with_registry(
            heap_id,
            storage,
            offset_size,
            length_size,
            direct_block_cache,
            None,
        )
    }

    /// Extract any fractal heap object from storage while reusing verified
    /// direct blocks and a caller-provided filter registry.
    pub fn get_object_storage_cached_with_registry(
        &self,
        heap_id: &[u8],
        storage: &dyn Storage,
        offset_size: u8,
        length_size: u8,
        direct_block_cache: &mut FractalHeapDirectBlockCache,
        filter_registry: Option<&FilterRegistry>,
    ) -> Result<Vec<u8>> {
        match self.heap_id_kind(heap_id)? {
            HeapIdKind::Managed => self.get_managed_object_storage_cached_impl(
                heap_id,
                storage,
                offset_size,
                length_size,
                direct_block_cache,
                filter_registry,
            ),
            HeapIdKind::Huge => self.get_huge_object_storage(
                heap_id,
                storage,
                offset_size,
                length_size,
                filter_registry,
            ),
            HeapIdKind::Tiny => self.decode_tiny_object(heap_id),
        }
    }

    /// Extract a fractal heap object. Kept for existing callers; now also
    /// handles tiny and huge IDs.
    pub fn get_managed_object(
        &self,
        heap_id: &[u8],
        file_data: &[u8],
        offset_size: u8,
        length_size: u8,
    ) -> Result<Vec<u8>> {
        self.get_object_with_registry(heap_id, file_data, offset_size, length_size, None)
    }

    /// Extract a fractal heap object from random-access storage. Kept for
    /// existing callers; now also handles tiny and huge IDs.
    pub fn get_managed_object_storage(
        &self,
        heap_id: &[u8],
        storage: &dyn Storage,
        offset_size: u8,
        length_size: u8,
    ) -> Result<Vec<u8>> {
        self.get_object_storage_with_registry(heap_id, storage, offset_size, length_size, None)
    }

    fn get_managed_object_impl(
        &self,
        heap_id: &[u8],
        file_data: &[u8],
        offset_size: u8,
        length_size: u8,
        filter_registry: Option<&FilterRegistry>,
    ) -> Result<Vec<u8>> {
        let (heap_offset, obj_length) = self.decode_managed_heap_id(heap_id)?;

        if obj_length == 0 {
            return Ok(Vec::new());
        }

        let location = self.find_direct_block(heap_offset, file_data, offset_size, length_size)?;
        let block = self.load_direct_block(file_data, location, filter_registry)?;
        self.verify_direct_block_bytes(&block, offset_size)?;
        let offset_in_block = heap_offset
            .checked_sub(location.block_offset_in_heap)
            .ok_or_else(|| {
                Error::InvalidData("fractal heap object precedes direct block".into())
            })?;
        let data_start = usize::try_from(offset_in_block)
            .map_err(|_| Error::InvalidData("fractal heap object offset exceeds usize".into()))?;
        let len = usize::try_from(obj_length).map_err(|_| {
            Error::InvalidData("fractal heap object exceeds platform usize capacity".into())
        })?;
        let data_end = data_start
            .checked_add(len)
            .ok_or(Error::OffsetOutOfBounds(location.address))?;

        if data_end > block.len() {
            return Err(Error::UnexpectedEof {
                offset: location
                    .address
                    .checked_add(offset_in_block)
                    .ok_or(Error::OffsetOutOfBounds(location.address))?,
                needed: obj_length,
                available: block.len().saturating_sub(data_start) as u64,
            });
        }

        Ok(block[data_start..data_end].to_vec())
    }

    fn get_managed_object_storage_cached_impl(
        &self,
        heap_id: &[u8],
        storage: &dyn Storage,
        offset_size: u8,
        length_size: u8,
        direct_block_cache: &mut FractalHeapDirectBlockCache,
        filter_registry: Option<&FilterRegistry>,
    ) -> Result<Vec<u8>> {
        let (heap_offset, obj_length) = self.decode_managed_heap_id(heap_id)?;
        if obj_length == 0 {
            return Ok(Vec::new());
        }

        let location =
            self.find_direct_block_storage(heap_offset, storage, offset_size, length_size)?;
        let block = direct_block_cache.get_verified_block_storage(
            self,
            location,
            storage,
            offset_size,
            filter_registry,
        )?;
        let offset_in_block = heap_offset
            .checked_sub(location.block_offset_in_heap)
            .ok_or_else(|| {
                Error::InvalidData("fractal heap object precedes direct block".into())
            })?;
        let start = usize::try_from(offset_in_block)
            .map_err(|_| Error::InvalidData("fractal heap object offset exceeds usize".into()))?;
        let len = usize::try_from(obj_length).map_err(|_| {
            Error::InvalidData("fractal heap object exceeds platform usize capacity".into())
        })?;
        let end = start
            .checked_add(len)
            .ok_or(Error::OffsetOutOfBounds(location.address))?;
        if end > block.len() {
            let data_start = location
                .address
                .checked_add(offset_in_block)
                .ok_or(Error::OffsetOutOfBounds(location.address))?;
            return Err(Error::UnexpectedEof {
                offset: data_start,
                needed: obj_length,
                available: block.len().saturating_sub(start) as u64,
            });
        }
        Ok(block[start..end].to_vec())
    }

    fn get_huge_object(
        &self,
        heap_id: &[u8],
        file_data: &[u8],
        offset_size: u8,
        length_size: u8,
        filter_registry: Option<&FilterRegistry>,
    ) -> Result<Vec<u8>> {
        let location = self.resolve_huge_object_location(
            heap_id,
            Some(file_data),
            None,
            offset_size,
            length_size,
        )?;
        let start = usize::try_from(location.address)
            .map_err(|_| Error::OffsetOutOfBounds(location.address))?;
        let len = usize::try_from(location.disk_length).map_err(|_| {
            Error::InvalidData("huge fractal heap object exceeds platform usize capacity".into())
        })?;
        let end = start
            .checked_add(len)
            .ok_or(Error::OffsetOutOfBounds(location.address))?;
        if end > file_data.len() {
            return Err(Error::UnexpectedEof {
                offset: location.address,
                needed: location.disk_length,
                available: file_data.len().saturating_sub(start) as u64,
            });
        }
        self.decode_huge_object_bytes(&file_data[start..end], location, filter_registry)
    }

    fn get_huge_object_storage(
        &self,
        heap_id: &[u8],
        storage: &dyn Storage,
        offset_size: u8,
        length_size: u8,
        filter_registry: Option<&FilterRegistry>,
    ) -> Result<Vec<u8>> {
        let location = self.resolve_huge_object_location(
            heap_id,
            None,
            Some(storage),
            offset_size,
            length_size,
        )?;
        let len = usize::try_from(location.disk_length).map_err(|_| {
            Error::InvalidData("huge fractal heap object exceeds platform usize capacity".into())
        })?;
        let bytes = storage.read_range(location.address, len)?;
        self.decode_huge_object_bytes(bytes.as_ref(), location, filter_registry)
    }

    fn decode_huge_object_bytes(
        &self,
        bytes: &[u8],
        location: HugeObjectLocation,
        filter_registry: Option<&FilterRegistry>,
    ) -> Result<Vec<u8>> {
        if let Some(memory_length) = location.memory_length {
            self.apply_heap_filters(
                bytes,
                location.filter_mask,
                memory_length,
                "filtered fractal heap huge object",
                filter_registry,
            )
        } else {
            Ok(bytes.to_vec())
        }
    }

    fn load_direct_block(
        &self,
        file_data: &[u8],
        location: DirectBlockLocation,
        filter_registry: Option<&FilterRegistry>,
    ) -> Result<Vec<u8>> {
        let read_len = location.filtered_size.unwrap_or(location.block_size);
        let start = usize::try_from(location.address)
            .map_err(|_| Error::OffsetOutOfBounds(location.address))?;
        let len = usize::try_from(read_len).map_err(|_| {
            Error::InvalidData("fractal heap direct block size exceeds platform usize".into())
        })?;
        let end = start
            .checked_add(len)
            .ok_or(Error::OffsetOutOfBounds(location.address))?;
        if end > file_data.len() {
            return Err(Error::UnexpectedEof {
                offset: location.address,
                needed: read_len,
                available: file_data.len().saturating_sub(start) as u64,
            });
        }
        let block = if location.filtered_size.is_some() {
            self.apply_heap_filters(
                &file_data[start..end],
                location.filter_mask,
                location.block_size,
                "filtered fractal heap direct block",
                filter_registry,
            )?
        } else {
            file_data[start..end].to_vec()
        };
        let expected = usize::try_from(location.block_size).map_err(|_| {
            Error::InvalidData("fractal heap direct block size exceeds platform usize".into())
        })?;
        if block.len() != expected {
            return Err(Error::InvalidData(format!(
                "fractal heap direct block has {} bytes, expected {} bytes",
                block.len(),
                expected
            )));
        }
        Ok(block)
    }

    fn load_direct_block_storage(
        &self,
        location: DirectBlockLocation,
        storage: &dyn Storage,
        _offset_size: u8,
        filter_registry: Option<&FilterRegistry>,
    ) -> Result<Vec<u8>> {
        let read_len = location.filtered_size.unwrap_or(location.block_size);
        let len = usize::try_from(read_len).map_err(|_| {
            Error::InvalidData("fractal heap direct block size exceeds platform usize".into())
        })?;
        let bytes = storage.read_range(location.address, len)?;
        let block = if location.filtered_size.is_some() {
            self.apply_heap_filters(
                bytes.as_ref(),
                location.filter_mask,
                location.block_size,
                "filtered fractal heap direct block",
                filter_registry,
            )?
        } else {
            bytes.to_vec()
        };
        let expected = usize::try_from(location.block_size).map_err(|_| {
            Error::InvalidData("fractal heap direct block size exceeds platform usize".into())
        })?;
        if block.len() != expected {
            return Err(Error::InvalidData(format!(
                "fractal heap direct block has {} bytes, expected {} bytes",
                block.len(),
                expected
            )));
        }
        Ok(block)
    }

    fn apply_heap_filters(
        &self,
        bytes: &[u8],
        filter_mask: u32,
        expected_len: u64,
        context: &str,
        filter_registry: Option<&FilterRegistry>,
    ) -> Result<Vec<u8>> {
        let pipeline = self.filter_pipeline()?;
        let expected = usize::try_from(expected_len).map_err(|_| {
            Error::InvalidData(format!("{context} size exceeds platform usize capacity"))
        })?;
        let filter_output_limit = expected.checked_add(1).ok_or_else(|| {
            Error::InvalidData(format!(
                "{context} filter output limit exceeds platform usize capacity"
            ))
        })?;
        let decoded = filters::apply_pipeline_with_limit(
            bytes,
            &pipeline.filters,
            filter_mask,
            1,
            filter_registry,
            Some(filter_output_limit),
        )?;
        if decoded.len() != expected {
            return Err(Error::InvalidData(format!(
                "{context} decoded to {} bytes, expected {} bytes",
                decoded.len(),
                expected
            )));
        }
        Ok(decoded)
    }

    fn filter_pipeline(&self) -> Result<FilterPipelineMessage> {
        if self.io_filters_len == 0 {
            return Err(Error::InvalidData(
                "fractal heap object is marked filtered but the heap has no filter pipeline".into(),
            ));
        }
        let mut cursor = Cursor::new(&self.io_filter_info);
        crate::messages::filter_pipeline::parse(&mut cursor, 0, 0, self.io_filter_info.len())
    }

    fn resolve_huge_object_location(
        &self,
        heap_id: &[u8],
        file_data: Option<&[u8]>,
        storage: Option<&dyn Storage>,
        offset_size: u8,
        length_size: u8,
    ) -> Result<HugeObjectLocation> {
        let direct_unfiltered_len = 1 + usize::from(offset_size) + usize::from(length_size);
        let direct_filtered_len = direct_unfiltered_len + 4 + usize::from(length_size);

        if self.io_filters_len > 0 && heap_id.len() >= direct_filtered_len {
            let mut cursor = Cursor::new(&heap_id[1..]);
            let address = cursor.read_offset(offset_size)?;
            let disk_length = cursor.read_length(length_size)?;
            let filter_mask = cursor.read_u32_le()?;
            let memory_length = cursor.read_length(length_size)?;
            return Ok(HugeObjectLocation {
                address,
                disk_length,
                filter_mask,
                memory_length: Some(memory_length),
            });
        }

        if self.io_filters_len == 0 && heap_id.len() >= direct_unfiltered_len {
            let mut cursor = Cursor::new(&heap_id[1..]);
            let address = cursor.read_offset(offset_size)?;
            let disk_length = cursor.read_length(length_size)?;
            return Ok(HugeObjectLocation {
                address,
                disk_length,
                filter_mask: 0,
                memory_length: None,
            });
        }

        // HDF5 (H5HFhuge.c) keys an indirect huge object by
        // min(id_len - 1, sizeof(hsize_t)) bytes.
        let key_len = (heap_id.len().saturating_sub(1)).min(8);
        if key_len == 0 {
            return Err(Error::InvalidData(
                "huge fractal heap ID is too short".into(),
            ));
        }
        if Cursor::is_undefined_offset(self.btree_huge_objects_address, offset_size) {
            return Err(Error::UndefinedAddress);
        }

        let mut key_cursor = Cursor::new(&heap_id[1..]);
        let object_id = key_cursor.read_uvar(key_len)?;

        let header = if let Some(storage) = storage {
            crate::btree_v2::BTreeV2Header::parse_at_storage(
                storage,
                self.btree_huge_objects_address,
                offset_size,
                length_size,
            )?
        } else {
            let data = file_data.expect("file_data must exist when storage is None");
            let mut cursor = Cursor::new(data);
            cursor.set_position(self.btree_huge_objects_address);
            crate::btree_v2::BTreeV2Header::parse(&mut cursor, offset_size, length_size)?
        };

        let records = if let Some(storage) = storage {
            crate::btree_v2::collect_btree_v2_records_storage(
                storage,
                &header,
                offset_size,
                length_size,
                None,
                &[],
                None,
            )?
        } else {
            crate::btree_v2::collect_btree_v2_records(
                file_data.expect("file_data must exist when storage is None"),
                &header,
                offset_size,
                length_size,
                None,
                &[],
                None,
            )?
        };

        for record in records {
            match record {
                crate::btree_v2::BTreeV2Record::HugeIndirectNonFiltered {
                    address,
                    length,
                    object_id: record_id,
                } if record_id == object_id => {
                    return Ok(HugeObjectLocation {
                        address,
                        disk_length: length,
                        filter_mask: 0,
                        memory_length: None,
                    })
                }
                crate::btree_v2::BTreeV2Record::HugeIndirectFiltered {
                    object_id: record_id,
                    address,
                    filtered_length,
                    filter_mask,
                    memory_length,
                } if record_id == object_id => {
                    return Ok(HugeObjectLocation {
                        address,
                        disk_length: filtered_length,
                        filter_mask,
                        memory_length: Some(memory_length),
                    });
                }
                _ => {}
            }
        }

        Err(Error::InvalidData(format!(
            "huge fractal heap object ID {} not found",
            object_id
        )))
    }

    fn decode_tiny_object(&self, heap_id: &[u8]) -> Result<Vec<u8>> {
        let extended = self.heap_id_len > 18;
        let (data_start, len) = if extended {
            if heap_id.len() < 2 {
                return Err(Error::InvalidData(
                    "extended tiny heap ID is too short".into(),
                ));
            }
            let encoded = (u16::from(heap_id[0] & 0x0F) << 8) | u16::from(heap_id[1]);
            (2usize, usize::from(encoded) + 1)
        } else {
            (1usize, usize::from(heap_id[0] & 0x0F) + 1)
        };
        let data_end = data_start
            .checked_add(len)
            .ok_or_else(|| Error::InvalidData("tiny heap object length overflows".into()))?;
        if data_end > heap_id.len() {
            return Err(Error::InvalidData(format!(
                "tiny heap object needs {} bytes, heap ID has {}",
                data_end,
                heap_id.len()
            )));
        }
        Ok(heap_id[data_start..data_end].to_vec())
    }

    fn heap_id_kind(&self, heap_id: &[u8]) -> Result<HeapIdKind> {
        if heap_id.is_empty() {
            return Err(Error::InvalidData("empty fractal heap ID".into()));
        }
        let version = heap_id[0] >> 6;
        if version != 0 {
            return Err(Error::InvalidData(format!(
                "unsupported fractal heap ID version {}",
                version
            )));
        }
        match (heap_id[0] >> 4) & 0x03 {
            0 => Ok(HeapIdKind::Managed),
            1 => Ok(HeapIdKind::Huge),
            2 => Ok(HeapIdKind::Tiny),
            other => Err(Error::InvalidData(format!(
                "unknown fractal heap ID type {}",
                other
            ))),
        }
    }

    fn decode_managed_heap_id(&self, heap_id: &[u8]) -> Result<(u64, u64)> {
        let (offset_bytes, length_bytes) = self.managed_id_widths();
        let needed = 1 + offset_bytes + length_bytes;
        if heap_id.len() < needed {
            return Err(Error::InvalidData(format!(
                "managed fractal heap ID too short: need {} bytes, have {}",
                needed,
                heap_id.len()
            )));
        }
        let mut cursor = Cursor::new(&heap_id[1..needed]);
        let heap_offset = cursor.read_uvar(offset_bytes)?;
        let obj_length = cursor.read_uvar(length_bytes)?;
        Ok((heap_offset, obj_length))
    }

    fn managed_id_widths(&self) -> (usize, usize) {
        let offset_bytes = usize::from(self.max_heap_size).div_ceil(8).max(1);
        let max_len = self.max_direct_block_size.min(self.max_managed_object_size);
        let length_bytes = bytes_needed_to_encode(max_len).max(1);
        (offset_bytes, length_bytes)
    }

    /// Find the direct block containing a given heap offset.
    ///
    /// Returns (block_file_address, block_offset_within_heap, block_size).
    fn find_direct_block(
        &self,
        heap_offset: u64,
        file_data: &[u8],
        offset_size: u8,
        length_size: u8,
    ) -> Result<DirectBlockLocation> {
        if Cursor::is_undefined_offset(self.root_block_address, offset_size) {
            return Err(Error::UndefinedAddress);
        }

        if self.current_rows_in_root_indirect == 0 {
            // Root block is a direct block.
            // The entire managed space is in this one block.
            Ok(DirectBlockLocation {
                address: self.root_block_address,
                block_offset_in_heap: 0,
                block_size: self.starting_block_size,
                filtered_size: self.io_filter_size,
                filter_mask: self.io_filter_mask.unwrap_or(0),
            })
        } else {
            // Root block is an indirect block — traverse the doubling table.
            let mut visited = HashSet::new();
            self.find_direct_block_via_indirect(
                self.root_block_address,
                heap_offset,
                file_data,
                offset_size,
                length_size,
                self.current_rows_in_root_indirect,
                MAX_FRACTAL_HEAP_INDIRECT_DEPTH,
                &mut visited,
            )
        }
    }

    fn find_direct_block_storage(
        &self,
        heap_offset: u64,
        storage: &dyn Storage,
        offset_size: u8,
        length_size: u8,
    ) -> Result<DirectBlockLocation> {
        if Cursor::is_undefined_offset(self.root_block_address, offset_size) {
            return Err(Error::UndefinedAddress);
        }

        if self.current_rows_in_root_indirect == 0 {
            Ok(DirectBlockLocation {
                address: self.root_block_address,
                block_offset_in_heap: 0,
                block_size: self.starting_block_size,
                filtered_size: self.io_filter_size,
                filter_mask: self.io_filter_mask.unwrap_or(0),
            })
        } else {
            let mut visited = HashSet::new();
            self.find_direct_block_via_indirect_storage(
                self.root_block_address,
                heap_offset,
                storage,
                offset_size,
                length_size,
                self.current_rows_in_root_indirect,
                MAX_FRACTAL_HEAP_INDIRECT_DEPTH,
                &mut visited,
            )
        }
    }

    /// Traverse an indirect block to find the direct block for a given offset.
    #[allow(clippy::too_many_arguments)]
    fn find_direct_block_via_indirect(
        &self,
        indirect_address: u64,
        heap_offset: u64,
        file_data: &[u8],
        offset_size: u8,
        length_size: u8,
        nrows: u16,
        depth_remaining: usize,
        visited: &mut HashSet<u64>,
    ) -> Result<DirectBlockLocation> {
        self.enter_indirect_block(indirect_address, nrows, depth_remaining, visited)?;

        // Validate FHIB signature
        let addr = usize::try_from(indirect_address)
            .map_err(|_| Error::OffsetOutOfBounds(indirect_address))?;
        if addr
            .checked_add(4)
            .map_or(true, |end| end > file_data.len())
        {
            return Err(Error::OffsetOutOfBounds(indirect_address));
        }
        if file_data[addr..addr + 4] != FHIB_SIGNATURE {
            return Err(Error::InvalidData(format!(
                "expected FHIB signature at offset {:#x}, got {:?}",
                indirect_address,
                &file_data[addr..addr + 4]
            )));
        }

        // The doubling table has `table_width` entries per row.
        // Row 0 and 1 have blocks of size `starting_block_size`.
        // Row r (for r >= 1) has blocks of size `starting_block_size * 2^(r-1)`.
        //
        // We iterate through the rows to find which block contains the
        // target offset, then read the block address from the indirect block.

        let width = self.table_width as u64;
        let mut running_offset: u64 = 0;

        for row in 0..nrows as u64 {
            let block_size = self.block_size_for_row_checked(row)?;
            let is_direct = block_size <= self.max_direct_block_size;

            for col in 0..width {
                let block_end = running_offset.checked_add(block_size).ok_or_else(|| {
                    Error::InvalidData("fractal heap indirect block offset overflows u64".into())
                })?;
                if heap_offset >= running_offset && heap_offset < block_end {
                    // This is the block we want. Read its address from the
                    // indirect block.
                    let entry_index = row * width + col;

                    // Indirect block layout: signature(4) + version(1) +
                    // heap_header_addr(offset_size) + block_offset(max_heap_size/8 rounded up)
                    // Then direct-block entries, optionally with filtered size
                    // and mask, followed by child indirect-block addresses.
                    let iblock_header_size =
                        4 + 1 + offset_size as u64 + (self.max_heap_size as u64).div_ceil(8);

                    if is_direct {
                        let entry_size = self.direct_block_entry_size(offset_size, length_size);
                        let entry_offset =
                            entry_index.checked_mul(entry_size).ok_or_else(|| {
                                Error::InvalidData(
                                    "fractal heap direct entry offset overflows u64".into(),
                                )
                            })?;
                        let entry_pos = indirect_address
                            .checked_add(iblock_header_size)
                            .and_then(|pos| pos.checked_add(entry_offset))
                            .ok_or_else(|| {
                                Error::InvalidData(
                                    "fractal heap direct entry address overflows u64".into(),
                                )
                            })?;
                        let entry_len = usize::try_from(entry_size).map_err(|_| {
                            Error::InvalidData(
                                "fractal heap direct entry size exceeds platform usize".into(),
                            )
                        })?;
                        let entry_pos_usize = usize::try_from(entry_pos)
                            .map_err(|_| Error::OffsetOutOfBounds(entry_pos))?;
                        if entry_pos_usize
                            .checked_add(entry_len)
                            .map_or(true, |end| end > file_data.len())
                        {
                            return Err(Error::OffsetOutOfBounds(entry_pos));
                        }
                        let mut cursor = Cursor::new(file_data);
                        cursor.set_position(entry_pos);
                        let block_address = cursor.read_offset(offset_size)?;
                        if Cursor::is_undefined_offset(block_address, offset_size) {
                            return Err(Error::UndefinedAddress);
                        }
                        let (filtered_size, filter_mask) = if self.io_filters_len > 0 {
                            (
                                Some(cursor.read_length(length_size)?),
                                cursor.read_u32_le()?,
                            )
                        } else {
                            (None, 0)
                        };
                        return Ok(DirectBlockLocation {
                            address: block_address,
                            block_offset_in_heap: running_offset,
                            block_size,
                            filtered_size,
                            filter_mask,
                        });
                    } else {
                        // Need to recurse into a sub-indirect block.
                        let direct_count = self
                            .max_direct_block_rows_checked()?
                            .checked_mul(u64::from(self.table_width))
                            .ok_or_else(|| {
                                Error::InvalidData(
                                    "fractal heap direct entry count overflows u64".into(),
                                )
                            })?;
                        let indirect_index =
                            entry_index.checked_sub(direct_count).ok_or_else(|| {
                                Error::InvalidData(
                                    "fractal heap indirect entry precedes direct entries".into(),
                                )
                            })?;
                        let direct_entry_bytes = direct_count
                            .checked_mul(self.direct_block_entry_size(offset_size, length_size))
                            .ok_or_else(|| {
                                Error::InvalidData(
                                    "fractal heap direct entry table size overflows u64".into(),
                                )
                            })?;
                        let indirect_entry_offset = indirect_index
                            .checked_mul(u64::from(offset_size))
                            .ok_or_else(|| {
                                Error::InvalidData(
                                    "fractal heap indirect entry offset overflows u64".into(),
                                )
                            })?;
                        let entry_pos = indirect_address
                            .checked_add(iblock_header_size)
                            .and_then(|pos| pos.checked_add(direct_entry_bytes))
                            .and_then(|pos| pos.checked_add(indirect_entry_offset))
                            .ok_or_else(|| {
                                Error::InvalidData(
                                    "fractal heap indirect entry address overflows u64".into(),
                                )
                            })?;
                        let entry_pos_usize = usize::try_from(entry_pos)
                            .map_err(|_| Error::OffsetOutOfBounds(entry_pos))?;
                        if entry_pos_usize
                            .checked_add(usize::from(offset_size))
                            .map_or(true, |end| end > file_data.len())
                        {
                            return Err(Error::OffsetOutOfBounds(entry_pos));
                        }
                        let mut cursor = Cursor::new(file_data);
                        cursor.set_position(entry_pos);
                        let block_address = cursor.read_offset(offset_size)?;
                        if Cursor::is_undefined_offset(block_address, offset_size) {
                            return Err(Error::UndefinedAddress);
                        }
                        // Determine how many rows the sub-indirect has.
                        let sub_rows = self.rows_for_block_size_checked(block_size)?;
                        return self.find_direct_block_via_indirect(
                            block_address,
                            heap_offset - running_offset,
                            file_data,
                            offset_size,
                            length_size,
                            sub_rows,
                            depth_remaining - 1,
                            visited,
                        );
                    }
                }
                running_offset = block_end;
            }
        }

        Err(Error::InvalidData(format!(
            "fractal heap offset {} not found in doubling table",
            heap_offset
        )))
    }

    #[allow(clippy::too_many_arguments)]
    fn find_direct_block_via_indirect_storage(
        &self,
        indirect_address: u64,
        heap_offset: u64,
        storage: &dyn Storage,
        offset_size: u8,
        length_size: u8,
        nrows: u16,
        depth_remaining: usize,
        visited: &mut HashSet<u64>,
    ) -> Result<DirectBlockLocation> {
        self.enter_indirect_block(indirect_address, nrows, depth_remaining, visited)?;

        let sig = storage.read_range(indirect_address, 4)?;
        if sig.as_ref() != FHIB_SIGNATURE {
            return Err(Error::InvalidData(format!(
                "expected FHIB signature at offset {:#x}, got {:?}",
                indirect_address,
                sig.as_ref()
            )));
        }

        let width = self.table_width as u64;
        let mut running_offset = 0u64;

        for row in 0..u64::from(nrows) {
            let block_size = self.block_size_for_row_checked(row)?;
            let is_direct = block_size <= self.max_direct_block_size;

            for col in 0..width {
                let block_end = running_offset.checked_add(block_size).ok_or_else(|| {
                    Error::InvalidData("fractal heap indirect block offset overflows u64".into())
                })?;
                if heap_offset >= running_offset && heap_offset < block_end {
                    let entry_index = row * width + col;
                    let iblock_header_size = 4
                        + 1
                        + u64::from(offset_size)
                        + (u64::from(self.max_heap_size)).div_ceil(8);

                    if is_direct {
                        let entry_size = self.direct_block_entry_size(offset_size, length_size);
                        let entry_offset =
                            entry_index.checked_mul(entry_size).ok_or_else(|| {
                                Error::InvalidData(
                                    "fractal heap direct entry offset overflows u64".into(),
                                )
                            })?;
                        let entry_pos = indirect_address
                            .checked_add(iblock_header_size)
                            .and_then(|pos| pos.checked_add(entry_offset))
                            .ok_or_else(|| {
                                Error::InvalidData(
                                    "fractal heap direct entry address overflows u64".into(),
                                )
                            })?;
                        let entry_len = usize::try_from(entry_size).map_err(|_| {
                            Error::InvalidData(
                                "fractal heap direct entry size exceeds platform usize".into(),
                            )
                        })?;
                        let entry = storage.read_range(entry_pos, entry_len)?;
                        let mut cursor = Cursor::new(entry.as_ref());
                        let block_address = cursor.read_offset(offset_size)?;
                        if Cursor::is_undefined_offset(block_address, offset_size) {
                            return Err(Error::UndefinedAddress);
                        }
                        let (filtered_size, filter_mask) = if self.io_filters_len > 0 {
                            (
                                Some(cursor.read_length(length_size)?),
                                cursor.read_u32_le()?,
                            )
                        } else {
                            (None, 0)
                        };
                        return Ok(DirectBlockLocation {
                            address: block_address,
                            block_offset_in_heap: running_offset,
                            block_size,
                            filtered_size,
                            filter_mask,
                        });
                    }

                    let direct_count = self
                        .max_direct_block_rows_checked()?
                        .checked_mul(u64::from(self.table_width))
                        .ok_or_else(|| {
                            Error::InvalidData(
                                "fractal heap direct entry count overflows u64".into(),
                            )
                        })?;
                    let indirect_index =
                        entry_index.checked_sub(direct_count).ok_or_else(|| {
                            Error::InvalidData(
                                "fractal heap indirect entry precedes direct entries".into(),
                            )
                        })?;
                    let direct_entry_bytes = direct_count
                        .checked_mul(self.direct_block_entry_size(offset_size, length_size))
                        .ok_or_else(|| {
                            Error::InvalidData(
                                "fractal heap direct entry table size overflows u64".into(),
                            )
                        })?;
                    let indirect_entry_offset = indirect_index
                        .checked_mul(u64::from(offset_size))
                        .ok_or_else(|| {
                            Error::InvalidData(
                                "fractal heap indirect entry offset overflows u64".into(),
                            )
                        })?;
                    let entry_addr_pos = indirect_address
                        .checked_add(iblock_header_size)
                        .and_then(|pos| pos.checked_add(direct_entry_bytes))
                        .and_then(|pos| pos.checked_add(indirect_entry_offset))
                        .ok_or_else(|| {
                            Error::InvalidData(
                                "fractal heap indirect entry address overflows u64".into(),
                            )
                        })?;
                    let entry = storage.read_range(entry_addr_pos, usize::from(offset_size))?;
                    let mut cursor = Cursor::new(entry.as_ref());
                    let block_address = cursor.read_offset(offset_size)?;
                    if Cursor::is_undefined_offset(block_address, offset_size) {
                        return Err(Error::UndefinedAddress);
                    }

                    let sub_rows = self.rows_for_block_size_checked(block_size)?;
                    return self.find_direct_block_via_indirect_storage(
                        block_address,
                        heap_offset - running_offset,
                        storage,
                        offset_size,
                        length_size,
                        sub_rows,
                        depth_remaining - 1,
                        visited,
                    );
                }
                running_offset = block_end;
            }
        }

        Err(Error::InvalidData(format!(
            "fractal heap offset {} not found in doubling table",
            heap_offset
        )))
    }

    fn enter_indirect_block(
        &self,
        indirect_address: u64,
        nrows: u16,
        depth_remaining: usize,
        visited: &mut HashSet<u64>,
    ) -> Result<()> {
        if depth_remaining == 0 {
            return Err(Error::InvalidData(
                "fractal heap indirect traversal exceeded recursion limit".into(),
            ));
        }
        if nrows > MAX_FRACTAL_HEAP_INDIRECT_ROWS {
            return Err(Error::InvalidData(format!(
                "fractal heap indirect block has {} rows, limit is {}",
                nrows, MAX_FRACTAL_HEAP_INDIRECT_ROWS
            )));
        }
        if !visited.insert(indirect_address) {
            return Err(Error::InvalidData(format!(
                "fractal heap indirect traversal revisits block at offset {:#x}",
                indirect_address
            )));
        }
        Ok(())
    }

    /// Compute the block size for a given row in the doubling table.
    fn block_size_for_row_checked(&self, row: u64) -> Result<u64> {
        if row == 0 {
            Ok(self.starting_block_size)
        } else {
            let shift = u32::try_from(row - 1)
                .map_err(|_| Error::InvalidData("fractal heap indirect row exceeds u32".into()))?;
            let factor = 1u64.checked_shl(shift).ok_or_else(|| {
                Error::InvalidData("fractal heap indirect row shift overflows u64".into())
            })?;
            self.starting_block_size.checked_mul(factor).ok_or_else(|| {
                Error::InvalidData("fractal heap indirect block size overflows u64".into())
            })
        }
    }

    /// Compute how many rows of the doubling table fit in a block of the
    /// given total size.
    fn rows_for_block_size_checked(&self, total_size: u64) -> Result<u16> {
        let mut rows = 0u16;
        let mut accum = 0u64;
        let width = u64::from(self.table_width);
        loop {
            if rows >= MAX_FRACTAL_HEAP_INDIRECT_ROWS {
                break;
            }
            let bs = self.block_size_for_row_checked(u64::from(rows))?;
            let row_total = bs.checked_mul(width).ok_or_else(|| {
                Error::InvalidData("fractal heap indirect row size overflows u64".into())
            })?;
            let next = accum.checked_add(row_total).ok_or_else(|| {
                Error::InvalidData("fractal heap indirect table size overflows u64".into())
            })?;
            if next > total_size {
                break;
            }
            accum = next;
            rows = rows.checked_add(1).ok_or_else(|| {
                Error::InvalidData("fractal heap indirect row count overflows u16".into())
            })?;
        }
        Ok(rows)
    }

    fn max_direct_block_rows_checked(&self) -> Result<u64> {
        let mut rows = 0u64;
        loop {
            if rows >= u64::from(MAX_FRACTAL_HEAP_INDIRECT_ROWS) {
                break;
            }
            if self.block_size_for_row_checked(rows)? > self.max_direct_block_size {
                break;
            }
            rows += 1;
        }
        Ok(rows)
    }

    fn direct_block_entry_size(&self, offset_size: u8, length_size: u8) -> u64 {
        let mut size = u64::from(offset_size);
        if self.io_filters_len > 0 {
            size += u64::from(length_size) + 4;
        }
        size
    }

    /// Size in bytes of an unfiltered direct block header.
    fn direct_block_header_size(&self, offset_size: u8) -> usize {
        // Signature(4) + Version(1) + Heap header address(offset_size) +
        // Block offset within heap (max_heap_size bits, rounded up to bytes) +
        // optional Checksum(4).
        let offset_bytes = (self.max_heap_size as usize).div_ceil(8);
        let checksum_bytes = if self.direct_blocks_are_checksummed() {
            4
        } else {
            0
        };
        4 + 1 + offset_size as usize + offset_bytes + checksum_bytes
    }

    fn direct_blocks_are_checksummed(&self) -> bool {
        self.flags & 0x02 != 0
    }

    fn direct_block_checksum_pos(&self, offset_size: u8) -> Option<usize> {
        if self.direct_blocks_are_checksummed() {
            Some(self.direct_block_header_size(offset_size) - 4)
        } else {
            None
        }
    }

    fn verify_direct_block_bytes(&self, block: &[u8], offset_size: u8) -> Result<()> {
        if block.len() < self.direct_block_header_size(offset_size) {
            return Err(Error::InvalidData(format!(
                "fractal heap direct block has {} bytes, expected at least {}",
                block.len(),
                self.direct_block_header_size(offset_size)
            )));
        }
        if block[..4] != _FHDB_SIGNATURE {
            return Err(Error::InvalidData(
                "invalid fractal heap direct block signature".into(),
            ));
        }
        let version = block[4];
        if version != 0 {
            return Err(Error::UnsupportedFractalHeapVersion(version));
        }
        if let Some(checksum_pos) = self.direct_block_checksum_pos(offset_size) {
            let stored_checksum = u32::from_le_bytes(
                block[checksum_pos..checksum_pos + 4]
                    .try_into()
                    .expect("direct block checksum slice has four bytes"),
            );
            let mut checksum_data = block.to_vec();
            checksum_data[checksum_pos..checksum_pos + 4].fill(0);
            let computed = jenkins_lookup3(&checksum_data);
            if computed != stored_checksum {
                return Err(Error::ChecksumMismatch {
                    expected: stored_checksum,
                    actual: computed,
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeapIdKind {
    Managed,
    Huge,
    Tiny,
}

fn bytes_needed_to_encode(value: u64) -> usize {
    if value <= u8::MAX as u64 {
        1
    } else if value <= u16::MAX as u64 {
        2
    } else if value <= 0x00FF_FFFF {
        3
    } else if value <= u32::MAX as u64 {
        4
    } else if value <= 0x00FF_FFFF_FFFF {
        5
    } else if value <= 0x0000_FFFF_FFFF_FFFF {
        6
    } else if value <= 0x00FF_FFFF_FFFF_FFFF {
        7
    } else {
        8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Storage, StorageBuffer};
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    fn base_heap() -> FractalHeap {
        FractalHeap {
            heap_id_len: 8,
            io_filters_len: 0,
            flags: 0x02,
            max_managed_object_size: 128,
            next_huge_id: 0,
            btree_huge_objects_address: u64::MAX,
            free_space_managed_address: 0,
            managed_space_amount: 0,
            managed_alloc_amount: 0,
            managed_iter_offset: 0,
            managed_objects_count: 0,
            huge_objects_size: 0,
            huge_objects_count: 0,
            tiny_objects_size: 0,
            tiny_objects_count: 0,
            table_width: 4,
            starting_block_size: 256,
            max_direct_block_size: 4096,
            max_heap_size: 16,
            starting_row_root_indirect: 0,
            root_block_address: 0,
            current_rows_in_root_indirect: 0,
            io_filter_size: None,
            io_filter_mask: None,
            io_filter_info: Vec::new(),
        }
    }

    fn deflate_filter_info() -> Vec<u8> {
        let mut data = vec![0x02, 0x01];
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&6u32.to_le_bytes());
        data
    }

    fn zlib_compress(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn filtered_heap_with_info(filter_info: Vec<u8>) -> FractalHeap {
        let mut heap = base_heap();
        heap.io_filters_len = filter_info.len() as u16;
        heap.io_filter_info = filter_info;
        heap
    }

    fn direct_block_with_object(heap: &FractalHeap, block_size: usize, obj: &[u8]) -> Vec<u8> {
        let offset_size = 8;
        let mut block = vec![0u8; block_size];
        block[0..4].copy_from_slice(b"FHDB");
        block[4] = 0;
        let obj_offset = heap.direct_block_header_size(offset_size);
        block[obj_offset..obj_offset + obj.len()].copy_from_slice(obj);
        if let Some(checksum_pos) = heap.direct_block_checksum_pos(offset_size) {
            let mut checksum_data = block.clone();
            checksum_data[checksum_pos..checksum_pos + 4].fill(0);
            let checksum = jenkins_lookup3(&checksum_data);
            block[checksum_pos..checksum_pos + 4].copy_from_slice(&checksum.to_le_bytes());
        }
        block
    }

    struct CountingStorage {
        data: Vec<u8>,
        reads: Arc<Mutex<Vec<(u64, usize)>>>,
    }

    impl Storage for CountingStorage {
        fn len(&self) -> u64 {
            self.data.len() as u64
        }

        fn read_range(&self, offset: u64, len: usize) -> Result<StorageBuffer> {
            self.reads.lock().unwrap().push((offset, len));
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
            Ok(StorageBuffer::from_vec(self.data[start..end].to_vec()))
        }
    }

    #[test]
    fn block_size_for_row_scales_by_table_width() {
        let heap = FractalHeap {
            heap_id_len: 8,
            io_filters_len: 0,
            flags: 0x02,
            max_managed_object_size: 0,
            next_huge_id: 0,
            btree_huge_objects_address: 0,
            free_space_managed_address: 0,
            managed_space_amount: 0,
            managed_alloc_amount: 0,
            managed_iter_offset: 0,
            managed_objects_count: 0,
            huge_objects_size: 0,
            huge_objects_count: 0,
            tiny_objects_size: 0,
            tiny_objects_count: 0,
            table_width: 4,
            starting_block_size: 256,
            max_direct_block_size: 4096,
            max_heap_size: 16,
            starting_row_root_indirect: 0,
            root_block_address: 0,
            current_rows_in_root_indirect: 0,
            io_filter_size: None,
            io_filter_mask: None,
            io_filter_info: Vec::new(),
        };

        assert_eq!(heap.block_size_for_row_checked(0).unwrap(), 256);
        assert_eq!(heap.block_size_for_row_checked(1).unwrap(), 256); // 256 * 2^0
        assert_eq!(heap.block_size_for_row_checked(2).unwrap(), 512); // 256 * 2^1
        assert_eq!(heap.block_size_for_row_checked(3).unwrap(), 1024); // 256 * 2^2
    }

    #[test]
    fn find_direct_block_rejects_indirect_row_count_above_limit() {
        let mut heap = base_heap();
        heap.root_block_address = 0;
        heap.current_rows_in_root_indirect = MAX_FRACTAL_HEAP_INDIRECT_ROWS + 1;

        let err = heap.find_direct_block(0, &[], 8, 8).unwrap_err();
        assert!(
            matches!(err, Error::InvalidData(ref message) if message.contains("limit")),
            "{err:?}"
        );

        let storage = crate::storage::BytesStorage::new(Vec::new());
        let err = heap
            .find_direct_block_storage(0, &storage, 8, 8)
            .unwrap_err();
        assert!(
            matches!(err, Error::InvalidData(ref message) if message.contains("limit")),
            "{err:?}"
        );
    }

    #[test]
    fn find_direct_block_rejects_cyclic_indirect_block() {
        let mut heap = base_heap();
        let indirect_address = 512u64;
        heap.table_width = 1;
        heap.starting_block_size = 256;
        heap.max_direct_block_size = 256;
        heap.max_heap_size = 64;
        heap.root_block_address = indirect_address;
        heap.current_rows_in_root_indirect = 3;

        let offset_size = 8u8;
        let length_size = 8u8;
        let offset_bytes = usize::from(heap.max_heap_size).div_ceil(8);
        let iblock_header_size = 4 + 1 + usize::from(offset_size) + offset_bytes;
        let direct_count = usize::try_from(
            heap.max_direct_block_rows_checked().unwrap() * u64::from(heap.table_width),
        )
        .unwrap();
        let direct_entry_size =
            usize::try_from(heap.direct_block_entry_size(offset_size, length_size)).unwrap();
        let indirect_entry_pos = iblock_header_size + direct_count * direct_entry_size;
        let mut indirect = vec![0u8; indirect_entry_pos + usize::from(offset_size)];
        indirect[0..4].copy_from_slice(b"FHIB");
        indirect[4] = 0;
        indirect[indirect_entry_pos..indirect_entry_pos + usize::from(offset_size)]
            .copy_from_slice(&indirect_address.to_le_bytes());

        let mut file_data = vec![0u8; indirect_address as usize + indirect.len()];
        file_data[indirect_address as usize..indirect_address as usize + indirect.len()]
            .copy_from_slice(&indirect);

        let err = heap
            .find_direct_block(600, &file_data, offset_size, length_size)
            .unwrap_err();
        assert!(
            matches!(err, Error::InvalidData(ref message) if message.contains("revisits block")),
            "{err:?}"
        );

        let storage = crate::storage::BytesStorage::new(file_data);
        let err = heap
            .find_direct_block_storage(600, &storage, offset_size, length_size)
            .unwrap_err();
        assert!(
            matches!(err, Error::InvalidData(ref message) if message.contains("revisits block")),
            "{err:?}"
        );
    }

    #[test]
    fn get_tiny_object() {
        let heap = base_heap();
        let heap_id = [0x20 | 3, b't', b'i', b'n', b'y'];
        let result = heap.get_object(&heap_id, &[], 8, 8).unwrap();
        assert_eq!(result, b"tiny");
    }

    #[test]
    fn get_huge_direct_object() {
        let heap = base_heap();
        let mut file_data = vec![0u8; 128];
        file_data[64..68].copy_from_slice(b"huge");

        let mut heap_id = Vec::new();
        heap_id.push(0x10);
        heap_id.extend_from_slice(&64u64.to_le_bytes());
        heap_id.extend_from_slice(&4u64.to_le_bytes());

        let result = heap.get_object(&heap_id, &file_data, 8, 8).unwrap();
        assert_eq!(result, b"huge");
    }

    #[test]
    fn get_filtered_huge_direct_object() {
        let heap = filtered_heap_with_info(deflate_filter_info());
        let payload = b"filtered huge payload";
        let compressed = zlib_compress(payload);
        let address = 64u64;
        let mut file_data = vec![0u8; address as usize + compressed.len()];
        file_data[address as usize..].copy_from_slice(&compressed);

        let mut heap_id = Vec::new();
        heap_id.push(0x10);
        heap_id.extend_from_slice(&address.to_le_bytes());
        heap_id.extend_from_slice(&(compressed.len() as u64).to_le_bytes());
        heap_id.extend_from_slice(&0u32.to_le_bytes());
        heap_id.extend_from_slice(&(payload.len() as u64).to_le_bytes());

        let result = heap.get_object(&heap_id, &file_data, 8, 8).unwrap();
        assert_eq!(result, payload);
    }

    #[test]
    fn get_filtered_managed_object_direct_root() {
        let mut heap = filtered_heap_with_info(deflate_filter_info());
        let block_address = 1000u64;
        let obj_data = b"filtered managed";
        let block = direct_block_with_object(&heap, heap.starting_block_size as usize, obj_data);
        let compressed = zlib_compress(&block);
        heap.root_block_address = block_address;
        heap.io_filter_size = Some(compressed.len() as u64);
        heap.io_filter_mask = Some(0);

        let file_size = block_address as usize + compressed.len();
        let mut file_data = vec![0u8; file_size];
        file_data[block_address as usize..].copy_from_slice(&compressed);

        let obj_offset = heap.direct_block_header_size(8) as u16;
        let mut heap_id = vec![0x00];
        heap_id.extend_from_slice(&obj_offset.to_le_bytes());
        heap_id.push(obj_data.len() as u8);

        let result = heap.get_object(&heap_id, &file_data, 8, 8).unwrap();
        assert_eq!(result, obj_data);
    }

    #[test]
    fn get_filtered_managed_object_from_indirect_block() {
        let mut heap = filtered_heap_with_info(deflate_filter_info());
        let indirect_address = 512u64;
        let block_address = 1000u64;
        let obj_data = b"filtered child";
        let block = direct_block_with_object(&heap, heap.starting_block_size as usize, obj_data);
        let compressed = zlib_compress(&block);

        heap.root_block_address = indirect_address;
        heap.current_rows_in_root_indirect = 1;

        let offset_bytes = usize::from(heap.max_heap_size).div_ceil(8);
        let iblock_header_size = 4 + 1 + 8 + offset_bytes;
        let direct_entry_size = 8 + 8 + 4;
        let mut indirect = vec![0u8; iblock_header_size + direct_entry_size * 4];
        indirect[0..4].copy_from_slice(b"FHIB");
        indirect[4] = 0;
        let entry_pos = iblock_header_size;
        indirect[entry_pos..entry_pos + 8].copy_from_slice(&block_address.to_le_bytes());
        indirect[entry_pos + 8..entry_pos + 16]
            .copy_from_slice(&(compressed.len() as u64).to_le_bytes());
        indirect[entry_pos + 16..entry_pos + 20].copy_from_slice(&0u32.to_le_bytes());

        let file_size = block_address as usize + compressed.len();
        let mut file_data = vec![0u8; file_size];
        file_data[indirect_address as usize..indirect_address as usize + indirect.len()]
            .copy_from_slice(&indirect);
        file_data[block_address as usize..].copy_from_slice(&compressed);
        let storage = crate::storage::BytesStorage::new(file_data);

        let obj_offset = heap.direct_block_header_size(8) as u16;
        let mut heap_id = vec![0x00];
        heap_id.extend_from_slice(&obj_offset.to_le_bytes());
        heap_id.push(obj_data.len() as u8);

        let result = heap.get_object_storage(&heap_id, &storage, 8, 8).unwrap();
        assert_eq!(result, obj_data);
    }

    #[test]
    fn direct_block_header_size_includes_optional_fields() {
        let heap = FractalHeap {
            heap_id_len: 8,
            io_filters_len: 0,
            flags: 0x02,
            max_managed_object_size: 0,
            next_huge_id: 0,
            btree_huge_objects_address: 0,
            free_space_managed_address: 0,
            managed_space_amount: 0,
            managed_alloc_amount: 0,
            managed_iter_offset: 0,
            managed_objects_count: 0,
            huge_objects_size: 0,
            huge_objects_count: 0,
            tiny_objects_size: 0,
            tiny_objects_count: 0,
            table_width: 4,
            starting_block_size: 256,
            max_direct_block_size: 4096,
            max_heap_size: 16,
            starting_row_root_indirect: 0,
            root_block_address: 0,
            current_rows_in_root_indirect: 0,
            io_filter_size: None,
            io_filter_mask: None,
            io_filter_info: Vec::new(),
        };

        // sig(4) + ver(1) + addr(8) + offset_bytes(2) + checksum(4) = 19
        assert_eq!(heap.direct_block_header_size(8), 19);

        // With 4-byte offsets: sig(4) + ver(1) + addr(4) + offset_bytes(2) + checksum(4) = 15
        assert_eq!(heap.direct_block_header_size(4), 15);
    }

    #[test]
    fn get_managed_object_direct_root() {
        // Set up a fractal heap where the root is a direct block.
        let offset_size: u8 = 8;
        let max_heap_size: u16 = 16;
        let starting_block_size: u64 = 256;

        // Direct block header size: sig(4) + ver(1) + addr(8) + offset_bytes(2) + checksum(4) = 19
        // (no I/O filters => checksum present)
        let db_header_size = 19usize;

        // Place the direct block at file offset 1000.
        let block_address: u64 = 1000;

        let heap = FractalHeap {
            heap_id_len: 8,
            io_filters_len: 0,
            flags: 0x02,
            max_managed_object_size: 128,
            next_huge_id: 0,
            btree_huge_objects_address: u64::MAX,
            free_space_managed_address: 0,
            managed_space_amount: starting_block_size,
            managed_alloc_amount: starting_block_size,
            managed_iter_offset: 0,
            managed_objects_count: 1,
            huge_objects_size: 0,
            huge_objects_count: 0,
            tiny_objects_size: 0,
            tiny_objects_count: 0,
            table_width: 4,
            starting_block_size,
            max_direct_block_size: 4096,
            max_heap_size,
            starting_row_root_indirect: 0,
            root_block_address: block_address,
            current_rows_in_root_indirect: 0,
            io_filter_size: None,
            io_filter_mask: None,
            io_filter_info: Vec::new(),
        };

        // Build file data with the direct block.
        let file_size = block_address as usize + starting_block_size as usize + 100;
        let mut file_data = vec![0u8; file_size];

        // Write direct block header at block_address.
        let ba = block_address as usize;
        file_data[ba..ba + 4].copy_from_slice(b"FHDB");
        file_data[ba + 4] = 0; // version
                               // heap header address (8 bytes) — doesn't matter for this test
                               // block offset (2 bytes) — 0

        // Write object data at its direct-block offset, after the block header.
        let obj_data = b"test object data";
        let obj_start = ba + db_header_size;
        file_data[obj_start..obj_start + obj_data.len()].copy_from_slice(obj_data);
        let checksum_pos = ba + db_header_size - 4;
        let mut checksum_data = file_data[ba..ba + starting_block_size as usize].to_vec();
        checksum_data[checksum_pos - ba..checksum_pos - ba + 4].fill(0);
        let checksum = jenkins_lookup3(&checksum_data);
        file_data[checksum_pos..checksum_pos + 4].copy_from_slice(&checksum.to_le_bytes());

        // Build heap ID for managed object at its direct-block offset, length=16.
        // Type nibble = 0, offset = direct block header size (16 bits), length = 16.
        let heap_id = [0x00, db_header_size as u8, 0x00, 0x10];

        let result = heap
            .get_managed_object(&heap_id, &file_data, offset_size, 8)
            .unwrap();
        assert_eq!(result, obj_data);
    }

    #[test]
    fn get_object_storage_cached_reads_direct_block_once() {
        let offset_size: u8 = 8;
        let max_heap_size: u16 = 16;
        let starting_block_size: u64 = 256;
        let db_header_size = 19usize;
        let block_address: u64 = 1000;

        let heap = FractalHeap {
            heap_id_len: 8,
            io_filters_len: 0,
            flags: 0x02,
            max_managed_object_size: 128,
            next_huge_id: 0,
            btree_huge_objects_address: u64::MAX,
            free_space_managed_address: 0,
            managed_space_amount: starting_block_size,
            managed_alloc_amount: starting_block_size,
            managed_iter_offset: 0,
            managed_objects_count: 2,
            huge_objects_size: 0,
            huge_objects_count: 0,
            tiny_objects_size: 0,
            tiny_objects_count: 0,
            table_width: 4,
            starting_block_size,
            max_direct_block_size: 4096,
            max_heap_size,
            starting_row_root_indirect: 0,
            root_block_address: block_address,
            current_rows_in_root_indirect: 0,
            io_filter_size: None,
            io_filter_mask: None,
            io_filter_info: Vec::new(),
        };

        let file_size = block_address as usize + starting_block_size as usize;
        let mut file_data = vec![0u8; file_size];
        let ba = block_address as usize;
        file_data[ba..ba + 4].copy_from_slice(b"FHDB");
        file_data[ba + 4] = 0;

        let obj1_offset = db_header_size;
        let obj2_offset = db_header_size + 16;
        file_data[ba + obj1_offset..ba + obj1_offset + 4].copy_from_slice(b"one!");
        file_data[ba + obj2_offset..ba + obj2_offset + 4].copy_from_slice(b"two!");

        let checksum_pos = ba + db_header_size - 4;
        let mut checksum_data = file_data[ba..ba + starting_block_size as usize].to_vec();
        checksum_data[checksum_pos - ba..checksum_pos - ba + 4].fill(0);
        let checksum = jenkins_lookup3(&checksum_data);
        file_data[checksum_pos..checksum_pos + 4].copy_from_slice(&checksum.to_le_bytes());

        let reads = Arc::new(Mutex::new(Vec::new()));
        let storage = CountingStorage {
            data: file_data,
            reads: reads.clone(),
        };
        let mut cache = FractalHeapDirectBlockCache::default();
        let heap_id1 = [0x00, obj1_offset as u8, 0x00, 0x04];
        let heap_id2 = [0x00, obj2_offset as u8, 0x00, 0x04];

        assert_eq!(
            heap.get_object_storage_cached(&heap_id1, &storage, offset_size, 8, &mut cache)
                .unwrap(),
            b"one!"
        );
        assert_eq!(
            heap.get_object_storage_cached(&heap_id2, &storage, offset_size, 8, &mut cache)
                .unwrap(),
            b"two!"
        );

        let direct_block_reads = reads
            .lock()
            .unwrap()
            .iter()
            .filter(|&&(offset, len)| {
                offset == block_address && len == starting_block_size as usize
            })
            .count();
        assert_eq!(direct_block_reads, 1);
    }
}
