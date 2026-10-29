use std::mem::MaybeUninit;
use std::num::NonZeroUsize;
use std::sync::{Arc, OnceLock};

use lru::LruCache;
use ndarray::{ArrayD, IxDyn};
use parking_lot::Mutex;
#[cfg(feature = "rayon")]
use rayon::prelude::*;
use smallvec::SmallVec;

use crate::attribute_api::{
    collect_attribute_messages_storage, decode_string, decoded_vlen_strings_storage,
    read_one_vlen_string_storage, resolve_vlen_bytes_storage, Attribute,
};
use crate::cache::{ChunkCache, ChunkCacheStats, ChunkKey};
use crate::chunk_index;
use crate::datatype_api::H5Type;
use crate::error::{ByteOrder, Error, Result};
use crate::filters::{self, FilterRegistry};
use crate::io::Cursor;
use crate::local_heap::LocalHeap;
use crate::messages::attribute::AttributeMessage;
use crate::messages::dataspace::{DataspaceMessage, DataspaceType};
use crate::messages::datatype::{Datatype, StringSize, VarLenKind};
use crate::messages::external_files::ExternalFilesMessage;
use crate::messages::fill_value::{FillTime, FillValueMessage};
use crate::messages::filter_pipeline::FilterPipelineMessage;
use crate::messages::layout::{ChunkIndexing, DataLayout};
use crate::messages::HdfMessage;
use crate::object_header::ObjectHeader;
use crate::storage::DynStorage;
use crate::FileContext;

const HOT_FULL_DATASET_CACHE_MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy)]
struct FlatBufferPtr {
    ptr: *mut u8,
    len: usize,
}

#[derive(Clone, Copy)]
struct ChunkCopyLayout<'a> {
    chunk_offsets: &'a [u64],
    chunk_shape: &'a [u64],
    dataset_shape: &'a [u64],
    dataset_strides: &'a [usize],
    chunk_strides: &'a [usize],
    elem_size: usize,
}

#[derive(Clone, Copy)]
struct UnitStrideCopyLayout<'a> {
    chunk_offsets: &'a [u64],
    chunk_shape: &'a [u64],
    dataset_shape: &'a [u64],
    resolved: &'a ResolvedSelection,
    chunk_strides: &'a [usize],
    result_strides: &'a [usize],
    elem_size: usize,
}

#[derive(Clone, Copy)]
struct ContiguousSliceDirectLayout<'a> {
    dataset_strides: &'a [usize],
    result_strides: &'a [usize],
    elem_size: usize,
    result_total_bytes: usize,
}

#[derive(Clone)]
struct ResolvedExternalRawSlot {
    logical_offset: u64,
    storage: DynStorage,
    file_offset: u64,
    size: u64,
}

pub(crate) struct DatasetParseContext {
    pub(crate) context: Arc<FileContext>,
}

#[derive(Clone, Copy)]
struct ChunkEntrySelection<'a> {
    shape: &'a [u64],
    ndim: usize,
    elem_size: usize,
    chunk_bounds: Option<(&'a [u64], &'a [u64])>,
}

// SAFETY: `FlatBufferPtr` has no safe dereference operations. Callers of its
// unsafe methods must keep the allocation alive and coordinate disjoint writes.
unsafe impl Send for FlatBufferPtr {}

// SAFETY: sharing the pointer does not itself access the allocation. Every
// access remains unsafe and requires callers to guarantee disjoint writes.
unsafe impl Sync for FlatBufferPtr {}

impl FlatBufferPtr {
    #[cfg(feature = "rayon")]
    #[inline(always)]
    unsafe fn copy_chunk(self, chunk_data: &[u8], layout: ChunkCopyLayout<'_>) -> Result<()> {
        // SAFETY: the caller upholds this method's allocation-lifetime and
        // disjoint-write requirements.
        unsafe { copy_chunk_to_flat_with_strides_ptr(chunk_data, self, layout) }
    }

    #[cfg(feature = "rayon")]
    #[inline(always)]
    unsafe fn copy_selected(
        self,
        chunk_data: &[u8],
        dim_indices: &[Vec<(usize, usize)>],
        chunk_strides: &[usize],
        result_strides: &[usize],
        elem_size: usize,
        ndim: usize,
    ) -> Result<()> {
        // SAFETY: the caller upholds this method's allocation-lifetime and
        // disjoint-write requirements.
        unsafe {
            copy_selected_elements_ptr(
                chunk_data,
                self.ptr,
                self.len,
                dim_indices,
                chunk_strides,
                result_strides,
                elem_size,
                ndim,
            )
        }
    }

    #[cfg(feature = "rayon")]
    #[inline(always)]
    unsafe fn copy_unit_stride_chunk_overlap(
        self,
        chunk_data: &[u8],
        layout: UnitStrideCopyLayout<'_>,
    ) -> Result<()> {
        // SAFETY: the caller upholds this method's allocation-lifetime and
        // disjoint-write requirements.
        unsafe { copy_unit_stride_chunk_overlap_ptr(chunk_data, self, layout) }
    }
}

/// Hyperslab selection for reading slices of datasets.
#[derive(Debug, Clone)]
pub struct SliceInfo {
    pub selections: Vec<SliceInfoElem>,
}

/// A single dimension's selection.
#[derive(Debug, Clone)]
pub enum SliceInfoElem {
    /// Select a single index (reduces dimensionality).
    Index(u64),
    /// Select a range with optional step.
    Slice { start: u64, end: u64, step: u64 },
}

#[derive(Clone, Debug)]
struct ResolvedSelectionDim {
    start: u64,
    end: u64,
    step: u64,
    count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ChunkEntryCacheKey {
    index_address: u64,
    first_chunk: SmallVec<[u64; 4]>,
    last_chunk: SmallVec<[u64; 4]>,
}

impl ResolvedSelectionDim {
    fn chunk_index_range(&self, chunk_extent: u64) -> Option<(u64, u64)> {
        if self.count == 0 {
            return None;
        }

        Some((self.start / chunk_extent, (self.end - 1) / chunk_extent))
    }
}

#[derive(Clone, Debug)]
struct ResolvedSelection {
    dims: Vec<ResolvedSelectionDim>,
    result_shape: Vec<usize>,
    result_elements: usize,
}

impl ResolvedSelection {
    fn result_dims_with_collapsed(&self) -> Vec<usize> {
        self.dims.iter().map(|dim| dim.count).collect()
    }

    fn is_unit_stride(&self) -> bool {
        self.dims.iter().all(|dim| dim.step == 1)
    }
}

impl SliceInfo {
    /// Create a selection that reads everything.
    pub fn all(ndim: usize) -> Self {
        SliceInfo {
            selections: vec![
                SliceInfoElem::Slice {
                    start: 0,
                    end: u64::MAX,
                    step: 1,
                };
                ndim
            ],
        }
    }
}

fn checked_usize(value: u64, context: &str) -> Result<usize> {
    usize::try_from(value).map_err(|_| {
        Error::InvalidData(format!(
            "{context} value {value} exceeds platform usize capacity"
        ))
    })
}

fn checked_mul_usize(lhs: usize, rhs: usize, context: &str) -> Result<usize> {
    lhs.checked_mul(rhs)
        .ok_or_else(|| Error::InvalidData(format!("{context} exceeds platform usize capacity")))
}

fn checked_add_usize(lhs: usize, rhs: usize, context: &str) -> Result<usize> {
    lhs.checked_add(rhs)
        .ok_or_else(|| Error::InvalidData(format!("{context} exceeds platform usize capacity")))
}

fn checked_mul_u64(lhs: u64, rhs: u64, context: &str) -> Result<u64> {
    lhs.checked_mul(rhs)
        .ok_or_else(|| Error::InvalidData(format!("{context} exceeds u64 capacity")))
}

fn checked_add_u64(lhs: u64, rhs: u64, context: &str) -> Result<u64> {
    lhs.checked_add(rhs)
        .ok_or_else(|| Error::InvalidData(format!("{context} exceeds u64 capacity")))
}

fn checked_shape_elements_usize(shape: &[u64], context: &str) -> Result<usize> {
    let mut total = 1usize;
    for &dim in shape {
        total = checked_mul_usize(total, checked_usize(dim, context)?, context)?;
    }
    Ok(total)
}

fn full_dataset_chunk_bounds(
    shape: &[u64],
    chunk_shape: &[u64],
) -> Result<Option<(Vec<u64>, Vec<u64>)>> {
    validate_chunk_shape(shape, chunk_shape)?;
    if shape.contains(&0) {
        return Ok(None);
    }

    let first_chunk = vec![0u64; shape.len()];
    let last_chunk = shape
        .iter()
        .zip(chunk_shape.iter())
        .map(|(&dim, &chunk)| dim.div_ceil(chunk) - 1)
        .collect();
    Ok(Some((first_chunk, last_chunk)))
}

fn validate_chunk_shape(shape: &[u64], chunk_shape: &[u64]) -> Result<()> {
    if chunk_shape.len() != shape.len() {
        return Err(Error::InvalidData(format!(
            "chunk rank {} does not match dataset rank {}",
            chunk_shape.len(),
            shape.len()
        )));
    }
    if let Some((dim, _)) = chunk_shape
        .iter()
        .enumerate()
        .find(|(_, chunk)| **chunk == 0)
    {
        return Err(Error::InvalidData(format!(
            "chunk dimension {dim} has zero extent"
        )));
    }
    Ok(())
}

fn validate_decoded_chunk_len(
    entry: &chunk_index::ChunkEntry,
    chunk_shape: &[u64],
    elem_size: usize,
    actual_len: usize,
) -> Result<()> {
    let expected_len = decoded_chunk_expected_len(chunk_shape, elem_size)?;
    if actual_len != expected_len {
        return Err(Error::InvalidData(format!(
            "chunk at offsets {:?} decoded to {} bytes, expected {} bytes",
            entry.offsets, actual_len, expected_len
        )));
    }
    Ok(())
}

fn decoded_chunk_expected_len(chunk_shape: &[u64], elem_size: usize) -> Result<usize> {
    let chunk_elements = checked_shape_elements_usize(chunk_shape, "decoded chunk element count")?;
    checked_mul_usize(chunk_elements, elem_size, "decoded chunk byte length")
}

fn validate_chunk_grid_coverage(
    entries: &mut [chunk_index::ChunkEntry],
    shape: &[u64],
    chunk_shape: &[u64],
    first_chunk: &[u64],
    last_chunk: &[u64],
) -> Result<bool> {
    validate_chunk_shape(shape, chunk_shape)?;
    if first_chunk.len() != shape.len() || last_chunk.len() != shape.len() {
        return Err(Error::InvalidData(format!(
            "chunk grid bounds rank does not match dataset rank {}",
            shape.len()
        )));
    }

    if shape.contains(&0) {
        if entries.is_empty() {
            return Ok(true);
        }
        return Err(Error::InvalidData(
            "chunk index contains entries for an empty dataset".into(),
        ));
    }

    for dim in 0..shape.len() {
        if first_chunk[dim] > last_chunk[dim] {
            return Err(Error::InvalidData(format!(
                "invalid chunk grid bounds for dimension {dim}: {} > {}",
                first_chunk[dim], last_chunk[dim]
            )));
        }
    }

    entries.sort_by(|a, b| a.offsets.cmp(&b.offsets));

    for i in 0..entries.len() {
        validate_chunk_entry_offsets(&entries[i], shape, chunk_shape, first_chunk, last_chunk)?;
        if i > 0 && entries[i].offsets == entries[i - 1].offsets {
            return Err(Error::InvalidData(format!(
                "duplicate chunk output offsets {:?} (addresses {:#x} and {:#x})",
                entries[i].offsets,
                entries[i - 1].address,
                entries[i].address
            )));
        }
    }

    let mut entry_idx = 0usize;
    let mut expected = first_chunk.to_vec();
    loop {
        let expected_offsets: Vec<u64> = expected
            .iter()
            .enumerate()
            .map(|(dim, chunk_index)| chunk_index * chunk_shape[dim])
            .collect();

        if entry_idx >= entries.len() || entries[entry_idx].offsets != expected_offsets {
            return Ok(false);
        }
        entry_idx += 1;

        if !advance_chunk_index(&mut expected, first_chunk, last_chunk) {
            break;
        }
    }

    Ok(entry_idx == entries.len())
}

fn validate_chunk_entry_offsets(
    entry: &chunk_index::ChunkEntry,
    shape: &[u64],
    chunk_shape: &[u64],
    first_chunk: &[u64],
    last_chunk: &[u64],
) -> Result<()> {
    if entry.offsets.len() != shape.len() {
        return Err(Error::InvalidData(format!(
            "chunk at address {:#x} has rank {}, expected {}",
            entry.address,
            entry.offsets.len(),
            shape.len()
        )));
    }

    for dim in 0..shape.len() {
        let offset = entry.offsets[dim];
        if offset >= shape[dim] {
            return Err(Error::InvalidData(format!(
                "chunk at address {:#x} has out-of-bounds offset {} for dimension {} of size {}",
                entry.address, offset, dim, shape[dim]
            )));
        }
        if offset % chunk_shape[dim] != 0 {
            return Err(Error::InvalidData(format!(
                "chunk at address {:#x} has non-grid offset {} for dimension {} with chunk extent {}",
                entry.address, offset, dim, chunk_shape[dim]
            )));
        }

        let chunk_index = offset / chunk_shape[dim];
        if chunk_index < first_chunk[dim] || chunk_index > last_chunk[dim] {
            return Err(Error::InvalidData(format!(
                "chunk at address {:#x} has offset {:?} outside requested chunk grid",
                entry.address, entry.offsets
            )));
        }
    }

    Ok(())
}

fn advance_chunk_index(index: &mut [u64], first_chunk: &[u64], last_chunk: &[u64]) -> bool {
    if index.is_empty() {
        return false;
    }

    for dim in (0..index.len()).rev() {
        if index[dim] < last_chunk[dim] {
            index[dim] += 1;
            if dim + 1 < index.len() {
                index[(dim + 1)..].copy_from_slice(&first_chunk[(dim + 1)..]);
            }
            return true;
        }
    }

    false
}

fn row_major_strides(shape: &[u64], context: &str) -> Result<Vec<usize>> {
    let ndim = shape.len();
    if ndim == 0 {
        return Ok(Vec::new());
    }

    let mut strides = vec![1usize; ndim];
    for i in (0..ndim - 1).rev() {
        let next_extent = checked_usize(shape[i + 1], context)?;
        strides[i] = checked_mul_usize(strides[i + 1], next_extent, context)?;
    }
    Ok(strides)
}

/// Convert an allocation of maybe-uninitialized bytes without copying it.
///
/// # Safety
///
/// Every element of `buffer` must have been initialized.
unsafe fn assume_init_u8_vec(mut buffer: Vec<MaybeUninit<u8>>) -> Vec<u8> {
    let ptr = buffer.as_mut_ptr() as *mut u8;
    let len = buffer.len();
    let capacity = buffer.capacity();
    std::mem::forget(buffer);
    // SAFETY: guaranteed by the caller; `u8` and `MaybeUninit<u8>` have the
    // same layout, and ownership of the allocation was transferred by forget.
    unsafe { Vec::from_raw_parts(ptr, len, capacity) }
}

/// `len` uninitialised slots, or an error when the allocation cannot be satisfied: a corrupt
/// dataspace (one dimension of 2.8e14) asked for petabytes and aborted the process instead of
/// failing the read (ModernViewer MV-110 fuzzing).
fn try_uninit_vec<T>(len: usize) -> Result<Vec<MaybeUninit<T>>> {
    let mut buffer: Vec<MaybeUninit<T>> = Vec::new();
    buffer.try_reserve_exact(len).map_err(|_| {
        Error::InvalidData(format!(
            "cannot allocate {len} elements of {} bytes for the selection; the dataspace may be corrupt",
            std::mem::size_of::<T>()
        ))
    })?;
    // SAFETY: the capacity is reserved and `MaybeUninit` needs no initialisation.
    unsafe { buffer.set_len(len) };
    Ok(buffer)
}

/// `len` zero bytes, failing like [`try_uninit_vec`] instead of aborting.
fn try_zeroed_bytes(len: usize) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(len).map_err(|_| {
        Error::InvalidData(format!(
            "cannot allocate {len} bytes for the selection; the dataspace may be corrupt"
        ))
    })?;
    buffer.resize(len, 0);
    Ok(buffer)
}

/// Convert an allocation of maybe-uninitialized values without copying it.
///
/// # Safety
///
/// Every element of `buffer` must have been initialized.
unsafe fn assume_init_vec<T>(mut buffer: Vec<MaybeUninit<T>>) -> Vec<T> {
    let ptr = buffer.as_mut_ptr() as *mut T;
    let len = buffer.len();
    let capacity = buffer.capacity();
    std::mem::forget(buffer);
    // SAFETY: guaranteed by the caller; `T` and `MaybeUninit<T>` have the
    // same layout, and ownership of the allocation was transferred by forget.
    unsafe { Vec::from_raw_parts(ptr, len, capacity) }
}

fn normalize_selection(selection: &SliceInfo, shape: &[u64]) -> Result<ResolvedSelection> {
    if selection.selections.len() != shape.len() {
        return Err(Error::InvalidData(format!(
            "slice has {} dimensions but dataset has {}",
            selection.selections.len(),
            shape.len()
        )));
    }

    let mut dims = Vec::with_capacity(shape.len());
    let mut result_shape = Vec::new();
    let mut result_elements = 1usize;

    for (i, sel) in selection.selections.iter().enumerate() {
        let dim_size = shape[i];
        match sel {
            SliceInfoElem::Index(idx) => {
                if *idx >= dim_size {
                    return Err(Error::SliceOutOfBounds {
                        dim: i,
                        index: *idx,
                        size: dim_size,
                    });
                }
                dims.push(ResolvedSelectionDim {
                    start: *idx,
                    end: *idx + 1,
                    step: 1,
                    count: 1,
                });
            }
            SliceInfoElem::Slice { start, end, step } => {
                if *step == 0 {
                    return Err(Error::InvalidData("slice step cannot be 0".into()));
                }
                if *start > dim_size {
                    return Err(Error::SliceOutOfBounds {
                        dim: i,
                        index: *start,
                        size: dim_size,
                    });
                }

                let actual_end = if *end == u64::MAX {
                    dim_size
                } else {
                    (*end).min(dim_size)
                };
                let count_u64 = if *start >= actual_end {
                    0
                } else {
                    (actual_end - *start).div_ceil(*step)
                };
                let count = checked_usize(count_u64, "slice element count")?;

                dims.push(ResolvedSelectionDim {
                    start: *start,
                    end: actual_end,
                    step: *step,
                    count,
                });
                result_shape.push(count);
                result_elements =
                    checked_mul_usize(result_elements, count, "slice result element count")?;
            }
        }
    }

    Ok(ResolvedSelection {
        dims,
        result_shape,
        result_elements,
    })
}

fn selection_dim_is_full_unit(dim: &ResolvedSelectionDim, dim_size: u64) -> bool {
    dim.step == 1
        && dim.start == 0
        && dim.end == dim_size
        && u64::try_from(dim.count).ok() == Some(dim_size)
}

fn selection_covers_full_dataset(resolved: &ResolvedSelection, shape: &[u64]) -> bool {
    resolved.result_shape.len() == shape.len()
        && resolved
            .dims
            .iter()
            .zip(shape.iter())
            .all(|(dim, &dim_size)| selection_dim_is_full_unit(dim, dim_size))
}

fn contiguous_slice_tail_start(shape: &[u64], resolved: &ResolvedSelection) -> usize {
    let ndim = shape.len();
    if ndim == 0 {
        return 0;
    }

    let mut tail_start = if resolved.dims[ndim - 1].step == 1 {
        ndim - 1
    } else {
        ndim
    };

    while tail_start > 0 {
        let prev = tail_start - 1;
        let later_dims_are_full =
            (tail_start..ndim).all(|d| selection_dim_is_full_unit(&resolved.dims[d], shape[d]));
        if resolved.dims[prev].step == 1 && later_dims_are_full {
            tail_start = prev;
        } else {
            break;
        }
    }

    tail_start
}

fn contiguous_slice_block_elements(
    resolved: &ResolvedSelection,
    tail_start: usize,
) -> Result<usize> {
    let mut elements = 1usize;
    for dim in &resolved.dims[tail_start..] {
        elements = checked_mul_usize(elements, dim.count, "contiguous slice block elements")?;
    }
    Ok(elements)
}

fn result_strides_for_dims(result_dims: &[usize]) -> Result<Vec<usize>> {
    let ndim = result_dims.len();
    let mut result_strides = vec![1usize; ndim];
    for d in (0..ndim.saturating_sub(1)).rev() {
        result_strides[d] =
            checked_mul_usize(result_strides[d + 1], result_dims[d + 1], "result stride")?;
    }
    Ok(result_strides)
}

/// A dataset within an HDF5 file.
#[derive(Clone)]
pub struct Dataset {
    pub(crate) context: Arc<FileContext>,
    pub(crate) name: String,
    pub(crate) data_address: u64,
    pub(crate) dataspace: DataspaceMessage,
    pub(crate) datatype: Datatype,
    pub(crate) layout: DataLayout,
    pub(crate) fill_value: Option<FillValueMessage>,
    pub(crate) filters: Option<FilterPipelineMessage>,
    pub(crate) external_files: Option<ExternalFilesMessage>,
    pub(crate) attributes: Vec<AttributeMessage>,
    pub(crate) chunk_cache: Arc<ChunkCache>,
    chunk_entry_cache: Arc<Mutex<LruCache<ChunkEntryCacheKey, Arc<Vec<chunk_index::ChunkEntry>>>>>,
    full_chunk_entries: Arc<OnceLock<Arc<Vec<chunk_index::ChunkEntry>>>>,
    full_dataset_bytes: Arc<OnceLock<Arc<Vec<u8>>>>,
    external_slots: Arc<OnceLock<Arc<Vec<ResolvedExternalRawSlot>>>>,
    pub(crate) filter_registry: Arc<FilterRegistry>,
}

/// One decoded chunk from a chunked dataset.
pub struct DatasetChunk {
    offsets: Vec<u64>,
    shape: Vec<u64>,
    filter_mask: u32,
    bytes: Arc<Vec<u8>>,
}

impl DatasetChunk {
    /// Dataset coordinates of the first element in this chunk.
    pub fn offsets(&self) -> &[u64] {
        &self.offsets
    }

    /// Logical chunk shape from the dataset layout.
    pub fn shape(&self) -> &[u64] {
        &self.shape
    }

    /// HDF5 filter mask recorded for this chunk.
    pub fn filter_mask(&self) -> u32 {
        self.filter_mask
    }

    /// Decoded logical bytes for this chunk.
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_ref()
    }
}

/// Iterator over decoded chunks in a chunked dataset.
pub struct DatasetChunkIterator {
    dataset: Dataset,
    entries: Vec<chunk_index::ChunkEntry>,
    index_address: u64,
    chunk_shape: Vec<u64>,
    elem_size: usize,
    next: usize,
}

impl Iterator for DatasetChunkIterator {
    type Item = Result<DatasetChunk>;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.entries.get(self.next)?;
        self.next += 1;

        Some(
            self.dataset
                .load_exact_chunk_data(entry, self.index_address, &self.chunk_shape, self.elem_size)
                .map(|bytes| DatasetChunk {
                    offsets: entry.offsets.clone(),
                    shape: self.chunk_shape.clone(),
                    filter_mask: entry.filter_mask,
                    bytes,
                }),
        )
    }
}

pub(crate) struct DatasetTemplate {
    name: String,
    data_address: u64,
    dataspace: DataspaceMessage,
    datatype: Datatype,
    layout: DataLayout,
    fill_value: Option<FillValueMessage>,
    filters: Option<FilterPipelineMessage>,
    external_files: Option<ExternalFilesMessage>,
    attributes: Vec<AttributeMessage>,
    chunk_entry_cache: Arc<Mutex<LruCache<ChunkEntryCacheKey, Arc<Vec<chunk_index::ChunkEntry>>>>>,
    full_chunk_entries: Arc<OnceLock<Arc<Vec<chunk_index::ChunkEntry>>>>,
    full_dataset_bytes: Arc<OnceLock<Arc<Vec<u8>>>>,
    external_slots: Arc<OnceLock<Arc<Vec<ResolvedExternalRawSlot>>>>,
}

impl Dataset {
    pub(crate) fn from_template(context: Arc<FileContext>, template: Arc<DatasetTemplate>) -> Self {
        Dataset {
            chunk_cache: context.chunk_cache.clone(),
            filter_registry: context.filter_registry.clone(),
            context,
            name: template.name.clone(),
            data_address: template.data_address,
            dataspace: template.dataspace.clone(),
            datatype: template.datatype.clone(),
            layout: template.layout.clone(),
            fill_value: template.fill_value.clone(),
            filters: template.filters.clone(),
            external_files: template.external_files.clone(),
            attributes: template.attributes.clone(),
            chunk_entry_cache: template.chunk_entry_cache.clone(),
            full_chunk_entries: template.full_chunk_entries.clone(),
            full_dataset_bytes: template.full_dataset_bytes.clone(),
            external_slots: template.external_slots.clone(),
        }
    }

    pub(crate) fn template(&self) -> Arc<DatasetTemplate> {
        Arc::new(DatasetTemplate {
            name: self.name.clone(),
            data_address: self.data_address,
            dataspace: self.dataspace.clone(),
            datatype: self.datatype.clone(),
            layout: self.layout.clone(),
            fill_value: self.fill_value.clone(),
            filters: self.filters.clone(),
            external_files: self.external_files.clone(),
            attributes: self.attributes.clone(),
            chunk_entry_cache: self.chunk_entry_cache.clone(),
            full_chunk_entries: self.full_chunk_entries.clone(),
            full_dataset_bytes: self.full_dataset_bytes.clone(),
            external_slots: self.external_slots.clone(),
        })
    }

    pub(crate) fn from_parsed_header(
        context: DatasetParseContext,
        address: u64,
        name: String,
        header: &ObjectHeader,
    ) -> Result<Self> {
        let mut dataspace: Option<DataspaceMessage> = None;
        let mut datatype: Option<Datatype> = None;
        let mut layout: Option<DataLayout> = None;
        let mut fill_value: Option<FillValueMessage> = None;
        let mut filter_pipeline: Option<FilterPipelineMessage> = None;
        let mut external_files: Option<ExternalFilesMessage> = None;
        let attributes = collect_attribute_messages_storage(
            header,
            context.context.storage.as_ref(),
            context.context.superblock.offset_size,
            context.context.superblock.length_size,
            Some(context.context.filter_registry.as_ref()),
        )?;

        for msg in &header.messages {
            match msg {
                HdfMessage::Dataspace(ds) => dataspace = Some(ds.clone()),
                HdfMessage::Datatype(dt) => datatype = Some(dt.datatype.clone()),
                HdfMessage::DataLayout(dl) => layout = Some(dl.layout.clone()),
                HdfMessage::FillValue(fv) => fill_value = Some(fv.clone()),
                HdfMessage::FilterPipeline(fp) => filter_pipeline = Some(fp.clone()),
                HdfMessage::ExternalFiles(ef) => external_files = Some(ef.clone()),
                _ => {}
            }
        }

        let dataspace =
            dataspace.ok_or_else(|| Error::InvalidData("dataset missing dataspace".into()))?;
        let dt = datatype.ok_or_else(|| Error::InvalidData("dataset missing datatype".into()))?;
        let layout =
            layout.ok_or_else(|| Error::InvalidData("dataset missing data layout".into()))?;
        let layout = normalize_layout(layout, &dataspace);
        let attr_fill_value = attributes
            .iter()
            .find(|attr| {
                attr.name == "_FillValue" && matches!(attr.dataspace.num_elements(), Ok(1))
            })
            .map(|attr| FillValueMessage {
                defined: !attr.raw_data.is_empty(),
                fill_time: FillTime::IfSet,
                value: Some(attr.raw_data.clone()),
            });
        let fill_value = match fill_value {
            Some(existing) if existing.value.is_some() => Some(existing),
            _ => attr_fill_value,
        };

        Ok(Dataset {
            context: context.context.clone(),
            name,
            data_address: address,
            dataspace,
            datatype: dt,
            layout,
            fill_value,
            filters: filter_pipeline,
            external_files,
            attributes,
            chunk_cache: context.context.chunk_cache.clone(),
            chunk_entry_cache: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(32).unwrap()))),
            full_chunk_entries: Arc::new(OnceLock::new()),
            full_dataset_bytes: Arc::new(OnceLock::new()),
            external_slots: Arc::new(OnceLock::new()),
            filter_registry: context.context.filter_registry.clone(),
        })
    }

    /// Dataset name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The object header address used to parse this dataset.
    /// Useful as an opaque identifier or for NC4 data_offset.
    pub fn address(&self) -> u64 {
        self.data_address
    }

    /// Shape of the dataset (dimensions).
    pub fn shape(&self) -> &[u64] {
        &self.dataspace.dims
    }

    /// Datatype of the dataset.
    pub fn dtype(&self) -> &Datatype {
        &self.datatype
    }

    /// Number of dimensions.
    pub fn ndim(&self) -> usize {
        self.dataspace.dims.len()
    }

    fn offset_size(&self) -> u8 {
        self.context.superblock.offset_size
    }

    fn length_size(&self) -> u8 {
        self.context.superblock.length_size
    }

    /// Maximum dimension sizes, if defined. `u64::MAX` indicates unlimited.
    pub fn max_dims(&self) -> Option<&[u64]> {
        self.dataspace.max_dims.as_deref()
    }

    /// Chunk dimensions, if the dataset is chunked.
    pub fn chunks(&self) -> Option<Vec<u32>> {
        match &self.layout {
            DataLayout::Chunked { dims, .. } => Some(dims.clone()),
            _ => None,
        }
    }

    /// Iterate over decoded chunks for a chunked dataset.
    pub fn iter_chunks(&self) -> Result<DatasetChunkIterator> {
        let DataLayout::Chunked {
            address,
            dims,
            chunk_indexing,
            ..
        } = &self.layout
        else {
            return Err(Error::InvalidData(format!(
                "dataset '{}' is not chunked",
                self.name
            )));
        };

        if Cursor::is_undefined_offset(*address, self.offset_size()) {
            return Ok(DatasetChunkIterator {
                dataset: self.clone(),
                entries: Vec::new(),
                index_address: *address,
                chunk_shape: dims.iter().map(|&d| d as u64).collect(),
                elem_size: self.raw_element_size()?,
                next: 0,
            });
        }

        let ndim = self.ndim();
        let shape = &self.dataspace.dims;
        let elem_size = self.raw_element_size()?;
        let chunk_shape: Vec<u64> = dims.iter().map(|&d| d as u64).collect();
        validate_chunk_shape(shape, &chunk_shape)?;
        let entries = self.collect_chunk_entries(
            *address,
            dims,
            chunk_indexing.as_ref(),
            ChunkEntrySelection {
                shape,
                ndim,
                elem_size,
                chunk_bounds: None,
            },
        )?;

        Ok(DatasetChunkIterator {
            dataset: self.clone(),
            entries,
            index_address: *address,
            chunk_shape,
            elem_size,
            next: 0,
        })
    }

    /// Return current chunk-cache statistics for this dataset's file.
    pub fn chunk_cache_stats(&self) -> ChunkCacheStats {
        self.chunk_cache.stats()
    }

    /// Fill value, if defined.
    pub fn fill_value(&self) -> Option<&FillValueMessage> {
        self.fill_value.as_ref()
    }

    /// Dataset attributes.
    pub fn attributes(&self) -> Vec<Attribute> {
        self.attributes
            .iter()
            .map(|a| attribute_from_message_storage(a, self.context.as_ref()))
            .collect()
    }

    /// Find an attribute by name.
    pub fn attribute(&self, name: &str) -> Result<Attribute> {
        self.attributes
            .iter()
            .find(|a| a.name == name)
            .map(|a| attribute_from_message_storage(a, self.context.as_ref()))
            .ok_or_else(|| Error::AttributeNotFound(name.to_string()))
    }

    /// Read a scalar string dataset or single string element.
    ///
    /// Use [`Dataset::read_strings`] when the dataset contains multiple strings.
    pub fn read_string(&self) -> Result<String> {
        let mut strings = self.read_strings()?;
        match strings.len() {
            1 => Ok(strings.swap_remove(0)),
            0 => Err(Error::InvalidData(format!(
                "dataset '{}' contains no string elements",
                self.name
            ))),
            count => Err(Error::InvalidData(format!(
                "dataset '{}' contains {count} string elements; use read_strings()",
                self.name
            ))),
        }
    }

    /// Read all string elements from a string-typed dataset.
    pub fn read_strings(&self) -> Result<Vec<String>> {
        match &self.datatype {
            Datatype::String {
                size: StringSize::Fixed(len),
                encoding,
                padding,
            } => {
                let raw = self.read_raw_bytes()?;
                let elem_size = *len as usize;
                let count = checked_usize(self.num_elements()?, "dataset string element count")?;
                let expected_bytes =
                    checked_mul_usize(count, elem_size, "dataset string byte size")?;
                if raw.len() < expected_bytes {
                    return Err(Error::InvalidData(format!(
                        "dataset '{}' string data too short: need {} bytes, have {}",
                        self.name,
                        expected_bytes,
                        raw.len()
                    )));
                }

                let mut strings = Vec::with_capacity(count);
                for i in 0..count {
                    let start = i * elem_size;
                    let end = start + elem_size;
                    strings.push(decode_string(&raw[start..end], *padding, *encoding)?);
                }
                Ok(strings)
            }
            Datatype::String {
                size: StringSize::Variable,
                encoding,
                padding,
            } => {
                let raw = self.read_raw_bytes()?;
                let count = checked_usize(self.num_elements()?, "dataset string element count")?;
                let ref_size = 4 + self.offset_size() as usize + 4;
                let expected_bytes =
                    checked_mul_usize(count, ref_size, "dataset string reference byte size")?;
                if raw.len() < expected_bytes {
                    return Err(Error::InvalidData(format!(
                        "dataset '{}' vlen string data too short: need {} bytes, have {}",
                        self.name,
                        expected_bytes,
                        raw.len()
                    )));
                }

                let mut strings = Vec::with_capacity(count);
                for i in 0..count {
                    let offset = i * ref_size;
                    strings.push(read_one_vlen_string_storage(
                        &raw,
                        offset,
                        self.context.storage.as_ref(),
                        self.offset_size(),
                        self.length_size(),
                        *padding,
                        *encoding,
                    )?);
                }
                Ok(strings)
            }
            Datatype::VarLen {
                base,
                kind: VarLenKind::String,
                encoding,
                padding,
            } => {
                if !base.is_vlen_string_base() {
                    return Err(Error::TypeMismatch {
                        expected: "String dataset".into(),
                        actual: format!("{:?}", self.datatype),
                    });
                }

                let raw = self.read_raw_bytes()?;
                let count = checked_usize(self.num_elements()?, "dataset string element count")?;
                let ref_size = 4 + self.offset_size() as usize + 4;
                let expected_bytes =
                    checked_mul_usize(count, ref_size, "dataset string reference byte size")?;
                if raw.len() < expected_bytes {
                    return Err(Error::InvalidData(format!(
                        "dataset '{}' vlen byte string data too short: need {} bytes, have {}",
                        self.name,
                        expected_bytes,
                        raw.len()
                    )));
                }

                let mut strings = Vec::with_capacity(count);
                for i in 0..count {
                    let offset = i * ref_size;
                    let ref_bytes = &raw[offset..offset + ref_size];
                    let value = resolve_vlen_bytes_storage(
                        ref_bytes,
                        self.context.storage.as_ref(),
                        self.offset_size(),
                        self.length_size(),
                    )
                    .unwrap_or_default();
                    strings.push(decode_string(&value, *padding, *encoding)?);
                }
                Ok(strings)
            }
            _ => Err(Error::TypeMismatch {
                expected: "String dataset".into(),
                actual: format!("{:?}", self.datatype),
            }),
        }
    }

    /// Total number of elements in the dataset.
    pub fn num_elements(&self) -> Result<u64> {
        Ok(self.dataspace.num_elements()?)
    }

    /// Read the entire dataset into an n-dimensional array.
    pub fn read_array<T: H5Type>(&self) -> Result<ArrayD<T>> {
        let result = match &self.layout {
            DataLayout::Compact { data } => self.read_compact::<T>(data),
            DataLayout::Contiguous { address, size } => self.read_contiguous::<T>(*address, *size),
            DataLayout::Chunked {
                address,
                dims,
                element_size,
                chunk_indexing,
            } => self.read_chunked::<T>(*address, dims, *element_size, chunk_indexing.as_ref()),
        };
        result.map_err(|e| e.with_context(&self.name))
    }

    /// Read the entire dataset into a caller-provided typed buffer.
    pub fn read_into<T: H5Type>(&self, dst: &mut [T]) -> Result<()> {
        let result = (|| {
            let element_count = checked_usize(self.num_elements()?, "dataset element count")?;
            if dst.len() != element_count {
                return Err(Error::InvalidData(format!(
                    "destination has {} elements, dataset requires {}",
                    dst.len(),
                    element_count
                )));
            }

            let elem_size = self.raw_element_size()?;
            if T::native_copy_compatible(&self.datatype) && std::mem::size_of::<T>() == elem_size {
                // SAFETY: `dst` is a live, contiguous mutable slice, and the
                // checked byte length exactly covers its initialized elements.
                let dst_bytes = unsafe {
                    std::slice::from_raw_parts_mut(
                        dst.as_mut_ptr() as *mut u8,
                        checked_mul_usize(dst.len(), elem_size, "destination byte length")?,
                    )
                };
                return self.read_raw_bytes_into_inner(dst_bytes);
            }

            let array = self.read_array::<T>()?;
            let values = array.as_slice_memory_order().ok_or_else(|| {
                Error::InvalidData("decoded array is not contiguous in memory order".into())
            })?;
            dst.clone_from_slice(values);
            Ok(())
        })();

        result.map_err(|e| e.with_context(&self.name))
    }

    /// Read the entire dataset using internal chunk-level parallelism when possible.
    ///
    /// Non-chunked datasets fall back to `read_array`.
    #[cfg(feature = "rayon")]
    pub fn read_array_parallel<T: H5Type>(&self) -> Result<ArrayD<T>> {
        match &self.layout {
            DataLayout::Chunked {
                address,
                dims,
                element_size,
                chunk_indexing,
            } => self.read_chunked_parallel::<T>(
                *address,
                dims,
                *element_size,
                chunk_indexing.as_ref(),
            ),
            _ => self.read_array::<T>(),
        }
    }

    /// Read the entire dataset using the provided Rayon thread pool.
    ///
    /// Non-chunked datasets fall back to `read_array`.
    #[cfg(feature = "rayon")]
    pub fn read_array_in_pool<T: H5Type>(&self, pool: &rayon::ThreadPool) -> Result<ArrayD<T>> {
        match &self.layout {
            DataLayout::Chunked {
                address,
                dims,
                element_size,
                chunk_indexing,
            } => pool.install(|| {
                self.read_chunked_parallel::<T>(
                    *address,
                    dims,
                    *element_size,
                    chunk_indexing.as_ref(),
                )
            }),
            _ => self.read_array::<T>(),
        }
    }

    /// Read a hyperslab of the dataset using chunk-level parallelism when possible.
    ///
    /// Chunked datasets decompress overlapping chunks in parallel via Rayon.
    /// Non-chunked layouts fall back to `read_slice`.
    #[cfg(feature = "rayon")]
    pub fn read_slice_parallel<T: H5Type>(&self, selection: &SliceInfo) -> Result<ArrayD<T>> {
        let resolved = normalize_selection(selection, &self.dataspace.dims)?;

        match &self.layout {
            DataLayout::Chunked {
                address,
                dims,
                element_size,
                chunk_indexing,
            } => self.read_chunked_slice_parallel::<T>(
                *address,
                dims,
                *element_size,
                chunk_indexing.as_ref(),
                selection,
                &resolved,
            ),
            _ => self.read_slice::<T>(selection),
        }
    }

    /// Read a hyperslab of the dataset.
    pub fn read_slice<T: H5Type>(&self, selection: &SliceInfo) -> Result<ArrayD<T>> {
        let resolved = normalize_selection(selection, &self.dataspace.dims)?;

        match &self.layout {
            DataLayout::Contiguous { address, size } => {
                self.read_contiguous_slice::<T>(*address, *size, &resolved)
            }
            DataLayout::Compact { data } => self.read_compact_slice::<T>(data, selection),
            DataLayout::Chunked {
                address,
                dims,
                element_size,
                chunk_indexing,
            } => self.read_chunked_slice::<T>(
                *address,
                dims,
                *element_size,
                chunk_indexing.as_ref(),
                selection,
                &resolved,
            ),
        }
    }

    fn read_compact<T: H5Type>(&self, data: &[u8]) -> Result<ArrayD<T>> {
        self.validate_allocated_raw_data_len("compact", data.len())?;
        self.decode_raw_data::<T>(data)
    }

    /// Read the dataset as logical element bytes.
    ///
    /// The returned bytes are after layout handling, chunk decompression, filter
    /// decoding, external raw-data resolution, and fill-value materialization.
    /// Numeric values remain in the dataset datatype's byte order.
    pub fn read_raw_bytes(&self) -> Result<Vec<u8>> {
        let result: Result<Vec<u8>> = (|| {
            let total_bytes = self.raw_byte_len()?;
            let mut output = try_zeroed_bytes(total_bytes)?;
            self.read_raw_bytes_into_inner(&mut output)?;
            Ok(output)
        })();

        result.map_err(|e| e.with_context(&self.name))
    }

    /// Number of logical raw bytes in the entire dataset.
    pub fn raw_byte_len(&self) -> Result<usize> {
        let elem_size = self.raw_element_size()?;
        let total_elements = checked_usize(self.num_elements()?, "dataset element count")?;
        checked_mul_usize(total_elements, elem_size, "dataset size in bytes")
    }

    /// Read logical raw bytes into a caller-provided buffer.
    ///
    /// The destination length must match [`Dataset::raw_byte_len`]. Numeric
    /// values remain in the dataset datatype's byte order.
    pub fn read_raw_bytes_into(&self, dst: &mut [u8]) -> Result<()> {
        let result: Result<()> = (|| {
            let total_bytes = self.raw_byte_len()?;
            if dst.len() != total_bytes {
                return Err(Error::InvalidData(format!(
                    "destination has {} bytes, dataset requires {}",
                    dst.len(),
                    total_bytes
                )));
            }
            self.read_raw_bytes_into_inner(dst)
        })();

        result.map_err(|e| e.with_context(&self.name))
    }

    /// Read logical raw bytes converted to native-endian numeric fields.
    pub fn read_native_bytes(&self) -> Result<Vec<u8>> {
        let result: Result<Vec<u8>> = (|| {
            let total_bytes = self.raw_byte_len()?;
            let mut output = try_zeroed_bytes(total_bytes)?;
            self.read_raw_bytes_into_inner(&mut output)?;
            self.convert_to_native_endian(&mut output)?;
            Ok(output)
        })();

        result.map_err(|e| e.with_context(&self.name))
    }

    /// Read logical raw bytes into a buffer and convert numeric fields to native endian.
    pub fn read_native_bytes_into(&self, dst: &mut [u8]) -> Result<()> {
        let result: Result<()> = (|| {
            let total_bytes = self.raw_byte_len()?;
            if dst.len() != total_bytes {
                return Err(Error::InvalidData(format!(
                    "destination has {} bytes, dataset requires {}",
                    dst.len(),
                    total_bytes
                )));
            }
            self.read_raw_bytes_into_inner(dst)?;
            self.convert_to_native_endian(dst)
        })();

        result.map_err(|e| e.with_context(&self.name))
    }

    fn read_raw_bytes_into_inner(&self, dst: &mut [u8]) -> Result<()> {
        match &self.layout {
            DataLayout::Compact { data } => {
                self.validate_allocated_raw_data_len("compact", data.len())?;
                dst.copy_from_slice(data);
                Ok(())
            }
            DataLayout::Contiguous { address, size } => {
                self.read_contiguous_bytes_into(*address, *size, dst)
            }
            DataLayout::Chunked {
                address,
                dims,
                element_size: _,
                chunk_indexing,
            } => self.read_chunked_bytes_into(*address, dims, chunk_indexing.as_ref(), dst),
        }
    }

    /// Size in bytes of one HDF5 variable-length reference in this file.
    pub fn vlen_reference_size(&self) -> usize {
        4 + self.offset_size() as usize + 4
    }

    /// Size in bytes of one logical raw element in this file.
    ///
    /// Variable-length strings and sequences are stored as global-heap
    /// references whose width depends on the file offset size.
    pub fn raw_element_size(&self) -> Result<usize> {
        raw_element_size_for_datatype(&self.datatype, self.vlen_reference_size())
    }

    /// Resolve one HDF5 variable-length reference into logical element bytes.
    ///
    /// `base_element_size` is the byte width of one value in the vlen sequence.
    /// The sequence length stored in the reference is interpreted as an element
    /// count and multiplied by this size.
    pub fn resolve_vlen_reference_bytes(
        &self,
        reference: &[u8],
        base_element_size: usize,
    ) -> Result<Vec<u8>> {
        if reference.len() < self.vlen_reference_size() {
            return Err(Error::InvalidData(format!(
                "dataset '{}' vlen reference too short: need {} bytes, have {}",
                self.name,
                self.vlen_reference_size(),
                reference.len()
            )));
        }

        let mut cursor = Cursor::new(reference);
        let seq_len = cursor.read_u32_le()? as usize;
        let heap_addr = cursor.read_offset(self.offset_size())?;
        let obj_index = cursor.read_u32_le()? as u16;

        if Cursor::is_undefined_offset(heap_addr, self.offset_size()) || obj_index == 0 {
            return Ok(Vec::new());
        }

        let expected_bytes =
            checked_mul_usize(seq_len, base_element_size, "vlen sequence byte size")?;
        let collection = crate::global_heap::GlobalHeapCollection::parse_at_storage(
            self.context.storage.as_ref(),
            heap_addr,
            self.offset_size(),
            self.length_size(),
        )?;
        let object = collection.get_object(obj_index).ok_or_else(|| {
            Error::InvalidData(format!(
                "dataset '{}' references missing vlen heap object {}",
                self.name, obj_index
            ))
        })?;
        if object.data.len() < expected_bytes {
            return Err(Error::InvalidData(format!(
                "dataset '{}' vlen heap object too short: need {} bytes, have {}",
                self.name,
                expected_bytes,
                object.data.len()
            )));
        }

        Ok(object.data[..expected_bytes].to_vec())
    }

    fn read_contiguous<T: H5Type>(&self, address: u64, size: u64) -> Result<ArrayD<T>> {
        if self.external_files.is_some() {
            let elem_size = self.raw_element_size()?;
            let total_elements = checked_usize(self.num_elements()?, "dataset element count")?;
            let total_bytes =
                checked_mul_usize(total_elements, elem_size, "dataset size in bytes")?;
            let raw = self.read_external_range(0, total_bytes)?;
            return self.decode_raw_data::<T>(&raw);
        }

        if Cursor::is_undefined_offset(address, self.offset_size()) || size == 0 {
            // Dataset with no data written — return fill values
            return self.make_fill_array::<T>();
        }

        let sz = checked_usize(size, "contiguous dataset size")?;
        self.validate_allocated_raw_data_len("contiguous", sz)?;
        let raw = self.context.read_range(address, sz)?;
        self.decode_raw_data::<T>(raw.as_ref())
    }

    fn read_contiguous_bytes_into(&self, address: u64, size: u64, dst: &mut [u8]) -> Result<()> {
        if self.external_files.is_some() {
            return self.read_external_range_into(0, dst);
        }

        if Cursor::is_undefined_offset(address, self.offset_size()) || size == 0 {
            self.fill_output_buffer(dst);
            return Ok(());
        }

        let sz = checked_usize(size, "contiguous dataset size")?;
        self.validate_allocated_raw_data_len("contiguous", sz)?;
        if dst.is_empty() {
            return Ok(());
        }
        let raw = self.context.read_range(address, sz)?;
        dst.copy_from_slice(raw.as_ref());
        Ok(())
    }

    fn read_contiguous_logical_range(
        &self,
        address: u64,
        logical_offset: usize,
        len: usize,
    ) -> Result<Vec<u8>> {
        if self.external_files.is_some() {
            return self.read_external_range(logical_offset, len);
        }

        let file_offset = checked_add_u64(
            address,
            u64::try_from(logical_offset).map_err(|_| {
                Error::InvalidData("contiguous logical offset exceeds u64 capacity".to_string())
            })?,
            "contiguous read file offset",
        )?;
        Ok(self.context.read_range(file_offset, len)?.to_vec())
    }

    fn read_external_range(&self, logical_offset: usize, len: usize) -> Result<Vec<u8>> {
        let mut output = vec![0u8; len];
        self.read_external_range_into(logical_offset, &mut output)?;
        Ok(output)
    }

    fn read_external_range_into(&self, logical_offset: usize, dst: &mut [u8]) -> Result<()> {
        self.fill_output_buffer(dst);
        if dst.is_empty() {
            return Ok(());
        }

        let request_start = u64::try_from(logical_offset).map_err(|_| {
            Error::InvalidData("external dataset offset exceeds u64 capacity".to_string())
        })?;
        let request_len = u64::try_from(dst.len()).map_err(|_| {
            Error::InvalidData("external dataset length exceeds u64 capacity".to_string())
        })?;
        let request_end = request_start
            .checked_add(request_len)
            .ok_or_else(|| Error::InvalidData("external dataset range overflows".into()))?;

        for slot in self.external_raw_slots()?.iter() {
            let slot_end = slot.logical_offset.saturating_add(slot.size);
            let overlap_start = request_start.max(slot.logical_offset);
            let overlap_end = request_end.min(slot_end);
            if overlap_start >= overlap_end {
                continue;
            }

            let read_offset = slot
                .file_offset
                .checked_add(overlap_start - slot.logical_offset)
                .ok_or_else(|| Error::InvalidData("external file read offset overflows".into()))?;
            let read_len = checked_usize(overlap_end - overlap_start, "external read length")?;
            let dst_start = checked_usize(overlap_start - request_start, "external read dst")?;
            let dst_end = checked_add_usize(dst_start, read_len, "external read dst end")?;
            let bytes = slot.storage.read_range(read_offset, read_len)?;
            dst[dst_start..dst_end].copy_from_slice(bytes.as_ref());
        }

        Ok(())
    }

    fn external_raw_slots(&self) -> Result<Arc<Vec<ResolvedExternalRawSlot>>> {
        if let Some(slots) = self.external_slots.get() {
            return Ok(slots.clone());
        }

        let slots = Arc::new(self.load_external_raw_slots()?);
        let _ = self.external_slots.set(slots.clone());
        Ok(self
            .external_slots
            .get()
            .expect("external slot cache must exist after initialization")
            .clone())
    }

    fn load_external_raw_slots(&self) -> Result<Vec<ResolvedExternalRawSlot>> {
        let Some(external_files) = self.external_files.as_ref() else {
            return Ok(Vec::new());
        };

        let heap = LocalHeap::parse_at_storage(
            self.context.storage.as_ref(),
            external_files.heap_address,
            self.offset_size(),
            self.length_size(),
        )?;

        let mut logical_offset = 0u64;
        let mut slots = Vec::with_capacity(external_files.slots.len());
        for slot in &external_files.slots {
            let filename =
                heap.get_string_storage(slot.name_offset, self.context.storage.as_ref())?;
            let storage = self
                .context
                .resolve_external_file(&filename)?
                .ok_or_else(|| {
                    Error::Other(format!(
                        "external raw data file '{filename}' could not be resolved"
                    ))
                })?;
            let size = if Cursor::is_undefined_offset(slot.size, self.length_size()) {
                u64::MAX.saturating_sub(logical_offset)
            } else {
                slot.size
            };

            slots.push(ResolvedExternalRawSlot {
                logical_offset,
                storage,
                file_offset: slot.offset,
                size,
            });

            if Cursor::is_undefined_offset(slot.size, self.length_size()) {
                break;
            }
            logical_offset = logical_offset.checked_add(slot.size).ok_or_else(|| {
                Error::InvalidData("external raw data logical offset overflows".into())
            })?;
        }

        Ok(slots)
    }

    fn read_chunked<T: H5Type>(
        &self,
        index_address: u64,
        chunk_dims: &[u32],
        _element_size: u32,
        chunk_indexing: Option<&ChunkIndexing>,
    ) -> Result<ArrayD<T>> {
        if Cursor::is_undefined_offset(index_address, self.offset_size()) {
            return self.make_fill_array::<T>();
        }

        let ndim = self.ndim();
        let shape = &self.dataspace.dims;
        let elem_size = self.raw_element_size()?;
        let total_elements = checked_usize(self.num_elements()?, "dataset element count")?;
        let total_bytes = checked_mul_usize(total_elements, elem_size, "dataset size in bytes")?;

        if total_bytes <= HOT_FULL_DATASET_CACHE_MAX_BYTES {
            if let Some(cached_bytes) = self.full_dataset_bytes.get() {
                return self.decode_raw_data::<T>(cached_bytes);
            }
        }

        let chunk_shape: Vec<u64> = chunk_dims.iter().map(|&d| d as u64).collect();
        validate_chunk_shape(shape, &chunk_shape)?;
        let dataset_strides = row_major_strides(shape, "dataset stride")?;
        let chunk_strides = row_major_strides(&chunk_shape, "chunk stride")?;

        let mut entries = self.collect_chunk_entries(
            index_address,
            chunk_dims,
            chunk_indexing,
            ChunkEntrySelection {
                shape,
                ndim,
                elem_size,
                chunk_bounds: None,
            },
        )?;

        let full_chunk_coverage = match full_dataset_chunk_bounds(shape, &chunk_shape)? {
            Some((first_chunk, last_chunk)) => validate_chunk_grid_coverage(
                &mut entries,
                shape,
                &chunk_shape,
                &first_chunk,
                &last_chunk,
            )?,
            None if entries.is_empty() => true,
            None => {
                return Err(Error::InvalidData(
                    "chunk index contains entries for an empty dataset".into(),
                ))
            }
        };
        if full_chunk_coverage {
            if T::native_copy_compatible(&self.datatype) && std::mem::size_of::<T>() == elem_size {
                let mut result_values: Vec<MaybeUninit<T>> = try_uninit_vec::<T>(total_elements)?;
                let result_ptr = result_values.as_mut_ptr() as *mut u8;
                let result_len = checked_mul_usize(
                    result_values.len(),
                    std::mem::size_of::<T>(),
                    "typed dataset size in bytes",
                )?;

                for entry in &entries {
                    let chunk_data =
                        self.load_exact_chunk_data(entry, index_address, &chunk_shape, elem_size)?;
                    // SAFETY: the validated chunk grid has no duplicate
                    // offsets, the helper checks every range, and the output
                    // allocation remains live for the duration of the copy.
                    unsafe {
                        copy_chunk_to_flat_with_strides_ptr(
                            &chunk_data,
                            FlatBufferPtr {
                                ptr: result_ptr,
                                len: result_len,
                            },
                            ChunkCopyLayout {
                                chunk_offsets: &entry.offsets,
                                chunk_shape: &chunk_shape,
                                dataset_shape: shape,
                                dataset_strides: &dataset_strides,
                                chunk_strides: &chunk_strides,
                                elem_size,
                            },
                        )?;
                    }
                }

                if total_bytes <= HOT_FULL_DATASET_CACHE_MAX_BYTES {
                    let mut cached_bytes = try_zeroed_bytes(total_bytes)?;
                    // SAFETY: full chunk coverage initialized all
                    // `total_bytes` at `result_ptr`, and both allocations are
                    // live and distinct.
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            result_ptr,
                            cached_bytes.as_mut_ptr(),
                            total_bytes,
                        );
                    }
                    let _ = self.full_dataset_bytes.set(Arc::new(cached_bytes));
                }

                let mut result_shape = Vec::with_capacity(shape.len());
                for &dim in shape {
                    result_shape.push(checked_usize(dim, "dataset dimension")?);
                }
                // SAFETY: exact, duplicate-free chunk-grid coverage was
                // validated above and every chunk was copied successfully.
                let result_values = unsafe { assume_init_vec(result_values) };
                return ArrayD::from_shape_vec(IxDyn(&result_shape), result_values)
                    .map_err(|e| Error::InvalidData(format!("array shape error: {e}")));
            }

            let mut flat_data = try_uninit_vec::<u8>(total_bytes)?;
            let flat_ptr = flat_data.as_mut_ptr() as *mut u8;
            let flat_len = flat_data.len();

            for entry in &entries {
                let chunk_data =
                    self.load_exact_chunk_data(entry, index_address, &chunk_shape, elem_size)?;
                // SAFETY: the validated chunk grid has no duplicate offsets,
                // the helper checks every range, and the output allocation
                // remains live for the duration of the copy.
                unsafe {
                    copy_chunk_to_flat_with_strides_ptr(
                        &chunk_data,
                        FlatBufferPtr {
                            ptr: flat_ptr,
                            len: flat_len,
                        },
                        ChunkCopyLayout {
                            chunk_offsets: &entry.offsets,
                            chunk_shape: &chunk_shape,
                            dataset_shape: shape,
                            dataset_strides: &dataset_strides,
                            chunk_strides: &chunk_strides,
                            elem_size,
                        },
                    )?;
                }
            }

            // SAFETY: exact, duplicate-free chunk-grid coverage was validated
            // above and every chunk was copied successfully.
            let flat_data = unsafe { assume_init_u8_vec(flat_data) };
            if total_bytes <= HOT_FULL_DATASET_CACHE_MAX_BYTES {
                let _ = self.full_dataset_bytes.set(Arc::new(flat_data.clone()));
            }
            return self.decode_raw_data::<T>(&flat_data);
        }

        let mut flat_data = self.make_output_buffer(total_bytes)?;
        for entry in &entries {
            let chunk_data =
                self.load_exact_chunk_data(entry, index_address, &chunk_shape, elem_size)?;
            copy_chunk_to_flat_with_strides(
                &chunk_data,
                &mut flat_data,
                ChunkCopyLayout {
                    chunk_offsets: &entry.offsets,
                    chunk_shape: &chunk_shape,
                    dataset_shape: shape,
                    dataset_strides: &dataset_strides,
                    chunk_strides: &chunk_strides,
                    elem_size,
                },
            )?;
        }

        self.decode_raw_data::<T>(&flat_data)
    }

    fn read_chunked_bytes_into(
        &self,
        index_address: u64,
        chunk_dims: &[u32],
        chunk_indexing: Option<&ChunkIndexing>,
        dst: &mut [u8],
    ) -> Result<()> {
        if Cursor::is_undefined_offset(index_address, self.offset_size()) {
            self.fill_output_buffer(dst);
            return Ok(());
        }

        let ndim = self.ndim();
        let shape = &self.dataspace.dims;
        let elem_size = self.raw_element_size()?;

        if dst.len() <= HOT_FULL_DATASET_CACHE_MAX_BYTES {
            if let Some(cached_bytes) = self.full_dataset_bytes.get() {
                if cached_bytes.len() == dst.len() {
                    dst.copy_from_slice(cached_bytes.as_slice());
                    return Ok(());
                }
            }
        }

        let chunk_shape: Vec<u64> = chunk_dims.iter().map(|&d| d as u64).collect();
        validate_chunk_shape(shape, &chunk_shape)?;
        let dataset_strides = row_major_strides(shape, "dataset stride")?;
        let chunk_strides = row_major_strides(&chunk_shape, "chunk stride")?;

        let mut entries = self.collect_chunk_entries(
            index_address,
            chunk_dims,
            chunk_indexing,
            ChunkEntrySelection {
                shape,
                ndim,
                elem_size,
                chunk_bounds: None,
            },
        )?;

        let full_chunk_coverage = match full_dataset_chunk_bounds(shape, &chunk_shape)? {
            Some((first_chunk, last_chunk)) => validate_chunk_grid_coverage(
                &mut entries,
                shape,
                &chunk_shape,
                &first_chunk,
                &last_chunk,
            )?,
            None if entries.is_empty() => true,
            None => {
                return Err(Error::InvalidData(
                    "chunk index contains entries for an empty dataset".into(),
                ))
            }
        };

        self.fill_output_buffer(dst);
        for entry in &entries {
            let chunk_data =
                self.load_exact_chunk_data(entry, index_address, &chunk_shape, elem_size)?;
            copy_chunk_to_flat_with_strides(
                &chunk_data,
                dst,
                ChunkCopyLayout {
                    chunk_offsets: &entry.offsets,
                    chunk_shape: &chunk_shape,
                    dataset_shape: shape,
                    dataset_strides: &dataset_strides,
                    chunk_strides: &chunk_strides,
                    elem_size,
                },
            )?;
        }

        if full_chunk_coverage && dst.len() <= HOT_FULL_DATASET_CACHE_MAX_BYTES {
            let _ = self.full_dataset_bytes.set(Arc::new(dst.to_vec()));
        }

        Ok(())
    }

    #[cfg(feature = "rayon")]
    fn read_chunked_parallel<T: H5Type>(
        &self,
        index_address: u64,
        chunk_dims: &[u32],
        _element_size: u32,
        chunk_indexing: Option<&ChunkIndexing>,
    ) -> Result<ArrayD<T>> {
        if Cursor::is_undefined_offset(index_address, self.offset_size()) {
            return self.make_fill_array::<T>();
        }

        let ndim = self.ndim();
        let shape = &self.dataspace.dims;
        let elem_size = self.raw_element_size()?;
        let total_elements = checked_usize(self.num_elements()?, "dataset element count")?;
        let total_bytes = checked_mul_usize(total_elements, elem_size, "dataset size in bytes")?;

        if total_bytes <= HOT_FULL_DATASET_CACHE_MAX_BYTES {
            if let Some(cached_bytes) = self.full_dataset_bytes.get() {
                return self.decode_raw_data::<T>(cached_bytes);
            }
        }

        let chunk_shape: Vec<u64> = chunk_dims.iter().map(|&d| d as u64).collect();
        validate_chunk_shape(shape, &chunk_shape)?;
        let dataset_strides = row_major_strides(shape, "dataset stride")?;
        let chunk_strides = row_major_strides(&chunk_shape, "chunk stride")?;

        let mut entries = self.collect_chunk_entries(
            index_address,
            chunk_dims,
            chunk_indexing,
            ChunkEntrySelection {
                shape,
                ndim,
                elem_size,
                chunk_bounds: None,
            },
        )?;

        let full_chunk_coverage = match full_dataset_chunk_bounds(shape, &chunk_shape)? {
            Some((first_chunk, last_chunk)) => validate_chunk_grid_coverage(
                &mut entries,
                shape,
                &chunk_shape,
                &first_chunk,
                &last_chunk,
            )?,
            None if entries.is_empty() => true,
            None => {
                return Err(Error::InvalidData(
                    "chunk index contains entries for an empty dataset".into(),
                ))
            }
        };
        if full_chunk_coverage {
            if T::native_copy_compatible(&self.datatype) && std::mem::size_of::<T>() == elem_size {
                let mut result_values: Vec<MaybeUninit<T>> = try_uninit_vec::<T>(total_elements)?;
                let flat = FlatBufferPtr {
                    ptr: result_values.as_mut_ptr() as *mut u8,
                    len: checked_mul_usize(
                        result_values.len(),
                        std::mem::size_of::<T>(),
                        "typed dataset size in bytes",
                    )?,
                };

                entries
                    .par_iter()
                    .map(|entry| {
                        self.load_exact_chunk_data(entry, index_address, &chunk_shape, elem_size)
                            .and_then(|data| {
                                // SAFETY: validated chunk offsets are unique,
                                // so parallel copies write disjoint ranges;
                                // `result_values` remains live through collect.
                                unsafe {
                                    flat.copy_chunk(
                                        &data,
                                        ChunkCopyLayout {
                                            chunk_offsets: &entry.offsets,
                                            chunk_shape: &chunk_shape,
                                            dataset_shape: shape,
                                            dataset_strides: &dataset_strides,
                                            chunk_strides: &chunk_strides,
                                            elem_size,
                                        },
                                    )
                                }
                            })
                    })
                    .collect::<std::result::Result<Vec<_>, Error>>()?;

                let mut result_shape = Vec::with_capacity(shape.len());
                for &dim in shape {
                    result_shape.push(checked_usize(dim, "dataset dimension")?);
                }
                if total_bytes <= HOT_FULL_DATASET_CACHE_MAX_BYTES {
                    let mut cached_bytes = try_zeroed_bytes(total_bytes)?;
                    // SAFETY: full chunk coverage initialized all
                    // `total_bytes` at `flat.ptr`, and both allocations are
                    // live and distinct.
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            flat.ptr,
                            cached_bytes.as_mut_ptr(),
                            total_bytes,
                        );
                    }
                    let _ = self.full_dataset_bytes.set(Arc::new(cached_bytes));
                }
                // SAFETY: exact, duplicate-free chunk-grid coverage was
                // validated above and all parallel copies completed.
                let result_values = unsafe { assume_init_vec(result_values) };
                return ArrayD::from_shape_vec(IxDyn(&result_shape), result_values)
                    .map_err(|e| Error::InvalidData(format!("array shape error: {e}")));
            }

            let mut flat_data = try_uninit_vec::<u8>(total_bytes)?;
            let flat = FlatBufferPtr {
                ptr: flat_data.as_mut_ptr() as *mut u8,
                len: flat_data.len(),
            };

            entries
                .par_iter()
                .map(|entry| {
                    self.load_exact_chunk_data(entry, index_address, &chunk_shape, elem_size)
                        .and_then(|data| {
                            // SAFETY: validated chunk offsets are unique, so
                            // parallel copies write disjoint ranges;
                            // `flat_data` remains live through collect.
                            unsafe {
                                flat.copy_chunk(
                                    &data,
                                    ChunkCopyLayout {
                                        chunk_offsets: &entry.offsets,
                                        chunk_shape: &chunk_shape,
                                        dataset_shape: shape,
                                        dataset_strides: &dataset_strides,
                                        chunk_strides: &chunk_strides,
                                        elem_size,
                                    },
                                )
                            }
                        })
                })
                .collect::<std::result::Result<Vec<_>, Error>>()?;

            // SAFETY: exact, duplicate-free chunk-grid coverage was validated
            // above and all parallel copies completed.
            let flat_data = unsafe { assume_init_u8_vec(flat_data) };
            if total_bytes <= HOT_FULL_DATASET_CACHE_MAX_BYTES {
                let _ = self.full_dataset_bytes.set(Arc::new(flat_data.clone()));
            }
            return self.decode_raw_data::<T>(&flat_data);
        }

        let mut flat_data = self.make_output_buffer(total_bytes)?;
        let flat = FlatBufferPtr {
            ptr: flat_data.as_mut_ptr(),
            len: flat_data.len(),
        };

        entries
            .par_iter()
            .map(|entry| {
                self.load_exact_chunk_data(entry, index_address, &chunk_shape, elem_size)
                    .and_then(|data| {
                        // SAFETY: validated chunk offsets are unique, so
                        // parallel copies write disjoint ranges; `flat_data`
                        // remains live through collect.
                        unsafe {
                            flat.copy_chunk(
                                &data,
                                ChunkCopyLayout {
                                    chunk_offsets: &entry.offsets,
                                    chunk_shape: &chunk_shape,
                                    dataset_shape: shape,
                                    dataset_strides: &dataset_strides,
                                    chunk_strides: &chunk_strides,
                                    elem_size,
                                },
                            )
                        }
                    })
            })
            .collect::<std::result::Result<Vec<_>, Error>>()?;

        self.decode_raw_data::<T>(&flat_data)
    }

    /// Collect all chunk entries by dispatching on the chunk indexing type.
    ///
    /// Shared by `read_chunked` and `read_chunked_slice`.
    fn collect_chunk_entries(
        &self,
        index_address: u64,
        chunk_dims: &[u32],
        chunk_indexing: Option<&ChunkIndexing>,
        selection: ChunkEntrySelection<'_>,
    ) -> Result<Vec<chunk_index::ChunkEntry>> {
        if selection.chunk_bounds.is_none() {
            if let Some(cached) = self.full_chunk_entries.get() {
                return Ok((**cached).clone());
            }
        }

        let cache_key =
            selection
                .chunk_bounds
                .map(|(first_chunk, last_chunk)| ChunkEntryCacheKey {
                    index_address,
                    first_chunk: SmallVec::from_slice(first_chunk),
                    last_chunk: SmallVec::from_slice(last_chunk),
                });

        if let Some(ref key) = cache_key {
            let mut cache = self.chunk_entry_cache.lock();
            if let Some(cached) = cache.get(key) {
                return Ok((**cached).clone());
            }
        }

        let entries = match chunk_indexing {
            None => {
                // V1-V3: B-tree v1 chunk indexing
                self.collect_btree_v1_entries(
                    index_address,
                    selection.ndim,
                    chunk_dims,
                    selection.chunk_bounds,
                )
            }
            Some(ChunkIndexing::SingleChunk {
                filtered_size,
                filters,
            }) => Ok(vec![chunk_index::single_chunk_entry(
                index_address,
                *filtered_size,
                *filters,
                selection.ndim,
            )]),
            Some(ChunkIndexing::BTreeV2) => chunk_index::collect_v2_chunk_entries_storage(
                self.context.storage.as_ref(),
                index_address,
                self.offset_size(),
                self.length_size(),
                selection.ndim as u32,
                chunk_dims,
                selection.chunk_bounds,
            ),
            Some(ChunkIndexing::Implicit) => chunk_index::collect_implicit_chunk_entries(
                index_address,
                selection.shape,
                chunk_dims,
                selection.elem_size,
                selection.chunk_bounds,
                self.context.storage.len(),
            ),
            Some(ChunkIndexing::FixedArray { .. }) => {
                crate::fixed_array::collect_fixed_array_chunk_entries_storage(
                    self.context.storage.as_ref(),
                    index_address,
                    self.offset_size(),
                    self.length_size(),
                    selection.shape,
                    chunk_dims,
                    selection.chunk_bounds,
                )
            }
            Some(ChunkIndexing::ExtensibleArray { .. }) => {
                crate::extensible_array::collect_extensible_array_chunk_entries_storage(
                    self.context.storage.as_ref(),
                    index_address,
                    self.offset_size(),
                    self.length_size(),
                    selection.shape,
                    chunk_dims,
                    selection.chunk_bounds,
                )
            }
        }?;

        if let Some(key) = cache_key {
            let mut cache = self.chunk_entry_cache.lock();
            cache.put(key, Arc::new(entries.clone()));
        } else {
            let _ = self.full_chunk_entries.set(Arc::new(entries.clone()));
        }

        Ok(entries)
    }

    /// Collect chunk entries from a B-tree v1 index.
    fn collect_btree_v1_entries(
        &self,
        btree_address: u64,
        ndim: usize,
        chunk_dims: &[u32],
        chunk_bounds: Option<(&[u64], &[u64])>,
    ) -> Result<Vec<chunk_index::ChunkEntry>> {
        let leaves = crate::btree_v1::collect_btree_v1_leaves_storage(
            self.context.storage.as_ref(),
            btree_address,
            self.offset_size(),
            self.length_size(),
            Some(ndim as u32),
            chunk_dims,
            chunk_bounds,
        )?;

        let mut entries = Vec::with_capacity(leaves.len());
        for (key, chunk_addr) in &leaves {
            match key {
                crate::btree_v1::BTreeV1Key::RawData {
                    chunk_size,
                    filter_mask,
                    offsets,
                } => {
                    entries.push(chunk_index::ChunkEntry {
                        address: *chunk_addr,
                        size: *chunk_size as u64,
                        filter_mask: *filter_mask,
                        offsets: offsets[..ndim].to_vec(),
                    });
                }
                _ => {
                    return Err(Error::InvalidData(
                        "expected raw data key in chunk B-tree".into(),
                    ))
                }
            }
        }
        Ok(entries)
    }

    fn load_chunk_data(
        &self,
        entry: &chunk_index::ChunkEntry,
        dataset_addr: u64,
        chunk_shape: &[u64],
        elem_size: usize,
    ) -> Result<Arc<Vec<u8>>> {
        let cache_key = ChunkKey {
            dataset_addr,
            chunk_offsets: smallvec::SmallVec::from_slice(&entry.offsets),
        };
        let expected_len = decoded_chunk_expected_len(chunk_shape, elem_size)?;
        let filter_output_limit =
            checked_add_usize(expected_len, 1, "decoded chunk filter output limit")?;

        self.chunk_cache.get_or_insert_with(cache_key, || {
            let size = if entry.size > 0 {
                checked_usize(entry.size, "encoded chunk size")?
            } else {
                expected_len
            };
            let raw = self.context.read_range(entry.address, size)?;

            if let Some(ref pipeline) = self.filters {
                filters::apply_pipeline_with_limit(
                    raw.as_ref(),
                    &pipeline.filters,
                    entry.filter_mask,
                    elem_size,
                    Some(&self.filter_registry),
                    Some(filter_output_limit),
                )
            } else {
                Ok(raw.to_vec())
            }
        })
    }

    fn load_exact_chunk_data(
        &self,
        entry: &chunk_index::ChunkEntry,
        dataset_addr: u64,
        chunk_shape: &[u64],
        elem_size: usize,
    ) -> Result<Arc<Vec<u8>>> {
        let data = self.load_chunk_data(entry, dataset_addr, chunk_shape, elem_size)?;
        validate_decoded_chunk_len(entry, chunk_shape, elem_size, data.len())?;
        Ok(data)
    }

    /// Chunked slice: only read chunks that overlap the selection.
    ///
    /// Resolves each `SliceInfoElem` to concrete ranges, computes the chunk
    /// grid range per dimension, and only decompresses overlapping chunks.
    fn read_chunked_slice<T: H5Type>(
        &self,
        index_address: u64,
        chunk_dims: &[u32],
        _element_size: u32,
        chunk_indexing: Option<&ChunkIndexing>,
        _selection: &SliceInfo,
        resolved: &ResolvedSelection,
    ) -> Result<ArrayD<T>> {
        if resolved.result_elements == 0 {
            return self.make_fill_array_from_shape::<T>(0, &resolved.result_shape);
        }

        if Cursor::is_undefined_offset(index_address, self.offset_size()) {
            return self
                .make_fill_array_from_shape::<T>(resolved.result_elements, &resolved.result_shape);
        }

        let ndim = self.ndim();
        let shape = &self.dataspace.dims;
        let elem_size = self.raw_element_size()?;

        if let Some(array) = self.try_read_single_chunk_unit_stride_2d::<T>(
            index_address,
            chunk_dims,
            chunk_indexing,
            shape,
            elem_size,
            resolved,
        )? {
            return Ok(array);
        }

        let chunk_shape: Vec<u64> = chunk_dims.iter().map(|&d| d as u64).collect();
        validate_chunk_shape(shape, &chunk_shape)?;
        let mut first_chunk = vec![0u64; ndim];
        let mut last_chunk = vec![0u64; ndim];
        for d in 0..ndim {
            let (first, last) = resolved.dims[d]
                .chunk_index_range(chunk_shape[d])
                .expect("zero-sized result handled above");
            first_chunk[d] = first;
            last_chunk[d] = last;
        }

        // Collect all chunk entries.
        let mut overlapping = self.collect_chunk_entries(
            index_address,
            chunk_dims,
            chunk_indexing,
            ChunkEntrySelection {
                shape,
                ndim,
                elem_size,
                chunk_bounds: Some((&first_chunk, &last_chunk)),
            },
        )?;
        let fully_covered_grid = validate_chunk_grid_coverage(
            &mut overlapping,
            shape,
            &chunk_shape,
            &first_chunk,
            &last_chunk,
        )?;

        let result_total_bytes = checked_mul_usize(
            resolved.result_elements,
            elem_size,
            "slice result size in bytes",
        )?;
        // Compute result strides (including collapsed dims — they have count=1).
        let result_dims = resolved.result_dims_with_collapsed();
        let mut result_strides = vec![1usize; ndim];
        for d in (0..ndim.saturating_sub(1)).rev() {
            result_strides[d] =
                checked_mul_usize(result_strides[d + 1], result_dims[d + 1], "result stride")?;
        }
        let mut chunk_strides = vec![1usize; ndim];
        for d in (0..ndim.saturating_sub(1)).rev() {
            chunk_strides[d] = checked_mul_usize(
                chunk_strides[d + 1],
                chunk_shape[d + 1] as usize,
                "chunk stride",
            )?;
        }
        let use_unit_stride_fast_path = resolved.is_unit_stride();
        let fully_covered_unit_stride = use_unit_stride_fast_path && fully_covered_grid;

        if fully_covered_unit_stride {
            if T::native_copy_compatible(&self.datatype) && std::mem::size_of::<T>() == elem_size {
                let mut result_values: Vec<MaybeUninit<T>> =
                    try_uninit_vec::<T>(resolved.result_elements)?;
                let result_ptr = result_values.as_mut_ptr() as *mut u8;
                let result_len = checked_mul_usize(
                    result_values.len(),
                    std::mem::size_of::<T>(),
                    "typed slice result size in bytes",
                )?;

                for entry in &overlapping {
                    let chunk_data =
                        self.load_exact_chunk_data(entry, index_address, &chunk_shape, elem_size)?;

                    // SAFETY: validated chunk offsets are unique, the overlap
                    // helper checks every range, and the output allocation
                    // remains live for the duration of the copy.
                    unsafe {
                        copy_unit_stride_chunk_overlap_ptr(
                            &chunk_data,
                            FlatBufferPtr {
                                ptr: result_ptr,
                                len: result_len,
                            },
                            UnitStrideCopyLayout {
                                chunk_offsets: &entry.offsets,
                                chunk_shape: &chunk_shape,
                                dataset_shape: shape,
                                resolved,
                                chunk_strides: &chunk_strides,
                                result_strides: &result_strides,
                                elem_size,
                            },
                        )?;
                    }
                }

                // SAFETY: the fully-covered selection gate proves that the
                // successful overlap copies initialized every result element.
                let result_values = unsafe { assume_init_vec(result_values) };
                return ArrayD::from_shape_vec(IxDyn(&resolved.result_shape), result_values)
                    .map_err(|e| Error::InvalidData(format!("array shape error: {e}")));
            }

            let mut result_buf = try_uninit_vec::<u8>(result_total_bytes)?;
            let result_ptr = result_buf.as_mut_ptr() as *mut u8;
            let result_len = result_buf.len();

            for entry in &overlapping {
                let chunk_data =
                    self.load_exact_chunk_data(entry, index_address, &chunk_shape, elem_size)?;

                // SAFETY: validated chunk offsets are unique, the overlap
                // helper checks every range, and the output allocation remains
                // live for the duration of the copy.
                unsafe {
                    copy_unit_stride_chunk_overlap_ptr(
                        &chunk_data,
                        FlatBufferPtr {
                            ptr: result_ptr,
                            len: result_len,
                        },
                        UnitStrideCopyLayout {
                            chunk_offsets: &entry.offsets,
                            chunk_shape: &chunk_shape,
                            dataset_shape: shape,
                            resolved,
                            chunk_strides: &chunk_strides,
                            result_strides: &result_strides,
                            elem_size,
                        },
                    )?;
                }
            }

            // SAFETY: the fully-covered selection gate proves that the
            // successful overlap copies initialized every result byte.
            let result_buf = unsafe { assume_init_u8_vec(result_buf) };
            return self.decode_buffer_with_shape::<T>(
                &result_buf,
                resolved.result_elements,
                &resolved.result_shape,
            );
        }

        let mut result_buf = self.make_output_buffer(result_total_bytes)?;

        // For each overlapping chunk: decompress and copy matching elements.
        for entry in &overlapping {
            let chunk_data =
                self.load_exact_chunk_data(entry, index_address, &chunk_shape, elem_size)?;

            if use_unit_stride_fast_path {
                copy_unit_stride_chunk_overlap(
                    &chunk_data,
                    &mut result_buf,
                    UnitStrideCopyLayout {
                        chunk_offsets: &entry.offsets,
                        chunk_shape: &chunk_shape,
                        dataset_shape: shape,
                        resolved,
                        chunk_strides: &chunk_strides,
                        result_strides: &result_strides,
                        elem_size,
                    },
                )?;
                continue;
            }

            // For each dimension, compute which elements within this chunk fall
            // within the selection.
            let mut dim_indices: Vec<Vec<(usize, usize)>> = Vec::with_capacity(ndim);
            for d in 0..ndim {
                let chunk_start = entry.offsets[d];
                let chunk_end = (chunk_start + chunk_shape[d]).min(shape[d]);
                let dim = &resolved.dims[d];
                let sel_start = dim.start;
                let sel_end = dim.end;
                let sel_step = dim.step;
                let mut indices = Vec::new();

                // Find first selected index >= chunk_start
                let first_sel = if sel_start >= chunk_start {
                    sel_start
                } else {
                    let steps_to_skip = (chunk_start - sel_start).div_ceil(sel_step);
                    sel_start + steps_to_skip * sel_step
                };

                let mut sel_idx = first_sel;
                while sel_idx < sel_end && sel_idx < chunk_end {
                    let chunk_local = checked_usize(sel_idx - chunk_start, "chunk-local index")?;
                    // Compute result-space index for this dimension.
                    let result_dim_idx =
                        checked_usize((sel_idx - dim.start) / sel_step, "result index")?;
                    indices.push((chunk_local, result_dim_idx));
                    sel_idx += sel_step;
                }

                dim_indices.push(indices);
            }

            // Iterate over the cartesian product of matching indices.
            copy_selected_elements(
                &chunk_data,
                &mut result_buf,
                &dim_indices,
                &chunk_strides,
                &result_strides,
                elem_size,
                ndim,
            )?;
        }

        self.decode_buffer_with_shape::<T>(
            &result_buf,
            resolved.result_elements,
            &resolved.result_shape,
        )
    }

    fn try_read_single_chunk_unit_stride_2d<T: H5Type>(
        &self,
        index_address: u64,
        chunk_dims: &[u32],
        chunk_indexing: Option<&ChunkIndexing>,
        shape: &[u64],
        elem_size: usize,
        resolved: &ResolvedSelection,
    ) -> Result<Option<ArrayD<T>>> {
        if shape.len() != 2
            || chunk_dims.len() != 2
            || resolved.dims.len() != 2
            || resolved.result_shape.len() != 2
            || !resolved.is_unit_stride()
            || std::mem::size_of::<T>() != elem_size
            || !T::native_copy_compatible(&self.datatype)
        {
            return Ok(None);
        }

        let chunk_shape = [u64::from(chunk_dims[0]), u64::from(chunk_dims[1])];
        validate_chunk_shape(shape, &chunk_shape)?;

        let row_dim = &resolved.dims[0];
        let col_dim = &resolved.dims[1];
        let Some((first_row_chunk, last_row_chunk)) = row_dim.chunk_index_range(chunk_shape[0])
        else {
            return Ok(None);
        };
        let Some((first_col_chunk, last_col_chunk)) = col_dim.chunk_index_range(chunk_shape[1])
        else {
            return Ok(None);
        };
        if first_row_chunk != last_row_chunk || first_col_chunk != last_col_chunk {
            return Ok(None);
        }

        let first_chunk = [first_row_chunk, first_col_chunk];
        let last_chunk = first_chunk;
        let expected_offsets = [
            checked_mul_u64(first_row_chunk, chunk_shape[0], "single chunk row offset")?,
            checked_mul_u64(
                first_col_chunk,
                chunk_shape[1],
                "single chunk column offset",
            )?,
        ];

        let entries = self.collect_chunk_entries(
            index_address,
            chunk_dims,
            chunk_indexing,
            ChunkEntrySelection {
                shape,
                ndim: 2,
                elem_size,
                chunk_bounds: Some((&first_chunk, &last_chunk)),
            },
        )?;

        if entries.is_empty() {
            return self
                .make_fill_array_from_shape::<T>(resolved.result_elements, &resolved.result_shape)
                .map(Some);
        }

        if entries.len() != 1 || entries[0].offsets.as_slice() != expected_offsets.as_slice() {
            return Ok(None);
        }

        let entry = &entries[0];
        let chunk_data =
            self.load_exact_chunk_data(entry, index_address, &chunk_shape, elem_size)?;

        let row_start = checked_usize(
            row_dim.start - expected_offsets[0],
            "single chunk slice row start",
        )?;
        let col_start = checked_usize(
            col_dim.start - expected_offsets[1],
            "single chunk slice column start",
        )?;
        let row_count = row_dim.count;
        let col_count = col_dim.count;
        let chunk_cols = checked_usize(chunk_shape[1], "single chunk column extent")?;
        let row_bytes = checked_mul_usize(col_count, elem_size, "single chunk slice row bytes")?;

        let mut result_values: Vec<MaybeUninit<T>> = try_uninit_vec::<T>(resolved.result_elements)?;
        let result_ptr = result_values.as_mut_ptr() as *mut u8;
        let result_len = checked_mul_usize(
            result_values.len(),
            std::mem::size_of::<T>(),
            "typed slice result size in bytes",
        )?;

        for row in 0..row_count {
            let chunk_row = checked_add_usize(row_start, row, "single chunk slice source row")?;
            let src_elem = checked_add_usize(
                checked_mul_usize(chunk_row, chunk_cols, "single chunk slice source row")?,
                col_start,
                "single chunk slice source element",
            )?;
            let dst_elem = checked_mul_usize(row, col_count, "single chunk slice destination row")?;

            let src_start =
                checked_mul_usize(src_elem, elem_size, "single chunk slice source byte offset")?;
            let dst_start = checked_mul_usize(
                dst_elem,
                elem_size,
                "single chunk slice destination byte offset",
            )?;
            let src_end =
                checked_add_usize(src_start, row_bytes, "single chunk slice source byte end")?;
            let dst_end = checked_add_usize(
                dst_start,
                row_bytes,
                "single chunk slice destination byte end",
            )?;

            if src_end > chunk_data.len() || dst_end > result_len {
                return Err(Error::InvalidData(format!(
                    "single chunk unit-stride slice out of bounds: source range {}..{} of {} bytes, destination range {}..{} of {} bytes",
                    src_start,
                    src_end,
                    chunk_data.len(),
                    dst_start,
                    dst_end,
                    result_len
                )));
            }

            // SAFETY: the source and destination ranges were checked above;
            // their allocations are live and distinct.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    chunk_data.as_ptr().add(src_start),
                    result_ptr.add(dst_start),
                    row_bytes,
                );
            }
        }

        // SAFETY: this fast path covers one complete unit-stride selection and
        // the row loop initialized every checked destination range.
        let result_values = unsafe { assume_init_vec(result_values) };
        ArrayD::from_shape_vec(IxDyn(&resolved.result_shape), result_values)
            .map(Some)
            .map_err(|e| Error::InvalidData(format!("array shape error: {e}")))
    }

    /// Parallel variant of `read_chunked_slice`: decompresses overlapping chunks
    /// in parallel using Rayon, then copies selected elements into the result buffer.
    ///
    /// Each chunk writes to a disjoint region of the result buffer (chunks don't
    /// overlap in output space), so this is safe to parallelize.
    #[cfg(feature = "rayon")]
    fn read_chunked_slice_parallel<T: H5Type>(
        &self,
        index_address: u64,
        chunk_dims: &[u32],
        _element_size: u32,
        chunk_indexing: Option<&ChunkIndexing>,
        _selection: &SliceInfo,
        resolved: &ResolvedSelection,
    ) -> Result<ArrayD<T>> {
        if resolved.result_elements == 0 {
            return self.make_fill_array_from_shape::<T>(0, &resolved.result_shape);
        }

        if Cursor::is_undefined_offset(index_address, self.offset_size()) {
            return self
                .make_fill_array_from_shape::<T>(resolved.result_elements, &resolved.result_shape);
        }

        let ndim = self.ndim();
        let shape = &self.dataspace.dims;
        let elem_size = self.raw_element_size()?;
        let chunk_shape: Vec<u64> = chunk_dims.iter().map(|&d| d as u64).collect();
        validate_chunk_shape(shape, &chunk_shape)?;
        let mut first_chunk = vec![0u64; ndim];
        let mut last_chunk = vec![0u64; ndim];
        for d in 0..ndim {
            let (first, last) = resolved.dims[d]
                .chunk_index_range(chunk_shape[d])
                .expect("zero-sized result handled above");
            first_chunk[d] = first;
            last_chunk[d] = last;
        }

        // Collect all chunk entries.
        let mut overlapping = self.collect_chunk_entries(
            index_address,
            chunk_dims,
            chunk_indexing,
            ChunkEntrySelection {
                shape,
                ndim,
                elem_size,
                chunk_bounds: Some((&first_chunk, &last_chunk)),
            },
        )?;
        let fully_covered_grid = validate_chunk_grid_coverage(
            &mut overlapping,
            shape,
            &chunk_shape,
            &first_chunk,
            &last_chunk,
        )?;

        // Allocate result buffer (raw bytes) initialized from fill value.
        let result_total_bytes = checked_mul_usize(
            resolved.result_elements,
            elem_size,
            "slice result size in bytes",
        )?;
        // Compute result strides (including collapsed dims — they have count=1).
        let result_dims = resolved.result_dims_with_collapsed();
        let mut result_strides = vec![1usize; ndim];
        for d in (0..ndim.saturating_sub(1)).rev() {
            result_strides[d] =
                checked_mul_usize(result_strides[d + 1], result_dims[d + 1], "result stride")?;
        }
        let mut chunk_strides = vec![1usize; ndim];
        for d in (0..ndim.saturating_sub(1)).rev() {
            chunk_strides[d] = checked_mul_usize(
                chunk_strides[d + 1],
                chunk_shape[d + 1] as usize,
                "chunk stride",
            )?;
        }
        let use_unit_stride_fast_path = resolved.is_unit_stride();
        let fully_covered_unit_stride = use_unit_stride_fast_path && fully_covered_grid;

        if fully_covered_unit_stride {
            if T::native_copy_compatible(&self.datatype) && std::mem::size_of::<T>() == elem_size {
                let mut result_values: Vec<MaybeUninit<T>> =
                    try_uninit_vec::<T>(resolved.result_elements)?;
                let flat = FlatBufferPtr {
                    ptr: result_values.as_mut_ptr() as *mut u8,
                    len: checked_mul_usize(
                        result_values.len(),
                        std::mem::size_of::<T>(),
                        "typed slice result size in bytes",
                    )?,
                };

                overlapping
                    .par_iter()
                    .map(|entry| {
                        let chunk_data = self.load_exact_chunk_data(
                            entry,
                            index_address,
                            &chunk_shape,
                            elem_size,
                        )?;

                        // SAFETY: validated chunk offsets are unique, so
                        // parallel overlap copies write disjoint ranges;
                        // `result_values` remains live through collect.
                        unsafe {
                            flat.copy_unit_stride_chunk_overlap(
                                &chunk_data,
                                UnitStrideCopyLayout {
                                    chunk_offsets: &entry.offsets,
                                    chunk_shape: &chunk_shape,
                                    dataset_shape: shape,
                                    resolved,
                                    chunk_strides: &chunk_strides,
                                    result_strides: &result_strides,
                                    elem_size,
                                },
                            )?;
                        }

                        Ok(())
                    })
                    .collect::<std::result::Result<Vec<_>, Error>>()?;

                // SAFETY: the fully-covered selection gate proves that all
                // successful parallel copies initialized the result.
                let result_values = unsafe { assume_init_vec(result_values) };
                return ArrayD::from_shape_vec(IxDyn(&resolved.result_shape), result_values)
                    .map_err(|e| Error::InvalidData(format!("array shape error: {e}")));
            }

            let mut result_buf = try_uninit_vec::<u8>(result_total_bytes)?;
            let flat = FlatBufferPtr {
                ptr: result_buf.as_mut_ptr() as *mut u8,
                len: result_buf.len(),
            };

            overlapping
                .par_iter()
                .map(|entry| {
                    let chunk_data =
                        self.load_exact_chunk_data(entry, index_address, &chunk_shape, elem_size)?;

                    // SAFETY: validated chunk offsets are unique, so parallel
                    // overlap copies write disjoint ranges; `result_buf`
                    // remains live through collect.
                    unsafe {
                        flat.copy_unit_stride_chunk_overlap(
                            &chunk_data,
                            UnitStrideCopyLayout {
                                chunk_offsets: &entry.offsets,
                                chunk_shape: &chunk_shape,
                                dataset_shape: shape,
                                resolved,
                                chunk_strides: &chunk_strides,
                                result_strides: &result_strides,
                                elem_size,
                            },
                        )?;
                    }

                    Ok(())
                })
                .collect::<std::result::Result<Vec<_>, Error>>()?;

            // SAFETY: the fully-covered selection gate proves that all
            // successful parallel copies initialized every result byte.
            let result_buf = unsafe { assume_init_u8_vec(result_buf) };
            return self.decode_buffer_with_shape::<T>(
                &result_buf,
                resolved.result_elements,
                &resolved.result_shape,
            );
        }

        let mut result_buf = self.make_output_buffer(result_total_bytes)?;

        let flat = FlatBufferPtr {
            ptr: result_buf.as_mut_ptr(),
            len: result_buf.len(),
        };

        overlapping
            .par_iter()
            .map(|entry| {
                let chunk_data =
                    self.load_exact_chunk_data(entry, index_address, &chunk_shape, elem_size)?;

                if use_unit_stride_fast_path {
                    // SAFETY: validated chunk offsets are unique, so parallel
                    // overlap copies write disjoint ranges; `result_buf`
                    // remains live through collect.
                    unsafe {
                        flat.copy_unit_stride_chunk_overlap(
                            &chunk_data,
                            UnitStrideCopyLayout {
                                chunk_offsets: &entry.offsets,
                                chunk_shape: &chunk_shape,
                                dataset_shape: shape,
                                resolved,
                                chunk_strides: &chunk_strides,
                                result_strides: &result_strides,
                                elem_size,
                            },
                        )?;
                    }
                    return Ok(());
                }

                // For each dimension, compute which elements within this chunk fall
                // within the selection.
                let mut dim_indices: Vec<Vec<(usize, usize)>> = Vec::with_capacity(ndim);
                for d in 0..ndim {
                    let chunk_start = entry.offsets[d];
                    let chunk_end = (chunk_start + chunk_shape[d]).min(shape[d]);
                    let dim = &resolved.dims[d];
                    let sel_start = dim.start;
                    let sel_end = dim.end;
                    let sel_step = dim.step;
                    let mut indices = Vec::new();

                    let first_sel = if sel_start >= chunk_start {
                        sel_start
                    } else {
                        let steps_to_skip = (chunk_start - sel_start).div_ceil(sel_step);
                        sel_start + steps_to_skip * sel_step
                    };

                    let mut sel_idx = first_sel;
                    while sel_idx < sel_end && sel_idx < chunk_end {
                        let chunk_local =
                            checked_usize(sel_idx - chunk_start, "chunk-local index")?;
                        let result_dim_idx =
                            checked_usize((sel_idx - dim.start) / sel_step, "result index")?;
                        indices.push((chunk_local, result_dim_idx));
                        sel_idx += sel_step;
                    }

                    dim_indices.push(indices);
                }

                // SAFETY: each chunk writes to disjoint output positions because
                // chunks occupy non-overlapping regions of the dataset grid and
                // the selection maps each dataset coordinate to a unique result index.
                unsafe {
                    flat.copy_selected(
                        &chunk_data,
                        &dim_indices,
                        &chunk_strides,
                        &result_strides,
                        elem_size,
                        ndim,
                    )?;
                }

                Ok(())
            })
            .collect::<std::result::Result<Vec<_>, Error>>()?;

        self.decode_buffer_with_shape::<T>(
            &result_buf,
            resolved.result_elements,
            &resolved.result_shape,
        )
    }

    fn read_contiguous_slice<T: H5Type>(
        &self,
        address: u64,
        size: u64,
        resolved: &ResolvedSelection,
    ) -> Result<ArrayD<T>> {
        if resolved.result_elements == 0 {
            return self.make_fill_array_from_shape::<T>(0, &resolved.result_shape);
        }

        if self.external_files.is_none()
            && (Cursor::is_undefined_offset(address, self.offset_size()) || size == 0)
        {
            return self
                .make_fill_array_from_shape::<T>(resolved.result_elements, &resolved.result_shape);
        }
        if self.external_files.is_none() {
            self.validate_allocated_raw_data_len(
                "contiguous",
                checked_usize(size, "contiguous dataset size")?,
            )?;
        }

        let shape = &self.dataspace.dims;
        if selection_covers_full_dataset(resolved, shape) {
            return self.read_contiguous::<T>(address, size);
        }

        let elem_size = self.raw_element_size()?;
        let result_total_bytes = checked_mul_usize(
            resolved.result_elements,
            elem_size,
            "contiguous slice result size in bytes",
        )?;
        let dataset_strides = row_major_strides(shape, "contiguous dataset stride")?;
        let result_dims = resolved.result_dims_with_collapsed();
        let result_strides = result_strides_for_dims(&result_dims)?;
        let result_buf = self.read_contiguous_slice_bytes_direct(
            address,
            size,
            resolved,
            ContiguousSliceDirectLayout {
                dataset_strides: &dataset_strides,
                result_strides: &result_strides,
                elem_size,
                result_total_bytes,
            },
        )?;

        self.decode_buffer_with_shape::<T>(
            &result_buf,
            resolved.result_elements,
            &resolved.result_shape,
        )
    }

    fn read_contiguous_slice_bytes_direct(
        &self,
        address: u64,
        size: u64,
        resolved: &ResolvedSelection,
        layout: ContiguousSliceDirectLayout<'_>,
    ) -> Result<Vec<u8>> {
        let shape = &self.dataspace.dims;
        let ndim = shape.len();
        if resolved.dims.len() != ndim
            || layout.dataset_strides.len() != ndim
            || layout.result_strides.len() != ndim
        {
            return Err(Error::InvalidData(format!(
                "contiguous slice layout rank does not match dataset rank {ndim}"
            )));
        }

        let storage_len = if self.external_files.is_some() {
            checked_mul_usize(
                checked_usize(self.num_elements()?, "dataset element count")?,
                layout.elem_size,
                "external dataset size",
            )?
        } else {
            checked_usize(size, "contiguous dataset size")?
        };
        let tail_start = contiguous_slice_tail_start(shape, resolved);
        let block_elements = contiguous_slice_block_elements(resolved, tail_start)?;
        let block_bytes = checked_mul_usize(
            block_elements,
            layout.elem_size,
            "contiguous slice block size in bytes",
        )?;
        let mut result_buf = self.make_output_buffer(layout.result_total_bytes)?;

        let prefix_blocks =
            resolved.dims[..tail_start]
                .iter()
                .try_fold(1usize, |acc, dim| -> Result<usize> {
                    checked_mul_usize(acc, dim.count, "contiguous slice block count")
                })?;
        let mut counters = vec![0usize; tail_start];

        for _ in 0..prefix_blocks {
            let mut source_elem = 0usize;
            let mut result_elem = 0usize;

            for (d, &counter) in counters.iter().enumerate().take(tail_start) {
                let ordinal = u64::try_from(counter).map_err(|_| {
                    Error::InvalidData("contiguous slice ordinal exceeds u64".to_string())
                })?;
                let coord = checked_add_u64(
                    resolved.dims[d].start,
                    checked_mul_u64(
                        ordinal,
                        resolved.dims[d].step,
                        "contiguous slice coordinate",
                    )?,
                    "contiguous slice coordinate",
                )?;
                let coord = checked_usize(coord, "contiguous slice source index")?;
                let source_term =
                    checked_mul_usize(coord, layout.dataset_strides[d], "contiguous slice source")?;
                let result_term = checked_mul_usize(
                    counter,
                    layout.result_strides[d],
                    "contiguous slice result",
                )?;
                source_elem =
                    checked_add_usize(source_elem, source_term, "contiguous slice source")?;
                result_elem =
                    checked_add_usize(result_elem, result_term, "contiguous slice result")?;
            }

            for (d, &dataset_stride) in layout
                .dataset_strides
                .iter()
                .enumerate()
                .take(ndim)
                .skip(tail_start)
            {
                let coord = checked_usize(resolved.dims[d].start, "contiguous slice source index")?;
                let source_term =
                    checked_mul_usize(coord, dataset_stride, "contiguous slice source")?;
                source_elem =
                    checked_add_usize(source_elem, source_term, "contiguous slice source")?;
            }

            let source_start = checked_mul_usize(
                source_elem,
                layout.elem_size,
                "contiguous slice source byte offset",
            )?;
            let source_end = checked_add_usize(
                source_start,
                block_bytes,
                "contiguous slice source byte end",
            )?;
            if source_end > storage_len {
                return Err(Error::InvalidData(format!(
                    "contiguous slice range {}..{} exceeds dataset storage size {}",
                    source_start, source_end, storage_len
                )));
            }

            let dst_start = checked_mul_usize(
                result_elem,
                layout.elem_size,
                "contiguous slice destination byte offset",
            )?;
            let dst_end = checked_add_usize(
                dst_start,
                block_bytes,
                "contiguous slice destination byte end",
            )?;
            if dst_end > result_buf.len() {
                return Err(Error::InvalidData(format!(
                    "contiguous slice destination range {}..{} exceeds result size {}",
                    dst_start,
                    dst_end,
                    result_buf.len()
                )));
            }

            let block = self.read_contiguous_logical_range(address, source_start, block_bytes)?;
            if block.len() != block_bytes {
                return Err(Error::InvalidData(format!(
                    "contiguous slice read returned {} bytes, expected {}",
                    block.len(),
                    block_bytes
                )));
            }
            result_buf[dst_start..dst_end].copy_from_slice(&block);

            let mut carry = true;
            for d in (0..tail_start).rev() {
                if carry {
                    counters[d] += 1;
                    if counters[d] < resolved.dims[d].count {
                        carry = false;
                    } else {
                        counters[d] = 0;
                    }
                }
            }
        }

        Ok(result_buf)
    }

    fn read_compact_slice<T: H5Type>(
        &self,
        data: &[u8],
        selection: &SliceInfo,
    ) -> Result<ArrayD<T>> {
        let full = self.read_compact::<T>(data)?;
        slice_array(&full, selection, &self.dataspace.dims)
    }

    fn decode_buffer_with_shape<T: H5Type>(
        &self,
        raw: &[u8],
        n: usize,
        shape: &[usize],
    ) -> Result<ArrayD<T>> {
        let elem_size = self.raw_element_size()?;
        let expected_bytes = checked_mul_usize(n, elem_size, "decoded buffer byte length")?;
        if raw.len() != expected_bytes {
            return Err(Error::InvalidData(format!(
                "decoded buffer has {} bytes, expected {} bytes",
                raw.len(),
                expected_bytes
            )));
        }

        if let Some(elements) = T::decode_vec(raw, &self.datatype, n) {
            let elements = elements?;
            return ArrayD::from_shape_vec(IxDyn(shape), elements)
                .map_err(|e| Error::InvalidData(format!("array shape error: {e}")));
        }

        let mut elements = Vec::with_capacity(n);
        for i in 0..n {
            let start = checked_mul_usize(i, elem_size, "decoded element byte offset")?;
            let end = checked_mul_usize(i + 1, elem_size, "decoded element end offset")?;
            elements.push(T::from_bytes(&raw[start..end], &self.datatype)?);
        }

        ArrayD::from_shape_vec(IxDyn(shape), elements)
            .map_err(|e| Error::InvalidData(format!("array shape error: {e}")))
    }

    fn decode_raw_data<T: H5Type>(&self, raw: &[u8]) -> Result<ArrayD<T>> {
        let n = checked_usize(self.num_elements()?, "dataset element count")?;
        let mut shape = Vec::with_capacity(self.dataspace.dims.len());
        for &dim in &self.dataspace.dims {
            shape.push(checked_usize(dim, "dataset dimension")?);
        }
        self.decode_buffer_with_shape::<T>(raw, n, &shape)
    }

    fn make_fill_array<T: H5Type>(&self) -> Result<ArrayD<T>> {
        let n = checked_usize(self.num_elements()?, "dataset element count")?;
        let mut shape = Vec::with_capacity(self.dataspace.dims.len());
        for &dim in &self.dataspace.dims {
            shape.push(checked_usize(dim, "dataset dimension")?);
        }
        self.make_fill_array_from_shape::<T>(n, &shape)
    }

    fn make_fill_array_from_shape<T: H5Type>(
        &self,
        element_count: usize,
        shape: &[usize],
    ) -> Result<ArrayD<T>> {
        let elem_size = self.raw_element_size()?;
        let total_bytes = checked_mul_usize(element_count, elem_size, "fill result size in bytes")?;
        let fill = self.make_output_buffer(total_bytes)?;
        self.decode_buffer_with_shape::<T>(&fill, element_count, shape)
    }

    fn make_output_buffer(&self, total_bytes: usize) -> Result<Vec<u8>> {
        let mut buf = try_zeroed_bytes(total_bytes)?;
        self.fill_output_buffer(&mut buf);
        Ok(buf)
    }

    fn fill_output_buffer(&self, buf: &mut [u8]) {
        buf.fill(0);
        if let Some(ref fv) = self.fill_value {
            if let Some(ref fill_bytes) = fv.value {
                if !fill_bytes.is_empty() {
                    for chunk in buf.chunks_exact_mut(fill_bytes.len()) {
                        chunk.copy_from_slice(fill_bytes);
                    }
                }
            }
        }
    }

    fn validate_allocated_raw_data_len(&self, storage_kind: &str, actual_len: usize) -> Result<()> {
        let expected_len = self.raw_byte_len()?;
        if actual_len != expected_len {
            return Err(Error::InvalidData(format!(
                "{storage_kind} raw data has {actual_len} bytes, expected {expected_len} bytes"
            )));
        }
        Ok(())
    }

    fn convert_to_native_endian(&self, bytes: &mut [u8]) -> Result<()> {
        let count = checked_usize(self.num_elements()?, "dataset element count")?;
        convert_datatype_to_native_endian(&self.datatype, self.vlen_reference_size(), bytes, count)
    }
}

fn native_byte_order() -> ByteOrder {
    if cfg!(target_endian = "little") {
        ByteOrder::LittleEndian
    } else {
        ByteOrder::BigEndian
    }
}

fn convert_datatype_to_native_endian(
    dtype: &Datatype,
    vlen_reference_size: usize,
    bytes: &mut [u8],
    count: usize,
) -> Result<()> {
    match dtype {
        Datatype::FixedPoint {
            size, byte_order, ..
        }
        | Datatype::FloatingPoint { size, byte_order }
        | Datatype::Bitfield { size, byte_order } => {
            swap_elements_to_native(bytes, count, *size as usize, *byte_order)
        }
        Datatype::Enum { base, .. } => {
            convert_datatype_to_native_endian(base, vlen_reference_size, bytes, count)
        }
        Datatype::Array { base, dims } => {
            let array_count = dims.iter().try_fold(1usize, |acc, &dim| {
                checked_mul_usize(
                    acc,
                    checked_usize(dim, "array datatype dimension")?,
                    "array datatype element count",
                )
            })?;
            let total_count =
                checked_mul_usize(count, array_count, "array datatype total element count")?;
            convert_datatype_to_native_endian(base, vlen_reference_size, bytes, total_count)
        }
        Datatype::Compound { size, fields } => {
            let record_size = *size as usize;
            let required = checked_mul_usize(count, record_size, "compound byte length")?;
            if bytes.len() < required {
                return Err(Error::InvalidData(format!(
                    "compound native-endian conversion needs {required} bytes, got {}",
                    bytes.len()
                )));
            }

            for record in 0..count {
                let record_start =
                    checked_mul_usize(record, record_size, "compound record byte offset")?;
                for field in fields {
                    let field_offset = field.byte_offset as usize;
                    let field_size =
                        raw_element_size_for_datatype(&field.datatype, vlen_reference_size)?;
                    let field_start = checked_add_usize(
                        record_start,
                        field_offset,
                        "compound field byte offset",
                    )?;
                    let field_end =
                        checked_add_usize(field_start, field_size, "compound field byte end")?;
                    if field_end > bytes.len() || field_offset + field_size > record_size {
                        return Err(Error::InvalidData(format!(
                            "compound field '{}' range exceeds record size",
                            field.name
                        )));
                    }
                    convert_datatype_to_native_endian(
                        &field.datatype,
                        vlen_reference_size,
                        &mut bytes[field_start..field_end],
                        1,
                    )?;
                }
            }
            Ok(())
        }
        Datatype::String { .. }
        | Datatype::VarLen { .. }
        | Datatype::Opaque { .. }
        | Datatype::Reference { .. } => Ok(()),
    }
}

fn swap_elements_to_native(
    bytes: &mut [u8],
    count: usize,
    elem_size: usize,
    byte_order: ByteOrder,
) -> Result<()> {
    let required = checked_mul_usize(count, elem_size, "native-endian byte length")?;
    if bytes.len() < required {
        return Err(Error::InvalidData(format!(
            "native-endian conversion needs {required} bytes, got {}",
            bytes.len()
        )));
    }

    if elem_size <= 1 || byte_order == native_byte_order() {
        return Ok(());
    }

    for chunk in bytes[..required].chunks_exact_mut(elem_size) {
        chunk.reverse();
    }
    Ok(())
}

fn attribute_from_message_storage(message: &AttributeMessage, context: &FileContext) -> Attribute {
    let decoded_strings = message
        .dataspace
        .num_elements()
        .ok()
        .and_then(|element_count| {
            decoded_vlen_strings_storage(
                &message.datatype,
                &message.raw_data,
                context.storage.as_ref(),
                context.superblock.offset_size,
                context.superblock.length_size,
                element_count,
            )
        });
    let raw_data = match &message.datatype {
        Datatype::VarLen {
            base,
            kind: VarLenKind::String,
            ..
        } if base.is_vlen_string_base() && matches!(message.dataspace.num_elements(), Ok(1)) => {
            resolve_vlen_bytes_storage(
                &message.raw_data,
                context.storage.as_ref(),
                context.superblock.offset_size,
                context.superblock.length_size,
            )
            .unwrap_or_else(|| message.raw_data.clone())
        }
        _ => message.raw_data.clone(),
    };

    Attribute {
        name: message.name.clone(),
        datatype: message.datatype.clone(),
        shape: match message.dataspace.dataspace_type {
            DataspaceType::Scalar => vec![],
            DataspaceType::Null => vec![0],
            DataspaceType::Simple => message.dataspace.dims.clone(),
        },
        raw_data,
        decoded_strings,
    }
}

fn normalize_layout(layout: DataLayout, dataspace: &DataspaceMessage) -> DataLayout {
    match layout {
        DataLayout::Chunked {
            address,
            mut dims,
            mut element_size,
            chunk_indexing,
        } if dims.len() == dataspace.dims.len() + 1 => {
            if let Some(legacy_element_size) = dims.pop() {
                if element_size == 0 {
                    element_size = legacy_element_size;
                }
            }
            DataLayout::Chunked {
                address,
                dims,
                element_size,
                chunk_indexing,
            }
        }
        other => other,
    }
}

fn raw_element_size_for_datatype(dtype: &Datatype, vlen_reference_size: usize) -> Result<usize> {
    match dtype {
        Datatype::String {
            size: StringSize::Variable,
            ..
        }
        | Datatype::VarLen { .. } => Ok(vlen_reference_size),
        Datatype::Array { base, dims } => {
            let base_size = raw_element_size_for_datatype(base, vlen_reference_size)?;
            let count = dims.iter().try_fold(1usize, |acc, &dim| {
                let dim = checked_usize(dim, "array datatype dimension")?;
                checked_mul_usize(acc, dim, "array datatype element count")
            })?;
            checked_mul_usize(base_size, count, "array datatype byte size")
        }
        Datatype::Enum { base, .. } => raw_element_size_for_datatype(base, vlen_reference_size),
        Datatype::FixedPoint { size, .. }
        | Datatype::FloatingPoint { size, .. }
        | Datatype::Bitfield { size, .. }
        | Datatype::Reference { size, .. } => Ok(*size as usize),
        Datatype::String {
            size: StringSize::Fixed(len),
            ..
        } => Ok(*len as usize),
        Datatype::Compound { size, .. } | Datatype::Opaque { size, .. } => Ok(*size as usize),
    }
}

#[cfg(test)]
/// Copy a chunk's data into the flat output buffer at the correct position.
fn copy_chunk_to_flat(
    chunk_data: &[u8],
    flat: &mut [u8],
    chunk_offsets: &[u64],
    chunk_shape: &[u64],
    dataset_shape: &[u64],
    elem_size: usize,
) -> Result<()> {
    let dataset_strides = row_major_strides(dataset_shape, "dataset stride")
        .expect("dataset strides should fit in usize");
    let chunk_strides =
        row_major_strides(chunk_shape, "chunk stride").expect("chunk strides should fit in usize");
    copy_chunk_to_flat_with_strides(
        chunk_data,
        flat,
        ChunkCopyLayout {
            chunk_offsets,
            chunk_shape,
            dataset_shape,
            dataset_strides: &dataset_strides,
            chunk_strides: &chunk_strides,
            elem_size,
        },
    )
}

fn copy_chunk_to_flat_with_strides(
    chunk_data: &[u8],
    flat: &mut [u8],
    layout: ChunkCopyLayout<'_>,
) -> Result<()> {
    // SAFETY: the mutable slice supplies a live, exclusively borrowed output
    // allocation, and the pointer helper checks every source and destination
    // range.
    unsafe {
        copy_chunk_to_flat_with_strides_ptr(
            chunk_data,
            FlatBufferPtr {
                ptr: flat.as_mut_ptr(),
                len: flat.len(),
            },
            layout,
        )
    }
}

#[inline(always)]
unsafe fn copy_chunk_to_flat_with_strides_ptr(
    chunk_data: &[u8],
    flat: FlatBufferPtr,
    layout: ChunkCopyLayout<'_>,
) -> Result<()> {
    let ndim = layout.dataset_shape.len();
    if layout.chunk_offsets.len() != ndim
        || layout.chunk_shape.len() != ndim
        || layout.dataset_strides.len() != ndim
        || layout.chunk_strides.len() != ndim
    {
        return Err(Error::InvalidData(format!(
            "chunk copy layout rank does not match dataset rank {ndim}"
        )));
    }

    if ndim == 0 {
        if chunk_data.len() < layout.elem_size || flat.len < layout.elem_size {
            return Err(Error::InvalidData(format!(
                "scalar chunk copy requires {} bytes, got source {} and destination {}",
                layout.elem_size,
                chunk_data.len(),
                flat.len
            )));
        }
        // SAFETY: both ranges were checked immediately above, and the source
        // slice and caller-owned output allocation are distinct.
        unsafe {
            std::ptr::copy_nonoverlapping(chunk_data.as_ptr(), flat.ptr, layout.elem_size);
        }
        return Ok(());
    }

    // Total elements in this chunk (clamped to dataset boundaries)
    let mut actual_chunk_shape = Vec::with_capacity(ndim);
    for i in 0..ndim {
        if layout.chunk_offsets[i] >= layout.dataset_shape[i] {
            return Err(Error::InvalidData(format!(
                "chunk offset {} is outside dimension {} of size {}",
                layout.chunk_offsets[i], i, layout.dataset_shape[i]
            )));
        }
        let remaining = layout.dataset_shape[i] - layout.chunk_offsets[i];
        actual_chunk_shape.push(checked_usize(
            remaining.min(layout.chunk_shape[i]),
            "actual chunk extent",
        )?);
    }

    let row_elems = *actual_chunk_shape.last().unwrap_or(&1);
    let row_bytes = checked_mul_usize(row_elems, layout.elem_size, "chunk row bytes")?;
    let mut dataset_origin = 0usize;
    for (d, offset) in layout.chunk_offsets.iter().enumerate() {
        let offset = checked_usize(*offset, "chunk offset")?;
        let term = checked_mul_usize(offset, layout.dataset_strides[d], "chunk origin")?;
        dataset_origin = checked_add_usize(dataset_origin, term, "chunk origin")?;
    }

    if ndim == 1 {
        let dst_start = checked_mul_usize(dataset_origin, layout.elem_size, "chunk dst offset")?;
        let dst_end = checked_add_usize(dst_start, row_bytes, "chunk dst end")?;
        if row_bytes > chunk_data.len() || dst_end > flat.len {
            return Err(Error::InvalidData(format!(
                "chunk copy out of bounds: source row needs {} bytes from {} bytes, destination range {}..{} exceeds {} bytes",
                row_bytes,
                chunk_data.len(),
                dst_start,
                dst_end,
                flat.len
            )));
        }
        // SAFETY: both ranges were checked immediately above, and the source
        // slice and caller-owned output allocation are distinct.
        unsafe {
            std::ptr::copy_nonoverlapping(chunk_data.as_ptr(), flat.ptr.add(dst_start), row_bytes);
        }
        return Ok(());
    }

    let outer_dims = &actual_chunk_shape[..ndim - 1];
    let total_rows = checked_product_usize(outer_dims, "chunk row count")?;
    let mut outer_idx = vec![0usize; ndim - 1];

    for _ in 0..total_rows {
        let mut chunk_row = 0usize;
        let mut dataset_row = dataset_origin;
        for (d, outer) in outer_idx.iter().copied().enumerate() {
            let chunk_term = checked_mul_usize(outer, layout.chunk_strides[d], "chunk row")?;
            let dataset_term = checked_mul_usize(outer, layout.dataset_strides[d], "dataset row")?;
            chunk_row = checked_add_usize(chunk_row, chunk_term, "chunk row")?;
            dataset_row = checked_add_usize(dataset_row, dataset_term, "dataset row")?;
        }

        let src_start = checked_mul_usize(chunk_row, layout.elem_size, "chunk src offset")?;
        let dst_start = checked_mul_usize(dataset_row, layout.elem_size, "chunk dst offset")?;
        let src_end = checked_add_usize(src_start, row_bytes, "chunk src end")?;
        let dst_end = checked_add_usize(dst_start, row_bytes, "chunk dst end")?;
        if src_end > chunk_data.len() || dst_end > flat.len {
            return Err(Error::InvalidData(format!(
                "chunk copy out of bounds: source range {}..{} of {} bytes, destination range {}..{} of {} bytes",
                src_start,
                src_end,
                chunk_data.len(),
                dst_start,
                dst_end,
                flat.len
            )));
        }
        // SAFETY: both ranges were checked immediately above, and the source
        // slice and caller-owned output allocation are distinct.
        unsafe {
            std::ptr::copy_nonoverlapping(
                chunk_data.as_ptr().add(src_start),
                flat.ptr.add(dst_start),
                row_bytes,
            );
        }

        let mut carry = true;
        for d in (0..outer_idx.len()).rev() {
            if carry {
                outer_idx[d] += 1;
                if outer_idx[d] < outer_dims[d] {
                    carry = false;
                } else {
                    outer_idx[d] = 0;
                }
            }
        }
    }

    Ok(())
}

fn checked_product_usize(values: &[usize], context: &str) -> Result<usize> {
    let mut product = 1usize;
    for &value in values {
        product = checked_mul_usize(product, value, context)?;
    }
    Ok(product)
}

fn unit_stride_chunk_overlap_plan(
    chunk_offsets: &[u64],
    chunk_shape: &[u64],
    dataset_shape: &[u64],
    resolved: &ResolvedSelection,
) -> Result<(Vec<usize>, Vec<usize>, Vec<usize>)> {
    let ndim = dataset_shape.len();
    let mut overlap_counts = Vec::with_capacity(ndim);
    let mut chunk_local_start = Vec::with_capacity(ndim);
    let mut result_start = Vec::with_capacity(ndim);

    for d in 0..ndim {
        let chunk_start = chunk_offsets[d];
        let chunk_end = (chunk_start + chunk_shape[d]).min(dataset_shape[d]);
        let dim = &resolved.dims[d];
        let overlap_start = chunk_start.max(dim.start);
        let overlap_end = chunk_end.min(dim.end);
        if overlap_start >= overlap_end {
            return Ok((Vec::new(), Vec::new(), Vec::new()));
        }

        overlap_counts.push(checked_usize(
            overlap_end - overlap_start,
            "chunk overlap size",
        )?);
        chunk_local_start.push(checked_usize(
            overlap_start - chunk_start,
            "chunk overlap start",
        )?);
        result_start.push(checked_usize(
            overlap_start - dim.start,
            "slice result overlap start",
        )?);
    }

    Ok((overlap_counts, chunk_local_start, result_start))
}

#[inline(always)]
fn copy_unit_stride_chunk_overlap(
    chunk_data: &[u8],
    result_buf: &mut [u8],
    layout: UnitStrideCopyLayout<'_>,
) -> Result<()> {
    // SAFETY: the mutable slice supplies a live, exclusively borrowed output
    // allocation, and the pointer helper checks every source and destination
    // range.
    unsafe {
        copy_unit_stride_chunk_overlap_ptr(
            chunk_data,
            FlatBufferPtr {
                ptr: result_buf.as_mut_ptr(),
                len: result_buf.len(),
            },
            layout,
        )
    }
}

/// Copy a unit-step rectangular overlap from a chunk into the result buffer.
///
/// This is the hot path for contiguous hyperslab reads over chunked datasets:
/// rather than copying one element at a time, it copies contiguous runs along
/// the innermost dimension with a single memcpy per output row.
///
/// # Safety
///
/// The caller must guarantee that `[result_ptr .. result_ptr + result_len)` is
/// valid for writes. Concurrent callers must write to disjoint byte ranges.
#[inline(always)]
unsafe fn copy_unit_stride_chunk_overlap_ptr(
    chunk_data: &[u8],
    result: FlatBufferPtr,
    layout: UnitStrideCopyLayout<'_>,
) -> Result<()> {
    let ndim = layout.dataset_shape.len();
    if layout.chunk_offsets.len() != ndim
        || layout.chunk_shape.len() != ndim
        || layout.resolved.dims.len() != ndim
        || layout.chunk_strides.len() != ndim
        || layout.result_strides.len() != ndim
    {
        return Err(Error::InvalidData(format!(
            "unit-stride copy layout rank does not match dataset rank {ndim}"
        )));
    }

    if ndim == 0 {
        if chunk_data.len() < layout.elem_size || result.len < layout.elem_size {
            return Err(Error::InvalidData(format!(
                "scalar slice copy requires {} bytes, got source {} and destination {}",
                layout.elem_size,
                chunk_data.len(),
                result.len
            )));
        }
        // SAFETY: both ranges were checked immediately above, and the source
        // slice and caller-owned output allocation are distinct.
        unsafe {
            std::ptr::copy_nonoverlapping(chunk_data.as_ptr(), result.ptr, layout.elem_size);
        }
        return Ok(());
    }

    let (overlap_counts, chunk_local_start, result_start) = unit_stride_chunk_overlap_plan(
        layout.chunk_offsets,
        layout.chunk_shape,
        layout.dataset_shape,
        layout.resolved,
    )?;
    if overlap_counts.is_empty() {
        return Ok(());
    }

    let row_elems = *overlap_counts.last().unwrap_or(&1);
    let row_bytes = checked_mul_usize(row_elems, layout.elem_size, "unit-stride slice row bytes")?;

    let mut chunk_origin = 0usize;
    let mut result_origin = 0usize;
    for d in 0..ndim {
        let chunk_term = checked_mul_usize(
            chunk_local_start[d],
            layout.chunk_strides[d],
            "chunk overlap origin",
        )?;
        let result_term = checked_mul_usize(
            result_start[d],
            layout.result_strides[d],
            "slice result origin",
        )?;
        chunk_origin = checked_add_usize(chunk_origin, chunk_term, "chunk overlap origin")?;
        result_origin = checked_add_usize(result_origin, result_term, "slice result origin")?;
    }

    if ndim == 1 {
        let src_start = checked_mul_usize(chunk_origin, layout.elem_size, "slice src offset")?;
        let dst_start = checked_mul_usize(result_origin, layout.elem_size, "slice dst offset")?;
        let src_end = checked_add_usize(src_start, row_bytes, "slice src end")?;
        let dst_end = checked_add_usize(dst_start, row_bytes, "slice dst end")?;
        if src_end > chunk_data.len() || dst_end > result.len {
            return Err(Error::InvalidData(format!(
                "unit-stride slice copy out of bounds: source range {}..{} of {} bytes, destination range {}..{} of {} bytes",
                src_start,
                src_end,
                chunk_data.len(),
                dst_start,
                dst_end,
                result.len
            )));
        }
        // SAFETY: both ranges were checked immediately above, and the source
        // slice and caller-owned output allocation are distinct.
        unsafe {
            std::ptr::copy_nonoverlapping(
                chunk_data.as_ptr().add(src_start),
                result.ptr.add(dst_start),
                row_bytes,
            );
        }
        return Ok(());
    }

    let outer_counts = &overlap_counts[..ndim - 1];
    let total_rows = checked_product_usize(outer_counts, "unit-stride slice row count")?;
    let mut outer_idx = vec![0usize; ndim - 1];

    for _ in 0..total_rows {
        let mut chunk_row = chunk_origin;
        let mut result_row = result_origin;
        for (d, outer) in outer_idx.iter().copied().enumerate() {
            let chunk_term = checked_mul_usize(outer, layout.chunk_strides[d], "slice chunk row")?;
            let result_term =
                checked_mul_usize(outer, layout.result_strides[d], "slice result row")?;
            chunk_row = checked_add_usize(chunk_row, chunk_term, "slice chunk row")?;
            result_row = checked_add_usize(result_row, result_term, "slice result row")?;
        }

        let src_start = checked_mul_usize(chunk_row, layout.elem_size, "slice src offset")?;
        let dst_start = checked_mul_usize(result_row, layout.elem_size, "slice dst offset")?;
        let src_end = checked_add_usize(src_start, row_bytes, "slice src end")?;
        let dst_end = checked_add_usize(dst_start, row_bytes, "slice dst end")?;
        if src_end > chunk_data.len() || dst_end > result.len {
            return Err(Error::InvalidData(format!(
                "unit-stride slice copy out of bounds: source range {}..{} of {} bytes, destination range {}..{} of {} bytes",
                src_start,
                src_end,
                chunk_data.len(),
                dst_start,
                dst_end,
                result.len
            )));
        }
        // SAFETY: both ranges were checked immediately above, and the source
        // slice and caller-owned output allocation are distinct.
        unsafe {
            std::ptr::copy_nonoverlapping(
                chunk_data.as_ptr().add(src_start),
                result.ptr.add(dst_start),
                row_bytes,
            );
        }

        let mut carry = true;
        for d in (0..outer_idx.len()).rev() {
            if carry {
                outer_idx[d] += 1;
                if outer_idx[d] < outer_counts[d] {
                    carry = false;
                } else {
                    outer_idx[d] = 0;
                }
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
/// Copy selected elements from a chunk into the result buffer.
///
/// `dim_indices[d]` is a list of `(chunk_local_idx, result_dim_idx)` pairs for dimension `d`.
#[inline(always)]
fn copy_selected_elements(
    chunk_data: &[u8],
    result_buf: &mut [u8],
    dim_indices: &[Vec<(usize, usize)>],
    chunk_strides: &[usize],
    result_strides: &[usize],
    elem_size: usize,
    ndim: usize,
) -> Result<()> {
    if dim_indices.len() != ndim || chunk_strides.len() != ndim || result_strides.len() != ndim {
        return Err(Error::InvalidData(format!(
            "selected-element copy layout rank does not match rank {ndim}"
        )));
    }

    // Check for empty selection
    if dim_indices.iter().any(|v| v.is_empty()) {
        return Ok(());
    }

    // Recursive cartesian-product iteration, but unrolled iteratively.
    let counts: Vec<usize> = dim_indices.iter().map(|v| v.len()).collect();
    let total = checked_product_usize(&counts, "selected-element copy count")?;
    let mut counters = vec![0usize; ndim];

    for _ in 0..total {
        let mut chunk_flat = 0;
        let mut result_flat = 0;
        for d in 0..ndim {
            let (cl, ri) = dim_indices[d][counters[d]];
            let chunk_term = checked_mul_usize(cl, chunk_strides[d], "selected chunk offset")?;
            let result_term = checked_mul_usize(ri, result_strides[d], "selected result offset")?;
            chunk_flat = checked_add_usize(chunk_flat, chunk_term, "selected chunk offset")?;
            result_flat = checked_add_usize(result_flat, result_term, "selected result offset")?;
        }

        let src_start = checked_mul_usize(chunk_flat, elem_size, "selected source byte offset")?;
        let dst_start =
            checked_mul_usize(result_flat, elem_size, "selected destination byte offset")?;
        let src_end = checked_add_usize(src_start, elem_size, "selected source byte end")?;
        let dst_end = checked_add_usize(dst_start, elem_size, "selected destination byte end")?;

        if src_end > chunk_data.len() || dst_end > result_buf.len() {
            return Err(Error::InvalidData(format!(
                "selected-element copy out of bounds: source range {}..{} of {} bytes, destination range {}..{} of {} bytes",
                src_start,
                src_end,
                chunk_data.len(),
                dst_start,
                dst_end,
                result_buf.len()
            )));
        }
        result_buf[dst_start..dst_end].copy_from_slice(&chunk_data[src_start..src_end]);

        // Increment counters (row-major)
        let mut carry = true;
        for d in (0..ndim).rev() {
            if carry {
                counters[d] += 1;
                if counters[d] < dim_indices[d].len() {
                    carry = false;
                } else {
                    counters[d] = 0;
                }
            }
        }
    }

    Ok(())
}

/// Copy selected elements from a chunk into a raw output pointer.
///
/// This is the pointer-based variant of `copy_selected_elements`, suitable for
/// parallel use where multiple threads write to disjoint regions of the same buffer.
///
/// # Safety
///
/// The caller must guarantee that no two concurrent calls write to the same
/// byte range within `[result_ptr .. result_ptr + result_len)`.
#[cfg(feature = "rayon")]
#[allow(clippy::too_many_arguments)]
#[inline(always)]
unsafe fn copy_selected_elements_ptr(
    chunk_data: &[u8],
    result_ptr: *mut u8,
    result_len: usize,
    dim_indices: &[Vec<(usize, usize)>],
    chunk_strides: &[usize],
    result_strides: &[usize],
    elem_size: usize,
    ndim: usize,
) -> Result<()> {
    if dim_indices.len() != ndim || chunk_strides.len() != ndim || result_strides.len() != ndim {
        return Err(Error::InvalidData(format!(
            "selected-element copy layout rank does not match rank {ndim}"
        )));
    }

    if dim_indices.iter().any(|v| v.is_empty()) {
        return Ok(());
    }

    let counts: Vec<usize> = dim_indices.iter().map(|v| v.len()).collect();
    let total = checked_product_usize(&counts, "selected-element copy count")?;
    let mut counters = vec![0usize; ndim];

    for _ in 0..total {
        let mut chunk_flat = 0;
        let mut result_flat = 0;
        for d in 0..ndim {
            let (cl, ri) = dim_indices[d][counters[d]];
            let chunk_term = checked_mul_usize(cl, chunk_strides[d], "selected chunk offset")?;
            let result_term = checked_mul_usize(ri, result_strides[d], "selected result offset")?;
            chunk_flat = checked_add_usize(chunk_flat, chunk_term, "selected chunk offset")?;
            result_flat = checked_add_usize(result_flat, result_term, "selected result offset")?;
        }

        let src_start = checked_mul_usize(chunk_flat, elem_size, "selected source byte offset")?;
        let dst_start =
            checked_mul_usize(result_flat, elem_size, "selected destination byte offset")?;
        let src_end = checked_add_usize(src_start, elem_size, "selected source byte end")?;
        let dst_end = checked_add_usize(dst_start, elem_size, "selected destination byte end")?;

        if src_end > chunk_data.len() || dst_end > result_len {
            return Err(Error::InvalidData(format!(
                "selected-element copy out of bounds: source range {}..{} of {} bytes, destination range {}..{} of {} bytes",
                src_start,
                src_end,
                chunk_data.len(),
                dst_start,
                dst_end,
                result_len
            )));
        }
        // SAFETY: both ranges were checked immediately above, and the source
        // slice and caller-owned output allocation are distinct.
        unsafe {
            std::ptr::copy_nonoverlapping(
                chunk_data.as_ptr().add(src_start),
                result_ptr.add(dst_start),
                elem_size,
            );
        }

        let mut carry = true;
        for d in (0..ndim).rev() {
            if carry {
                counters[d] += 1;
                if counters[d] < dim_indices[d].len() {
                    carry = false;
                } else {
                    counters[d] = 0;
                }
            }
        }
    }

    Ok(())
}

/// Slice an ndarray according to a SliceInfo selection.
fn slice_array<T: H5Type + Clone>(
    array: &ArrayD<T>,
    selection: &SliceInfo,
    shape: &[u64],
) -> Result<ArrayD<T>> {
    // Build result shape
    let mut result_shape = Vec::new();

    for (i, sel) in selection.selections.iter().enumerate() {
        let dim_size = shape[i];
        match sel {
            SliceInfoElem::Index(idx) => {
                if *idx >= dim_size {
                    return Err(Error::SliceOutOfBounds {
                        dim: i,
                        index: *idx,
                        size: dim_size,
                    });
                }
                // Don't add to result_shape — this dimension is collapsed
            }
            SliceInfoElem::Slice { start, end, step } => {
                let dim_size = checked_usize(dim_size, "slice dimension size")?;
                let actual_end = if *end == u64::MAX {
                    dim_size
                } else {
                    checked_usize(*end, "slice end")?.min(dim_size)
                };
                let actual_start = checked_usize(*start, "slice start")?;
                let actual_step = checked_usize(*step, "slice step")?;
                if actual_step == 0 {
                    return Err(Error::InvalidData("slice step cannot be 0".into()));
                }
                if actual_start > dim_size {
                    return Err(Error::SliceOutOfBounds {
                        dim: i,
                        index: *start,
                        size: shape[i],
                    });
                }
                let n = (actual_end - actual_start).div_ceil(actual_step);
                result_shape.push(n);
            }
        }
    }

    // Extract elements manually (ndarray's slicing API is complex with dynamic dims)
    let ndim = shape.len();
    let total = checked_product_usize(&result_shape, "slice result element count")?;
    let mut elements = Vec::with_capacity(total);

    // Generate all indices in the result
    let mut result_idx = vec![0usize; result_shape.len()];

    for _ in 0..total {
        // Map result index to source index
        let mut src_idx = Vec::with_capacity(ndim);
        let mut ri = 0;
        for sel in selection.selections.iter() {
            match sel {
                SliceInfoElem::Index(idx) => {
                    src_idx.push(checked_usize(*idx, "slice source index")?);
                }
                SliceInfoElem::Slice { start, step, .. } => {
                    let start = checked_usize(*start, "slice start")?;
                    let step = checked_usize(*step, "slice step")?;
                    let offset =
                        checked_mul_usize(result_idx[ri], step, "slice source index offset")?;
                    src_idx.push(checked_add_usize(start, offset, "slice source index")?);
                    ri += 1;
                }
            }
        }

        elements.push(array[IxDyn(&src_idx)].clone());

        // Increment result index
        if !result_shape.is_empty() {
            let mut carry = true;
            for d in (0..result_shape.len()).rev() {
                if carry {
                    result_idx[d] += 1;
                    if result_idx[d] < result_shape[d] {
                        carry = false;
                    } else {
                        result_idx[d] = 0;
                    }
                }
            }
        }
    }

    ArrayD::from_shape_vec(IxDyn(&result_shape), elements)
        .map_err(|e| Error::InvalidData(format!("slice shape error: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::BytesStorage;

    /// Buffers sized from a corrupt dataspace fail the read instead of aborting.
    #[test]
    fn impossible_selection_buffers_are_errors() {
        assert!(try_uninit_vec::<u64>(usize::MAX / 4).is_err());
        assert!(try_zeroed_bytes(usize::MAX / 2).is_err());
        assert_eq!(try_uninit_vec::<u64>(3).unwrap().len(), 3);
        assert_eq!(try_zeroed_bytes(5).unwrap(), vec![0u8; 5]);
    }
    use crate::superblock::Superblock;
    use std::collections::HashMap;

    fn test_context_with_offset_size(bytes: Vec<u8>, offset_size: u8) -> Arc<FileContext> {
        let storage: DynStorage = Arc::new(BytesStorage::new(bytes));
        Arc::new(FileContext {
            storage,
            superblock: Superblock {
                version: 2,
                offset_size,
                length_size: 8,
                group_leaf_node_k: 0,
                group_internal_node_k: 0,
                indexed_storage_k: 0,
                consistency_flags: 0,
                base_address: 0,
                free_space_address: u64::MAX,
                eof_address: 0,
                driver_info_address: u64::MAX,
                root_symbol_table_entry: None,
                root_object_header_address: Some(0),
                extension_address: None,
            },
            chunk_cache: Arc::new(ChunkCache::new(1024, 8)),
            header_cache: Arc::new(Mutex::new(HashMap::new())),
            dataset_path_cache: Arc::new(Mutex::new(HashMap::new())),
            filter_registry: Arc::new(FilterRegistry::default()),
            external_file_resolver: None,
            external_link_resolver: None,
            external_file_cache: Mutex::new(HashMap::new()),
            sohm_table: OnceLock::new(),
            full_file_cache: OnceLock::new(),
        })
    }

    fn test_context(bytes: Vec<u8>) -> Arc<FileContext> {
        test_context_with_offset_size(bytes, 8)
    }

    fn fixed_u16_dataset(layout: DataLayout, storage_bytes: Vec<u8>) -> Dataset {
        let context = test_context(storage_bytes);
        Dataset {
            context: context.clone(),
            name: "short".to_string(),
            data_address: 0,
            dataspace: DataspaceMessage {
                rank: 1,
                dims: vec![3],
                max_dims: None,
                dataspace_type: DataspaceType::Simple,
            },
            datatype: Datatype::FixedPoint {
                size: 2,
                signed: false,
                byte_order: ByteOrder::LittleEndian,
            },
            layout,
            fill_value: None,
            filters: None,
            external_files: None,
            attributes: Vec::new(),
            chunk_cache: context.chunk_cache.clone(),
            chunk_entry_cache: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(32).unwrap()))),
            full_chunk_entries: Arc::new(OnceLock::new()),
            full_dataset_bytes: Arc::new(OnceLock::new()),
            external_slots: Arc::new(OnceLock::new()),
            filter_registry: context.filter_registry.clone(),
        }
    }

    fn vlen_string_dataset(
        layout: DataLayout,
        storage_bytes: Vec<u8>,
        offset_size: u8,
        fill_value: Option<FillValueMessage>,
    ) -> Dataset {
        let context = test_context_with_offset_size(storage_bytes, offset_size);
        Dataset {
            context: context.clone(),
            name: "vlen".to_string(),
            data_address: 0,
            dataspace: DataspaceMessage {
                rank: 1,
                dims: vec![2],
                max_dims: None,
                dataspace_type: DataspaceType::Simple,
            },
            datatype: Datatype::String {
                size: StringSize::Variable,
                encoding: crate::messages::datatype::StringEncoding::Ascii,
                padding: crate::messages::datatype::StringPadding::NullTerminate,
            },
            layout,
            fill_value,
            filters: None,
            external_files: None,
            attributes: Vec::new(),
            chunk_cache: context.chunk_cache.clone(),
            chunk_entry_cache: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(32).unwrap()))),
            full_chunk_entries: Arc::new(OnceLock::new()),
            full_dataset_bytes: Arc::new(OnceLock::new()),
            external_slots: Arc::new(OnceLock::new()),
            filter_registry: context.filter_registry.clone(),
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct RawElement(Vec<u8>);

    impl H5Type for RawElement {
        fn hdf5_type() -> Datatype {
            Datatype::Opaque {
                size: 0,
                tag: "raw".to_string(),
            }
        }

        fn from_bytes(bytes: &[u8], _dtype: &Datatype) -> Result<Self> {
            Ok(Self(bytes.to_vec()))
        }

        fn element_size(dtype: &Datatype) -> usize {
            crate::datatype_api::dtype_element_size(dtype).unwrap_or(0)
        }
    }

    #[test]
    fn slice_info_all() {
        let s = SliceInfo::all(3);
        assert_eq!(s.selections.len(), 3);
    }

    #[test]
    fn raw_element_size_uses_file_vlen_reference_width() {
        let dtype = Datatype::VarLen {
            base: Box::new(Datatype::FixedPoint {
                size: 1,
                signed: false,
                byte_order: crate::error::ByteOrder::LittleEndian,
            }),
            kind: VarLenKind::Sequence,
            encoding: crate::messages::datatype::StringEncoding::Ascii,
            padding: crate::messages::datatype::StringPadding::NullTerminate,
        };

        assert_eq!(raw_element_size_for_datatype(&dtype, 12).unwrap(), 12);
        assert_eq!(
            raw_element_size_for_datatype(
                &Datatype::Array {
                    base: Box::new(dtype),
                    dims: vec![2, 3],
                },
                12,
            )
            .unwrap(),
            72
        );
    }

    #[test]
    fn chunked_vlen_slice_uses_file_reference_width() {
        for (offset_size, ref_size) in [(2u8, 10usize), (4u8, 12usize)] {
            let chunk_bytes: Vec<u8> = (0..(2 * ref_size)).map(|i| (i + 1) as u8).collect();
            let dataset = vlen_string_dataset(
                DataLayout::Chunked {
                    address: 0,
                    dims: vec![2],
                    element_size: 16,
                    chunk_indexing: Some(ChunkIndexing::Implicit),
                },
                chunk_bytes.clone(),
                offset_size,
                None,
            );

            let selection = SliceInfo {
                selections: vec![SliceInfoElem::Slice {
                    start: 0,
                    end: 2,
                    step: 1,
                }],
            };
            let array = dataset.read_slice::<RawElement>(&selection).unwrap();
            let values: Vec<_> = array.iter().cloned().collect();

            assert_eq!(values[0].0, chunk_bytes[..ref_size]);
            assert_eq!(values[1].0, chunk_bytes[ref_size..2 * ref_size]);
        }
    }

    #[cfg(feature = "rayon")]
    #[test]
    fn parallel_chunked_vlen_slice_uses_file_reference_width() {
        for (offset_size, ref_size) in [(2u8, 10usize), (4u8, 12usize)] {
            let chunk_bytes: Vec<u8> = (0..(2 * ref_size)).map(|i| (i + 1) as u8).collect();
            let dataset = vlen_string_dataset(
                DataLayout::Chunked {
                    address: 0,
                    dims: vec![2],
                    element_size: 16,
                    chunk_indexing: Some(ChunkIndexing::Implicit),
                },
                chunk_bytes.clone(),
                offset_size,
                None,
            );

            let selection = SliceInfo {
                selections: vec![SliceInfoElem::Slice {
                    start: 0,
                    end: 2,
                    step: 1,
                }],
            };
            let array = dataset
                .read_slice_parallel::<RawElement>(&selection)
                .unwrap();
            let values: Vec<_> = array.iter().cloned().collect();

            assert_eq!(values[0].0, chunk_bytes[..ref_size]);
            assert_eq!(values[1].0, chunk_bytes[ref_size..2 * ref_size]);
        }
    }

    #[test]
    fn undefined_chunked_vlen_slice_fill_uses_file_reference_width() {
        for (offset_size, ref_size, undefined_address) in
            [(2u8, 10usize, 0xFFFF), (4u8, 12usize, 0xFFFF_FFFF)]
        {
            let fill_bytes = vec![0xA5; ref_size];
            let dataset = vlen_string_dataset(
                DataLayout::Chunked {
                    address: undefined_address,
                    dims: vec![2],
                    element_size: 16,
                    chunk_indexing: Some(ChunkIndexing::Implicit),
                },
                Vec::new(),
                offset_size,
                Some(FillValueMessage {
                    defined: true,
                    fill_time: FillTime::IfSet,
                    value: Some(fill_bytes.clone()),
                }),
            );

            let selection = SliceInfo {
                selections: vec![SliceInfoElem::Slice {
                    start: 0,
                    end: 2,
                    step: 1,
                }],
            };
            let array = dataset.read_slice::<RawElement>(&selection).unwrap();
            let values: Vec<_> = array.iter().cloned().collect();

            assert_eq!(values[0].0, fill_bytes);
            assert_eq!(values[1].0, fill_bytes);
        }
    }

    #[test]
    fn array_raw_element_size_rejects_overflow() {
        let dtype = Datatype::Array {
            base: Box::new(Datatype::FixedPoint {
                size: 8,
                signed: false,
                byte_order: crate::error::ByteOrder::LittleEndian,
            }),
            dims: vec![u64::MAX, 2],
        };

        let err = raw_element_size_for_datatype(&dtype, 12).unwrap_err();
        assert!(err.to_string().contains("array datatype"));
    }

    #[test]
    fn dataset_num_elements_rejects_overflow() {
        let mut dataset = fixed_u16_dataset(DataLayout::Compact { data: vec![] }, Vec::new());
        dataset.dataspace.dims = vec![u64::MAX, 2];

        let err = dataset.num_elements().unwrap_err();
        assert!(err.to_string().contains("element count"));
    }

    #[test]
    fn compact_raw_data_requires_exact_logical_length() {
        let dataset = fixed_u16_dataset(
            DataLayout::Compact {
                data: vec![1, 0, 2, 0, 3],
            },
            Vec::new(),
        );

        let err = dataset.read_array::<u16>().unwrap_err();
        assert!(
            matches!(err, Error::Context { .. })
                && err
                    .to_string()
                    .contains("compact raw data has 5 bytes, expected 6 bytes"),
            "expected compact raw length error, got: {err}"
        );
    }

    #[test]
    fn contiguous_raw_data_requires_exact_logical_length() {
        let dataset = fixed_u16_dataset(
            DataLayout::Contiguous {
                address: 0,
                size: 5,
            },
            vec![1, 0, 2, 0, 3],
        );

        let err = dataset.read_raw_bytes().unwrap_err();
        assert!(
            matches!(err, Error::Context { .. })
                && err
                    .to_string()
                    .contains("contiguous raw data has 5 bytes, expected 6 bytes"),
            "expected contiguous raw length error, got: {err}"
        );
    }

    #[test]
    fn copy_chunk_1d() {
        let chunk_data = vec![1u8, 2, 3, 4]; // 4 elements of 1 byte each
        let mut flat = vec![0u8; 8];
        let chunk_offsets = vec![2u64]; // starts at index 2
        let chunk_shape = vec![4u64];
        let dataset_shape = vec![8u64];

        copy_chunk_to_flat(
            &chunk_data,
            &mut flat,
            &chunk_offsets,
            &chunk_shape,
            &dataset_shape,
            1,
        )
        .unwrap();
        assert_eq!(flat, vec![0, 0, 1, 2, 3, 4, 0, 0]);
    }

    #[test]
    fn copy_chunk_2d_rowwise() {
        let chunk_data = vec![1u8, 2, 3, 4, 5, 6];
        let mut flat = vec![0u8; 16];
        let chunk_offsets = vec![1u64, 1u64];
        let chunk_shape = vec![2u64, 3u64];
        let dataset_shape = vec![4u64, 4u64];

        copy_chunk_to_flat(
            &chunk_data,
            &mut flat,
            &chunk_offsets,
            &chunk_shape,
            &dataset_shape,
            1,
        )
        .unwrap();

        assert_eq!(flat, vec![0, 0, 0, 0, 0, 1, 2, 3, 0, 4, 5, 6, 0, 0, 0, 0,]);
    }

    #[test]
    fn copy_unit_stride_chunk_overlap_2d_partial() {
        let chunk_data: Vec<u8> = (1..=16).collect();
        let mut result = vec![0u8; 6];
        let chunk_offsets = vec![0u64, 0u64];
        let chunk_shape = vec![4u64, 4u64];
        let dataset_shape = vec![4u64, 4u64];
        let resolved = ResolvedSelection {
            dims: vec![
                ResolvedSelectionDim {
                    start: 1,
                    end: 3,
                    step: 1,
                    count: 2,
                },
                ResolvedSelectionDim {
                    start: 1,
                    end: 4,
                    step: 1,
                    count: 3,
                },
            ],
            result_shape: vec![2, 3],
            result_elements: 6,
        };
        let chunk_strides = vec![4usize, 1usize];
        let result_strides = vec![3usize, 1usize];

        copy_unit_stride_chunk_overlap(
            &chunk_data,
            &mut result,
            UnitStrideCopyLayout {
                chunk_offsets: &chunk_offsets,
                chunk_shape: &chunk_shape,
                dataset_shape: &dataset_shape,
                resolved: &resolved,
                chunk_strides: &chunk_strides,
                result_strides: &result_strides,
                elem_size: 1,
            },
        )
        .unwrap();

        assert_eq!(result, vec![6, 7, 8, 10, 11, 12]);
    }

    fn chunk_entry(offsets: &[u64], address: u64) -> chunk_index::ChunkEntry {
        chunk_index::ChunkEntry {
            address,
            size: 0,
            filter_mask: 0,
            offsets: offsets.to_vec(),
        }
    }

    #[test]
    fn chunk_grid_coverage_detects_missing_chunk() {
        let mut entries = vec![
            chunk_entry(&[0, 0], 0x1000),
            chunk_entry(&[0, 2], 0x2000),
            chunk_entry(&[2, 0], 0x3000),
        ];

        let complete =
            validate_chunk_grid_coverage(&mut entries, &[4, 4], &[2, 2], &[0, 0], &[1, 1]).unwrap();

        assert!(!complete);
    }

    #[test]
    fn chunk_grid_coverage_rejects_duplicate_offsets() {
        let mut entries = vec![
            chunk_entry(&[0, 0], 0x1000),
            chunk_entry(&[0, 0], 0x2000),
            chunk_entry(&[0, 2], 0x3000),
            chunk_entry(&[2, 0], 0x4000),
        ];

        let err = validate_chunk_grid_coverage(&mut entries, &[4, 4], &[2, 2], &[0, 0], &[1, 1])
            .unwrap_err();

        assert!(matches!(err, Error::InvalidData(_)));
    }

    #[test]
    fn decoded_chunk_len_requires_exact_size() {
        let entry = chunk_entry(&[0, 0], 0x1000);

        validate_decoded_chunk_len(&entry, &[2, 3], 4, 24).unwrap();
        let err = validate_decoded_chunk_len(&entry, &[2, 3], 4, 23).unwrap_err();

        assert!(matches!(err, Error::InvalidData(_)));
    }

    #[test]
    fn copy_chunk_errors_on_short_row() {
        let chunk_data = vec![1u8, 2, 3, 4, 5];
        let mut flat = vec![0u8; 16];
        let chunk_offsets = vec![1u64, 1u64];
        let chunk_shape = vec![2u64, 3u64];
        let dataset_shape = vec![4u64, 4u64];

        let err = copy_chunk_to_flat(
            &chunk_data,
            &mut flat,
            &chunk_offsets,
            &chunk_shape,
            &dataset_shape,
            1,
        )
        .unwrap_err();

        assert!(matches!(err, Error::InvalidData(_)));
    }

    #[test]
    fn copy_unit_stride_chunk_overlap_errors_on_short_row() {
        let chunk_data: Vec<u8> = (1..=7).collect();
        let mut result = vec![0u8; 6];
        let chunk_offsets = vec![0u64, 0u64];
        let chunk_shape = vec![4u64, 4u64];
        let dataset_shape = vec![4u64, 4u64];
        let resolved = ResolvedSelection {
            dims: vec![
                ResolvedSelectionDim {
                    start: 1,
                    end: 3,
                    step: 1,
                    count: 2,
                },
                ResolvedSelectionDim {
                    start: 1,
                    end: 4,
                    step: 1,
                    count: 3,
                },
            ],
            result_shape: vec![2, 3],
            result_elements: 6,
        };
        let chunk_strides = vec![4usize, 1usize];
        let result_strides = vec![3usize, 1usize];

        let err = copy_unit_stride_chunk_overlap(
            &chunk_data,
            &mut result,
            UnitStrideCopyLayout {
                chunk_offsets: &chunk_offsets,
                chunk_shape: &chunk_shape,
                dataset_shape: &dataset_shape,
                resolved: &resolved,
                chunk_strides: &chunk_strides,
                result_strides: &result_strides,
                elem_size: 1,
            },
        )
        .unwrap_err();

        assert!(matches!(err, Error::InvalidData(_)));
    }

    // ------------------------------------------------------------------
    // validate_chunk_grid_coverage: the gate for the uninitialized-output
    // fast path. It must return Ok(true) only for an exact, gap-free,
    // duplicate-free tiling; otherwise the caller falls back to a
    // zero-initialized buffer, so returning false (not true) on a missing
    // chunk is what keeps the assume_init path sound.
    // ------------------------------------------------------------------

    #[test]
    fn grid_coverage_true_for_exact_tiling() {
        // 4x6 dataset, 2x3 chunks -> a full 2x2 chunk grid.
        let shape = [4u64, 6];
        let chunk_shape = [2u64, 3];
        let mut entries = vec![
            chunk_entry(&[0, 0], 0x100),
            chunk_entry(&[0, 3], 0x200),
            chunk_entry(&[2, 0], 0x300),
            chunk_entry(&[2, 3], 0x400),
        ];
        let covered =
            validate_chunk_grid_coverage(&mut entries, &shape, &chunk_shape, &[0, 0], &[1, 1])
                .unwrap();
        assert!(covered);
    }

    #[test]
    fn grid_coverage_false_when_a_chunk_is_missing() {
        // Drop the last chunk: coverage is incomplete, so the fast path must
        // be declined (Ok(false)), never Ok(true).
        let shape = [4u64, 6];
        let chunk_shape = [2u64, 3];
        let mut entries = vec![
            chunk_entry(&[0, 0], 0x100),
            chunk_entry(&[0, 3], 0x200),
            chunk_entry(&[2, 0], 0x300),
        ];
        let covered =
            validate_chunk_grid_coverage(&mut entries, &shape, &chunk_shape, &[0, 0], &[1, 1])
                .unwrap();
        assert!(!covered);
    }

    #[test]
    fn grid_coverage_rejects_duplicate_chunks() {
        let shape = [4u64, 6];
        let chunk_shape = [2u64, 3];
        let mut entries = vec![
            chunk_entry(&[0, 0], 0x100),
            chunk_entry(&[0, 0], 0x999),
            chunk_entry(&[0, 3], 0x200),
            chunk_entry(&[2, 0], 0x300),
            chunk_entry(&[2, 3], 0x400),
        ];
        let err =
            validate_chunk_grid_coverage(&mut entries, &shape, &chunk_shape, &[0, 0], &[1, 1])
                .unwrap_err();
        assert!(matches!(err, Error::InvalidData(msg) if msg.contains("duplicate chunk")));
    }

    #[test]
    fn grid_coverage_rejects_out_of_grid_offsets() {
        // An offset not aligned to the chunk grid must be rejected outright.
        let shape = [4u64, 6];
        let chunk_shape = [2u64, 3];
        let mut entries = vec![
            chunk_entry(&[0, 0], 0x100),
            chunk_entry(&[1, 3], 0x200), // row offset 1 is not a multiple of 2
            chunk_entry(&[2, 0], 0x300),
            chunk_entry(&[2, 3], 0x400),
        ];
        let err =
            validate_chunk_grid_coverage(&mut entries, &shape, &chunk_shape, &[0, 0], &[1, 1])
                .unwrap_err();
        assert!(matches!(err, Error::InvalidData(_)));
    }
}
