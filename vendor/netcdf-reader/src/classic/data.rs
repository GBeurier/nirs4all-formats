//! Data reading for classic (CDF-1/2/5) NetCDF files.
//!
//! Two layout types:
//! - **Non-record variables**: contiguous data at the offset stored in the variable header.
//! - **Record variables**: data is interleaved across records. Each record contains one
//!   slice from every record variable, in the order they appear in the header. The total
//!   record size is the sum of all record variables' vsize values (each padded to 4-byte
//!   boundary in CDF-1/2).

use ndarray::{ArrayD, IxDyn};
#[cfg(feature = "rayon")]
use rayon::prelude::*;

use crate::error::{Error, Result};
use crate::types::{NcType, NcVariable};

use super::storage::ClassicStorage;

/// Trait for types that can be read from classic NetCDF data.
pub trait NcReadType: Clone + Default + Send + 'static {
    /// The NetCDF type this Rust type corresponds to.
    fn nc_type() -> NcType;

    /// Read a single element from big-endian bytes.
    fn from_be_bytes(bytes: &[u8]) -> Result<Self>;

    /// Size in bytes of one element.
    fn element_size() -> usize;

    /// Bulk decode `count` elements from a contiguous big-endian byte slice.
    ///
    /// Default implementation falls back to per-element decoding. Types with
    /// multi-byte elements override this with an optimized bulk path using
    /// `chunks_exact` + byte-swap (on LE hosts) or `copy_nonoverlapping`
    /// (on BE hosts).
    fn decode_bulk_be(raw: &[u8], count: usize) -> Result<Vec<Self>> {
        let elem_size = Self::element_size();
        let needed = count.checked_mul(elem_size).ok_or_else(|| {
            Error::InvalidData("classic decode byte count exceeds platform usize".to_string())
        })?;
        if raw.len() < needed {
            return Err(Error::InvalidData(format!(
                "need {} bytes for {} elements, got {}",
                needed,
                count,
                raw.len()
            )));
        }
        let mut values = Vec::with_capacity(count);
        for i in 0..count {
            let start = i * elem_size;
            values.push(Self::from_be_bytes(&raw[start..start + elem_size])?);
        }
        Ok(values)
    }

    /// Bulk decode elements from a contiguous big-endian byte slice into a
    /// caller-provided destination buffer.
    fn decode_bulk_be_into(raw: &[u8], dst: &mut [Self]) -> Result<()> {
        let elem_size = Self::element_size();
        let needed = dst.len().checked_mul(elem_size).ok_or_else(|| {
            Error::InvalidData("classic decode byte count exceeds platform usize".to_string())
        })?;
        if raw.len() < needed {
            return Err(Error::InvalidData(format!(
                "need {} bytes for {} elements, got {}",
                needed,
                dst.len(),
                raw.len()
            )));
        }
        for (out, chunk) in dst.iter_mut().zip(raw[..needed].chunks_exact(elem_size)) {
            *out = Self::from_be_bytes(chunk)?;
        }
        Ok(())
    }
}

macro_rules! impl_nc_read_type {
    ($ty:ty, $nc_type:path, $size:literal) => {
        impl NcReadType for $ty {
            fn nc_type() -> NcType {
                $nc_type
            }

            fn from_be_bytes(bytes: &[u8]) -> Result<Self> {
                if bytes.len() < $size {
                    return Err(Error::InvalidData(format!(
                        "need {} bytes for {}, got {}",
                        $size,
                        stringify!($ty),
                        bytes.len()
                    )));
                }
                let mut arr = [0u8; $size];
                arr.copy_from_slice(&bytes[..$size]);
                Ok(<$ty>::from_be_bytes(arr))
            }

            fn element_size() -> usize {
                $size
            }

            fn decode_bulk_be(raw: &[u8], count: usize) -> Result<Vec<Self>> {
                let total_bytes = count.checked_mul($size).ok_or_else(|| {
                    Error::InvalidData(
                        "classic decode byte count exceeds platform usize".to_string(),
                    )
                })?;
                if raw.len() < total_bytes {
                    return Err(Error::InvalidData(format!(
                        "need {} bytes for {} elements of {}, got {}",
                        total_bytes,
                        count,
                        stringify!($ty),
                        raw.len()
                    )));
                }
                let bytes = &raw[..total_bytes];
                #[cfg(target_endian = "big")]
                {
                    let mut values = Vec::<$ty>::with_capacity(count);
                    // SAFETY: `values` has capacity for `count` elements,
                    // `raw` contains exactly `count * size_of::<$ty>()` bytes,
                    // and numeric types admit every bit pattern.
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            bytes.as_ptr(),
                            values.as_mut_ptr() as *mut u8,
                            total_bytes,
                        );
                        values.set_len(count);
                    }
                    Ok(values)
                }
                #[cfg(target_endian = "little")]
                {
                    // LE host reading BE data: chunks_exact + byte-swap.
                    Ok(bytes
                        .chunks_exact($size)
                        .map(|chunk| {
                            let mut arr = [0u8; $size];
                            arr.copy_from_slice(chunk);
                            <$ty>::from_be_bytes(arr)
                        })
                        .collect())
                }
            }

            fn decode_bulk_be_into(raw: &[u8], dst: &mut [Self]) -> Result<()> {
                let total_bytes = dst.len().checked_mul($size).ok_or_else(|| {
                    Error::InvalidData(
                        "classic decode byte count exceeds platform usize".to_string(),
                    )
                })?;
                if raw.len() < total_bytes {
                    return Err(Error::InvalidData(format!(
                        "need {} bytes for {} elements of {}, got {}",
                        total_bytes,
                        dst.len(),
                        stringify!($ty),
                        raw.len()
                    )));
                }
                let bytes = &raw[..total_bytes];
                #[cfg(target_endian = "big")]
                {
                    // SAFETY: `values` has capacity for `count` bytes and
                    // `bytes` contains exactly `count` initialized bytes.
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            bytes.as_ptr(),
                            dst.as_mut_ptr() as *mut u8,
                            total_bytes,
                        );
                    }
                    Ok(())
                }
                #[cfg(target_endian = "little")]
                {
                    for (out, chunk) in dst.iter_mut().zip(bytes.chunks_exact($size)) {
                        let mut arr = [0u8; $size];
                        arr.copy_from_slice(chunk);
                        *out = <$ty>::from_be_bytes(arr);
                    }
                    Ok(())
                }
            }
        }
    };
}

impl_nc_read_type!(i8, NcType::Byte, 1);
impl_nc_read_type!(i16, NcType::Short, 2);
impl_nc_read_type!(i32, NcType::Int, 4);
impl_nc_read_type!(f32, NcType::Float, 4);
impl_nc_read_type!(f64, NcType::Double, 8);
impl_nc_read_type!(u8, NcType::UByte, 1);
impl_nc_read_type!(u16, NcType::UShort, 2);
impl_nc_read_type!(u32, NcType::UInt, 4);
impl_nc_read_type!(i64, NcType::Int64, 8);
impl_nc_read_type!(u64, NcType::UInt64, 8);

/// Read the entire data for a non-record variable into an ndarray.
///
/// The data is located at a contiguous region starting at `var.data_offset`
/// with total size `var.data_size`.
pub fn read_non_record_variable<T: NcReadType>(
    file_data: &[u8],
    var: &NcVariable,
) -> Result<ArrayD<T>> {
    if var.is_record_var {
        return Err(Error::InvalidData(
            "use read_record_variable for record variables".to_string(),
        ));
    }

    let offset = crate::types::checked_usize_from_u64(var.data_offset, "variable data offset")?;
    let total_elements = checked_non_record_element_count(var)?;
    let elem_size = T::element_size();
    let total_bytes = total_elements.checked_mul(elem_size).ok_or_else(|| {
        Error::InvalidData(format!(
            "variable '{}' size in bytes exceeds platform usize",
            var.name
        ))
    })?;

    let end = offset.checked_add(total_bytes).ok_or_else(|| {
        Error::InvalidData(format!(
            "variable '{}' byte range exceeds platform usize",
            var.name
        ))
    })?;
    if end > file_data.len() {
        return Err(Error::InvalidData(format!(
            "variable '{}' data extends beyond file: offset={}, size={}, file_len={}",
            var.name,
            offset,
            total_bytes,
            file_data.len()
        )));
    }

    let data_slice = &file_data[offset..end];
    let values = T::decode_bulk_be(data_slice, total_elements)?;

    let shape: Vec<usize> = var
        .shape()
        .iter()
        .map(|&s| crate::types::checked_usize_from_u64(s, "variable dimension"))
        .collect::<Result<Vec<_>>>()?;
    if shape.is_empty() {
        // Scalar variable.
        ArrayD::from_shape_vec(IxDyn(&[]), values)
    } else {
        ArrayD::from_shape_vec(IxDyn(&shape), values)
    }
    .map_err(|e| Error::InvalidData(format!("failed to create array: {}", e)))
}

/// Read the entire data for a non-record variable from range-backed storage.
pub(crate) fn read_non_record_variable_from_storage<T: NcReadType>(
    storage: &ClassicStorage,
    var: &NcVariable,
) -> Result<ArrayD<T>> {
    if var.is_record_var {
        return Err(Error::InvalidData(
            "use read_record_variable_from_storage for record variables".to_string(),
        ));
    }

    let total_elements = checked_non_record_element_count(var)?;
    let total_bytes = variable_data_bytes::<T>(var.name.as_str(), total_elements)?;
    let data = storage.read_range(var.data_offset, total_bytes)?;
    let values = T::decode_bulk_be(data.as_ref(), total_elements)?;
    let shape = checked_variable_shape(var)?;

    ArrayD::from_shape_vec(IxDyn(&shape), values)
        .map_err(|e| Error::InvalidData(format!("failed to create array: {}", e)))
}

/// Read the entire data for a non-record variable using Rayon for large reads.
#[cfg(feature = "rayon")]
pub(crate) fn read_non_record_variable_parallel_from_storage<T: NcReadType>(
    storage: &ClassicStorage,
    var: &NcVariable,
) -> Result<ArrayD<T>> {
    if var.is_record_var {
        return Err(Error::InvalidData(
            "use read_record_variable_parallel_from_storage for record variables".to_string(),
        ));
    }

    let total_elements = checked_non_record_element_count(var)?;
    let total_bytes = variable_data_bytes::<T>(var.name.as_str(), total_elements)?;
    let policy = storage.parallel_read_policy();
    if total_bytes < policy.min_bytes || total_elements == 0 {
        return read_non_record_variable_from_storage(storage, var);
    }

    // The parallel path allocates the whole result up front, so validate the
    // declared byte range fits in storage before dispatching; the serial path
    // already bounds-checks per read.
    let end = var
        .data_offset
        .checked_add(total_bytes as u64)
        .ok_or_else(|| {
            Error::InvalidData(format!("variable '{}' byte range exceeds u64", var.name))
        })?;
    if end > storage.len() {
        return Err(Error::InvalidData(format!(
            "variable '{}' data extends beyond file: needs {end} bytes, have {}",
            var.name,
            storage.len()
        )));
    }

    let values = read_contiguous_range_parallel::<T>(
        storage,
        var.data_offset,
        total_elements,
        policy.target_chunk_bytes,
    )?;
    let shape = checked_variable_shape(var)?;

    ArrayD::from_shape_vec(IxDyn(&shape), values)
        .map_err(|e| Error::InvalidData(format!("failed to create array: {}", e)))
}

/// Read the entire data for a non-record variable into a caller-provided buffer.
pub fn read_non_record_variable_into<T: NcReadType>(
    file_data: &[u8],
    var: &NcVariable,
    dst: &mut [T],
) -> Result<()> {
    if var.is_record_var {
        return Err(Error::InvalidData(
            "use read_record_variable_into for record variables".to_string(),
        ));
    }

    let total_elements = checked_non_record_element_count(var)?;
    if dst.len() != total_elements {
        return Err(Error::InvalidData(format!(
            "destination has {} elements, variable '{}' requires {}",
            dst.len(),
            var.name,
            total_elements
        )));
    }

    let offset = crate::types::checked_usize_from_u64(var.data_offset, "variable data offset")?;
    let elem_size = T::element_size();
    let total_bytes = total_elements.checked_mul(elem_size).ok_or_else(|| {
        Error::InvalidData(format!(
            "variable '{}' size in bytes exceeds platform usize",
            var.name
        ))
    })?;

    let end = offset.checked_add(total_bytes).ok_or_else(|| {
        Error::InvalidData(format!(
            "variable '{}' byte range exceeds platform usize",
            var.name
        ))
    })?;
    if end > file_data.len() {
        return Err(Error::InvalidData(format!(
            "variable '{}' data extends beyond file: offset={}, size={}, file_len={}",
            var.name,
            offset,
            total_bytes,
            file_data.len()
        )));
    }

    T::decode_bulk_be_into(&file_data[offset..end], dst)
}

/// Read a non-record variable from range-backed storage into a caller-provided buffer.
pub(crate) fn read_non_record_variable_into_from_storage<T: NcReadType>(
    storage: &ClassicStorage,
    var: &NcVariable,
    dst: &mut [T],
) -> Result<()> {
    if var.is_record_var {
        return Err(Error::InvalidData(
            "use read_record_variable_into_from_storage for record variables".to_string(),
        ));
    }

    let total_elements = checked_non_record_element_count(var)?;
    if dst.len() != total_elements {
        return Err(Error::InvalidData(format!(
            "destination has {} elements, variable '{}' requires {}",
            dst.len(),
            var.name,
            total_elements
        )));
    }

    let total_bytes = variable_data_bytes::<T>(var.name.as_str(), total_elements)?;
    let data = storage.read_range(var.data_offset, total_bytes)?;
    T::decode_bulk_be_into(data.as_ref(), dst)
}

/// Read the entire data for a record variable into an ndarray.
///
/// Record variables are interleaved: for each of `numrecs` records, every record
/// variable contributes `record_size` bytes (padded to 4-byte alignment for CDF-1/2).
/// The `record_stride` is the total size of one record across all record variables.
///
/// Parameters:
/// - `file_data`: the raw file bytes
/// - `var`: the record variable to read
/// - `numrecs`: number of records (from the file header)
/// - `record_stride`: total bytes per record (sum of all record variables' padded vsizes)
pub fn read_record_variable<T: NcReadType>(
    file_data: &[u8],
    var: &NcVariable,
    numrecs: u64,
    record_stride: u64,
) -> Result<ArrayD<T>> {
    if !var.is_record_var {
        return Err(Error::InvalidData(
            "use read_non_record_variable for non-record variables".to_string(),
        ));
    }

    let elem_size = T::element_size();
    let base_offset =
        crate::types::checked_usize_from_u64(var.data_offset, "record variable data offset")?;
    let record_stride_usize = crate::types::checked_usize_from_u64(record_stride, "record stride")?;

    // Shape: the first dimension is the unlimited dimension, replaced by numrecs.
    let shape = checked_record_shape(var, numrecs)?;
    let numrecs_usize = shape[0];

    // Number of elements per record (product of all dims except the first).
    let elements_per_record = checked_record_elements_per_record(var)?;
    let bytes_per_record = elements_per_record.checked_mul(elem_size).ok_or_else(|| {
        Error::InvalidData(format!(
            "record variable '{}' bytes per record exceed platform usize",
            var.name
        ))
    })?;
    let total_elements = numrecs_usize
        .checked_mul(elements_per_record)
        .ok_or_else(|| {
            Error::InvalidData(format!(
                "record variable '{}' element count exceeds platform usize",
                var.name
            ))
        })?;

    // Validate that every record fits in the file before allocating, so a
    // crafted `numrecs` cannot force a huge up-front allocation.
    validate_record_span(
        var,
        numrecs,
        record_stride,
        bytes_per_record as u64,
        file_data.len() as u64,
    )?;

    let mut values = Vec::with_capacity(total_elements);

    for rec in 0..numrecs_usize {
        let rec_offset = base_offset
            .checked_add(rec.checked_mul(record_stride_usize).ok_or_else(|| {
                Error::InvalidData(format!(
                    "record variable '{}' byte offset exceeds platform usize",
                    var.name
                ))
            })?)
            .ok_or_else(|| {
                Error::InvalidData(format!(
                    "record variable '{}' byte offset exceeds platform usize",
                    var.name
                ))
            })?;
        let rec_end = rec_offset.checked_add(bytes_per_record).ok_or_else(|| {
            Error::InvalidData(format!(
                "record variable '{}' record range exceeds platform usize",
                var.name
            ))
        })?;
        if rec_end > file_data.len() {
            return Err(Error::InvalidData(format!(
                "record {} for variable '{}' extends beyond file",
                rec, var.name
            )));
        }
        let rec_slice = &file_data[rec_offset..rec_end];
        let rec_values = T::decode_bulk_be(rec_slice, elements_per_record)?;
        values.extend(rec_values);
    }

    ArrayD::from_shape_vec(IxDyn(&shape), values)
        .map_err(|e| Error::InvalidData(format!("failed to create array: {}", e)))
}

/// Read the entire data for a record variable from range-backed storage.
pub(crate) fn read_record_variable_from_storage<T: NcReadType>(
    storage: &ClassicStorage,
    var: &NcVariable,
    numrecs: u64,
    record_stride: u64,
) -> Result<ArrayD<T>> {
    if !var.is_record_var {
        return Err(Error::InvalidData(
            "use read_non_record_variable_from_storage for non-record variables".to_string(),
        ));
    }

    let shape = checked_record_shape(var, numrecs)?;
    let elements_per_record = checked_record_elements_per_record(var)?;
    let bytes_per_record = variable_data_bytes::<T>(var.name.as_str(), elements_per_record)?;
    let numrecs_usize = crate::types::checked_usize_from_u64(numrecs, "record count")?;
    let total_elements = numrecs_usize
        .checked_mul(elements_per_record)
        .ok_or_else(|| {
            Error::InvalidData(format!(
                "record variable '{}' element count exceeds platform usize",
                var.name
            ))
        })?;
    validate_record_span(
        var,
        numrecs,
        record_stride,
        bytes_per_record as u64,
        storage.len(),
    )?;
    let mut values = Vec::with_capacity(total_elements);

    for rec in 0..numrecs {
        let rec_offset = record_byte_offset(var, rec, record_stride)?;
        let rec_slice = storage.read_range(rec_offset, bytes_per_record)?;
        let rec_values = T::decode_bulk_be(rec_slice.as_ref(), elements_per_record)?;
        values.extend(rec_values);
    }

    ArrayD::from_shape_vec(IxDyn(&shape), values)
        .map_err(|e| Error::InvalidData(format!("failed to create array: {}", e)))
}

/// Read the entire data for a record variable using Rayon for large reads.
#[cfg(feature = "rayon")]
pub(crate) fn read_record_variable_parallel_from_storage<T: NcReadType>(
    storage: &ClassicStorage,
    var: &NcVariable,
    numrecs: u64,
    record_stride: u64,
) -> Result<ArrayD<T>> {
    if !var.is_record_var {
        return Err(Error::InvalidData(
            "use read_non_record_variable_parallel_from_storage for non-record variables"
                .to_string(),
        ));
    }

    let shape = checked_record_shape(var, numrecs)?;
    let elements_per_record = checked_record_elements_per_record(var)?;
    let bytes_per_record = variable_data_bytes::<T>(var.name.as_str(), elements_per_record)?;
    let numrecs_usize = crate::types::checked_usize_from_u64(numrecs, "record count")?;
    let total_elements = numrecs_usize
        .checked_mul(elements_per_record)
        .ok_or_else(|| {
            Error::InvalidData(format!(
                "record variable '{}' element count exceeds platform usize",
                var.name
            ))
        })?;
    let logical_bytes = numrecs_usize.checked_mul(bytes_per_record).ok_or_else(|| {
        Error::InvalidData(format!(
            "record variable '{}' logical byte count exceeds platform usize",
            var.name
        ))
    })?;
    validate_record_span(
        var,
        numrecs,
        record_stride,
        bytes_per_record as u64,
        storage.len(),
    )?;
    let policy = storage.parallel_read_policy();
    if logical_bytes < policy.min_bytes || numrecs_usize <= 1 {
        return read_record_variable_from_storage(storage, var, numrecs, record_stride);
    }

    let records_per_chunk = (policy.target_chunk_bytes / bytes_per_record.max(1)).max(1);
    let elements_per_chunk = records_per_chunk
        .checked_mul(elements_per_record)
        .ok_or_else(|| {
            Error::InvalidData(
                "classic record chunk element count exceeds platform usize".to_string(),
            )
        })?;
    let mut values = vec![T::default(); total_elements];
    let chunk_plan = RecordChunkReadPlan {
        var,
        record_stride,
        elements_per_record,
        bytes_per_record,
    };
    values
        .par_chunks_mut(elements_per_chunk)
        .enumerate()
        .try_for_each(|(chunk, dst)| {
            let first_record = chunk.checked_mul(records_per_chunk).ok_or_else(|| {
                Error::InvalidData("classic record chunk offset exceeds platform usize".to_string())
            })?;
            let records = dst.len().checked_div(elements_per_record).ok_or_else(|| {
                Error::InvalidData("classic record elements per record is zero".to_string())
            })?;
            read_record_chunk_into::<T>(storage, &chunk_plan, first_record as u64, records, dst)
        })?;

    ArrayD::from_shape_vec(IxDyn(&shape), values)
        .map_err(|e| Error::InvalidData(format!("failed to create array: {}", e)))
}

/// Read the entire data for a record variable into a caller-provided buffer.
pub fn read_record_variable_into<T: NcReadType>(
    file_data: &[u8],
    var: &NcVariable,
    numrecs: u64,
    record_stride: u64,
    dst: &mut [T],
) -> Result<()> {
    if !var.is_record_var {
        return Err(Error::InvalidData(
            "use read_non_record_variable_into for non-record variables".to_string(),
        ));
    }

    let elem_size = T::element_size();
    let base_offset =
        crate::types::checked_usize_from_u64(var.data_offset, "record variable data offset")?;
    let numrecs_usize = crate::types::checked_usize_from_u64(numrecs, "record count")?;
    let record_stride_usize = crate::types::checked_usize_from_u64(record_stride, "record stride")?;

    if var.dimensions.is_empty() {
        return Err(Error::InvalidData(
            "record variable must have at least one dimension".to_string(),
        ));
    }

    let elements_per_record = checked_record_elements_per_record(var)?;
    let bytes_per_record = elements_per_record.checked_mul(elem_size).ok_or_else(|| {
        Error::InvalidData(format!(
            "record variable '{}' bytes per record exceed platform usize",
            var.name
        ))
    })?;
    let total_elements = numrecs_usize
        .checked_mul(elements_per_record)
        .ok_or_else(|| {
            Error::InvalidData(format!(
                "record variable '{}' element count exceeds platform usize",
                var.name
            ))
        })?;
    if dst.len() != total_elements {
        return Err(Error::InvalidData(format!(
            "destination has {} elements, variable '{}' requires {}",
            dst.len(),
            var.name,
            total_elements
        )));
    }

    for rec in 0..numrecs_usize {
        let rec_offset = base_offset
            .checked_add(rec.checked_mul(record_stride_usize).ok_or_else(|| {
                Error::InvalidData(format!(
                    "record variable '{}' byte offset exceeds platform usize",
                    var.name
                ))
            })?)
            .ok_or_else(|| {
                Error::InvalidData(format!(
                    "record variable '{}' byte offset exceeds platform usize",
                    var.name
                ))
            })?;
        let rec_end = rec_offset.checked_add(bytes_per_record).ok_or_else(|| {
            Error::InvalidData(format!(
                "record variable '{}' record range exceeds platform usize",
                var.name
            ))
        })?;
        if rec_end > file_data.len() {
            return Err(Error::InvalidData(format!(
                "record {} for variable '{}' extends beyond file",
                rec, var.name
            )));
        }

        let dst_start = rec.checked_mul(elements_per_record).ok_or_else(|| {
            Error::InvalidData(format!(
                "record variable '{}' destination offset exceeds platform usize",
                var.name
            ))
        })?;
        let dst_end = dst_start.checked_add(elements_per_record).ok_or_else(|| {
            Error::InvalidData(format!(
                "record variable '{}' destination range exceeds platform usize",
                var.name
            ))
        })?;
        T::decode_bulk_be_into(
            &file_data[rec_offset..rec_end],
            &mut dst[dst_start..dst_end],
        )?;
    }

    Ok(())
}

/// Read a record variable from range-backed storage into a caller-provided buffer.
pub(crate) fn read_record_variable_into_from_storage<T: NcReadType>(
    storage: &ClassicStorage,
    var: &NcVariable,
    numrecs: u64,
    record_stride: u64,
    dst: &mut [T],
) -> Result<()> {
    if !var.is_record_var {
        return Err(Error::InvalidData(
            "use read_non_record_variable_into_from_storage for non-record variables".to_string(),
        ));
    }

    if var.dimensions.is_empty() {
        return Err(Error::InvalidData(
            "record variable must have at least one dimension".to_string(),
        ));
    }

    let elements_per_record = checked_record_elements_per_record(var)?;
    let bytes_per_record = variable_data_bytes::<T>(var.name.as_str(), elements_per_record)?;
    let numrecs_usize = crate::types::checked_usize_from_u64(numrecs, "record count")?;
    let total_elements = numrecs_usize
        .checked_mul(elements_per_record)
        .ok_or_else(|| {
            Error::InvalidData(format!(
                "record variable '{}' element count exceeds platform usize",
                var.name
            ))
        })?;
    if dst.len() != total_elements {
        return Err(Error::InvalidData(format!(
            "destination has {} elements, variable '{}' requires {}",
            dst.len(),
            var.name,
            total_elements
        )));
    }

    for rec in 0..numrecs {
        let rec_offset = record_byte_offset(var, rec, record_stride)?;
        let rec_slice = storage.read_range(rec_offset, bytes_per_record)?;
        let dst_start = crate::types::checked_usize_from_u64(rec, "record index")?
            .checked_mul(elements_per_record)
            .ok_or_else(|| {
                Error::InvalidData(format!(
                    "record variable '{}' destination offset exceeds platform usize",
                    var.name
                ))
            })?;
        let dst_end = dst_start.checked_add(elements_per_record).ok_or_else(|| {
            Error::InvalidData(format!(
                "record variable '{}' destination range exceeds platform usize",
                var.name
            ))
        })?;
        T::decode_bulk_be_into(rec_slice.as_ref(), &mut dst[dst_start..dst_end])?;
    }

    Ok(())
}

/// Compute the record stride: total bytes per record across all record variables.
///
/// Each record variable's per-record contribution is its unpadded
/// `record_size` (derived from its dimensions), padded to a 4-byte boundary —
/// except in the spec's special case of exactly one record variable, whose
/// records are packed without padding.
pub fn compute_record_stride(variables: &[NcVariable]) -> Result<u64> {
    let mut record_vars = variables.iter().filter(|v| v.is_record_var);
    let (first, second) = (record_vars.next(), record_vars.next());
    if let (Some(only), None) = (first, second) {
        return Ok(only.record_size);
    }
    variables
        .iter()
        .filter(|v| v.is_record_var)
        .try_fold(0u64, |stride, v| {
            let size = v.record_size;
            // Pad each variable's per-record size to 4-byte boundary.
            let rem = size % 4;
            let padded = if rem == 0 {
                size
            } else {
                size.checked_add(4 - rem).ok_or_else(|| {
                    Error::InvalidData(format!(
                        "record variable '{}' padded record size exceeds u64",
                        v.name
                    ))
                })?
            };
            stride
                .checked_add(padded)
                .ok_or_else(|| Error::InvalidData("record stride exceeds u64".to_string()))
        })
}

fn checked_non_record_element_count(var: &NcVariable) -> Result<usize> {
    let mut total = 1u64;
    for dim in &var.dimensions {
        total = total.checked_mul(dim.size).ok_or_else(|| {
            Error::InvalidData("variable element count overflows u64".to_string())
        })?;
    }
    crate::types::checked_usize_from_u64(total, "variable element count")
}

pub(crate) fn checked_variable_shape(var: &NcVariable) -> Result<Vec<usize>> {
    var.shape()
        .iter()
        .map(|&s| crate::types::checked_usize_from_u64(s, "variable dimension"))
        .collect::<Result<Vec<_>>>()
}

fn checked_record_shape(var: &NcVariable, numrecs: u64) -> Result<Vec<usize>> {
    let mut shape: Vec<usize> = var
        .shape()
        .iter()
        .map(|&s| crate::types::checked_usize_from_u64(s, "record variable dimension"))
        .collect::<Result<Vec<_>>>()?;
    if shape.is_empty() {
        return Err(Error::InvalidData(
            "record variable must have at least one dimension".to_string(),
        ));
    }
    shape[0] = crate::types::checked_usize_from_u64(numrecs, "record count")?;
    Ok(shape)
}

fn checked_record_elements_per_record(var: &NcVariable) -> Result<usize> {
    let mut elements = 1usize;
    for dim in var.dimensions.iter().skip(1) {
        let size = crate::types::checked_usize_from_u64(dim.size, "record variable dimension")?;
        elements = elements.checked_mul(size).ok_or_else(|| {
            Error::InvalidData(format!(
                "record variable '{}' elements per record exceed platform usize",
                var.name
            ))
        })?;
    }
    Ok(elements)
}

pub(crate) fn variable_data_bytes<T: NcReadType>(
    var_name: &str,
    element_count: usize,
) -> Result<usize> {
    element_count.checked_mul(T::element_size()).ok_or_else(|| {
        Error::InvalidData(format!(
            "variable '{var_name}' size in bytes exceeds platform usize"
        ))
    })
}

/// Validate that all `numrecs` records fit within `available` bytes before a
/// caller allocates the whole result buffer, so a crafted record count cannot
/// force a huge up-front allocation ahead of the per-record bounds checks.
pub(crate) fn validate_record_span(
    var: &NcVariable,
    numrecs: u64,
    record_stride: u64,
    bytes_per_record: u64,
    available: u64,
) -> Result<()> {
    if numrecs == 0 {
        return Ok(());
    }
    let last_record_start = record_byte_offset(var, numrecs - 1, record_stride)?;
    let end = last_record_start
        .checked_add(bytes_per_record)
        .ok_or_else(|| {
            Error::InvalidData(format!(
                "record variable '{}' data extent exceeds u64",
                var.name
            ))
        })?;
    if end > available {
        return Err(Error::InvalidData(format!(
            "record variable '{}' needs {end} bytes but only {available} are available",
            var.name
        )));
    }
    Ok(())
}

pub(crate) fn record_byte_offset(var: &NcVariable, record: u64, record_stride: u64) -> Result<u64> {
    var.data_offset
        .checked_add(record.checked_mul(record_stride).ok_or_else(|| {
            Error::InvalidData(format!(
                "record variable '{}' byte offset exceeds u64",
                var.name
            ))
        })?)
        .ok_or_else(|| {
            Error::InvalidData(format!(
                "record variable '{}' byte offset exceeds u64",
                var.name
            ))
        })
}

#[cfg(feature = "rayon")]
pub(crate) fn read_contiguous_range_parallel<T: NcReadType>(
    storage: &ClassicStorage,
    base_offset: u64,
    total_elements: usize,
    target_chunk_bytes: usize,
) -> Result<Vec<T>> {
    if total_elements == 0 {
        return Ok(Vec::new());
    }

    let elem_size = T::element_size();
    let elements_per_chunk = (target_chunk_bytes / elem_size.max(1)).max(1);
    let mut values = vec![T::default(); total_elements];
    values
        .par_chunks_mut(elements_per_chunk)
        .enumerate()
        .try_for_each(|(chunk, dst)| {
            let start_element = chunk.checked_mul(elements_per_chunk).ok_or_else(|| {
                Error::InvalidData(
                    "classic parallel chunk offset exceeds platform usize".to_string(),
                )
            })?;
            let byte_offset = crate::types::checked_mul_u64(
                start_element as u64,
                elem_size as u64,
                "classic parallel byte offset",
            )?;
            let offset = base_offset.checked_add(byte_offset).ok_or_else(|| {
                Error::InvalidData("classic parallel byte offset exceeds u64".to_string())
            })?;
            let bytes = variable_data_bytes::<T>("parallel chunk", dst.len())?;
            let data = storage.read_range(offset, bytes)?;
            T::decode_bulk_be_into(data.as_ref(), dst)
        })?;

    Ok(values)
}

#[cfg(feature = "rayon")]
struct RecordChunkReadPlan<'a> {
    var: &'a NcVariable,
    record_stride: u64,
    elements_per_record: usize,
    bytes_per_record: usize,
}

#[cfg(feature = "rayon")]
fn read_record_chunk_into<T: NcReadType>(
    storage: &ClassicStorage,
    plan: &RecordChunkReadPlan<'_>,
    first_record: u64,
    records: usize,
    dst: &mut [T],
) -> Result<()> {
    if records == 0 {
        return Ok(());
    }

    let expected_elements = records
        .checked_mul(plan.elements_per_record)
        .ok_or_else(|| {
            Error::InvalidData(format!(
                "record variable '{}' chunk element count exceeds platform usize",
                plan.var.name
            ))
        })?;
    if dst.len() != expected_elements {
        return Err(Error::InvalidData(format!(
            "record variable '{}' chunk destination has {} elements, expected {}",
            plan.var.name,
            dst.len(),
            expected_elements
        )));
    }

    if plan.record_stride == plan.bytes_per_record as u64 {
        let offset = record_byte_offset(plan.var, first_record, plan.record_stride)?;
        let bytes = records.checked_mul(plan.bytes_per_record).ok_or_else(|| {
            Error::InvalidData(format!(
                "record variable '{}' chunk byte count exceeds platform usize",
                plan.var.name
            ))
        })?;
        let data = storage.read_range(offset, bytes)?;
        return T::decode_bulk_be_into(data.as_ref(), dst);
    }

    for ordinal in 0..records {
        let record = first_record
            .checked_add(ordinal as u64)
            .ok_or_else(|| Error::InvalidData("classic record index exceeds u64".to_string()))?;
        let offset = record_byte_offset(plan.var, record, plan.record_stride)?;
        let data = storage.read_range(offset, plan.bytes_per_record)?;
        let dst_start = ordinal
            .checked_mul(plan.elements_per_record)
            .ok_or_else(|| {
                Error::InvalidData(format!(
                    "record variable '{}' chunk destination offset exceeds platform usize",
                    plan.var.name
                ))
            })?;
        let dst_end = dst_start
            .checked_add(plan.elements_per_record)
            .ok_or_else(|| {
                Error::InvalidData(format!(
                    "record variable '{}' chunk destination range exceeds platform usize",
                    plan.var.name
                ))
            })?;
        T::decode_bulk_be_into(data.as_ref(), &mut dst[dst_start..dst_end])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NcDimension;

    #[test]
    fn read_non_record_1d_float() {
        // Create a fake file with 3 floats starting at offset 100.
        let mut file_data = vec![0u8; 200];
        let values = [1.0f32, 2.0f32, 3.0f32];
        for (i, &v) in values.iter().enumerate() {
            let bytes = v.to_be_bytes();
            file_data[100 + i * 4..100 + i * 4 + 4].copy_from_slice(&bytes);
        }

        let var = NcVariable {
            name: "temp".to_string(),
            dimensions: vec![NcDimension {
                name: "x".to_string(),
                size: 3,
                is_unlimited: false,
            }],
            dtype: NcType::Float,
            attributes: vec![],
            data_offset: 100,
            _data_size: 12,
            is_record_var: false,
            record_size: 0,
        };

        let arr: ArrayD<f32> = read_non_record_variable(&file_data, &var).unwrap();
        assert_eq!(arr.shape(), &[3]);
        assert_eq!(arr[[0]], 1.0f32);
        assert_eq!(arr[[1]], 2.0f32);
        assert_eq!(arr[[2]], 3.0f32);
    }

    #[test]
    fn non_record_variable_into_copies_values() {
        let mut file_data = vec![0u8; 200];
        let values = [1.0f32, 2.0f32, 3.0f32];
        for (i, &v) in values.iter().enumerate() {
            file_data[100 + i * 4..100 + i * 4 + 4].copy_from_slice(&v.to_be_bytes());
        }

        let var = NcVariable {
            name: "temp".to_string(),
            dimensions: vec![NcDimension {
                name: "x".to_string(),
                size: 3,
                is_unlimited: false,
            }],
            dtype: NcType::Float,
            attributes: vec![],
            data_offset: 100,
            _data_size: 12,
            is_record_var: false,
            record_size: 0,
        };

        let mut dst = [0.0f32; 3];
        read_non_record_variable_into(&file_data, &var, &mut dst).unwrap();
        assert_eq!(dst, values);
    }

    #[test]
    fn read_non_record_2d_int() {
        // 2x3 array of i32 at offset 0
        let values: Vec<i32> = vec![10, 20, 30, 40, 50, 60];
        let mut file_data = Vec::new();
        for &v in &values {
            file_data.extend_from_slice(&v.to_be_bytes());
        }

        let var = NcVariable {
            name: "grid".to_string(),
            dimensions: vec![
                NcDimension {
                    name: "y".to_string(),
                    size: 2,
                    is_unlimited: false,
                },
                NcDimension {
                    name: "x".to_string(),
                    size: 3,
                    is_unlimited: false,
                },
            ],
            dtype: NcType::Int,
            attributes: vec![],
            data_offset: 0,
            _data_size: 24,
            is_record_var: false,
            record_size: 0,
        };

        let arr: ArrayD<i32> = read_non_record_variable(&file_data, &var).unwrap();
        assert_eq!(arr.shape(), &[2, 3]);
        assert_eq!(arr[[0, 0]], 10);
        assert_eq!(arr[[0, 2]], 30);
        assert_eq!(arr[[1, 0]], 40);
        assert_eq!(arr[[1, 2]], 60);
    }

    #[test]
    fn read_non_record_variable_into_rejects_wrong_destination_len() {
        let var = NcVariable {
            name: "grid".to_string(),
            dimensions: vec![NcDimension {
                name: "x".to_string(),
                size: 3,
                is_unlimited: false,
            }],
            dtype: NcType::Float,
            attributes: vec![],
            data_offset: 0,
            _data_size: 12,
            is_record_var: false,
            record_size: 0,
        };

        let mut dst = [0.0f32; 2];
        let err = read_non_record_variable_into(&[0; 12], &var, &mut dst).unwrap_err();
        assert!(matches!(err, Error::InvalidData(_)));
    }

    #[test]
    fn record_stride_sums_padded_record_variables() {
        let vars = vec![
            NcVariable {
                name: "a".to_string(),
                dimensions: vec![],
                dtype: NcType::Float,
                attributes: vec![],
                data_offset: 0,
                _data_size: 0,
                is_record_var: true,
                record_size: 20, // 5 floats
            },
            NcVariable {
                name: "b".to_string(),
                dimensions: vec![],
                dtype: NcType::Short,
                attributes: vec![],
                data_offset: 0,
                _data_size: 0,
                is_record_var: true,
                record_size: 6, // 3 shorts -> padded to 8
            },
            NcVariable {
                name: "c".to_string(),
                dimensions: vec![],
                dtype: NcType::Double,
                attributes: vec![],
                data_offset: 0,
                _data_size: 100,
                is_record_var: false, // not a record var, should be excluded
                record_size: 0,
            },
        ];
        // a: 20 (already 4-aligned), b: 6 -> 8 = total 28
        assert_eq!(compute_record_stride(&vars).unwrap(), 28);
    }

    #[test]
    fn record_stride_rejects_padded_size_overflow() {
        // Two record variables so per-variable padding applies (a lone
        // record variable is packed without padding).
        let vars = vec![
            NcVariable {
                name: "huge".to_string(),
                dimensions: vec![],
                dtype: NcType::Byte,
                attributes: vec![],
                data_offset: 0,
                _data_size: 0,
                is_record_var: true,
                record_size: u64::MAX,
            },
            NcVariable {
                name: "small".to_string(),
                dimensions: vec![],
                dtype: NcType::Byte,
                attributes: vec![],
                data_offset: 0,
                _data_size: 0,
                is_record_var: true,
                record_size: 4,
            },
        ];

        let err = compute_record_stride(&vars).unwrap_err();
        assert!(matches!(err, Error::InvalidData(_)));
    }

    #[test]
    fn record_stride_rejects_sum_overflow() {
        let vars = vec![
            NcVariable {
                name: "a".to_string(),
                dimensions: vec![],
                dtype: NcType::Byte,
                attributes: vec![],
                data_offset: 0,
                _data_size: 0,
                is_record_var: true,
                record_size: u64::MAX - 7,
            },
            NcVariable {
                name: "b".to_string(),
                dimensions: vec![],
                dtype: NcType::Byte,
                attributes: vec![],
                data_offset: 0,
                _data_size: 0,
                is_record_var: true,
                record_size: 8,
            },
        ];

        let err = compute_record_stride(&vars).unwrap_err();
        assert!(matches!(err, Error::InvalidData(_)));
    }

    #[test]
    fn record_variable_reads_all_records() {
        // Single record variable "temp" with shape [time, x] where x=2.
        // 3 records, each with 2 floats = 8 bytes per record.
        // Record stride = 8 (only one record var, already 4-aligned).
        let mut file_data = vec![0u8; 200];
        let base = 100usize;
        let record_values: Vec<Vec<f32>> = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]];
        for (rec, vals) in record_values.iter().enumerate() {
            for (i, &v) in vals.iter().enumerate() {
                let offset = base + rec * 8 + i * 4;
                file_data[offset..offset + 4].copy_from_slice(&v.to_be_bytes());
            }
        }

        let var = NcVariable {
            name: "temp".to_string(),
            dimensions: vec![
                NcDimension {
                    name: "time".to_string(),
                    size: 0, // unlimited
                    is_unlimited: true,
                },
                NcDimension {
                    name: "x".to_string(),
                    size: 2,
                    is_unlimited: false,
                },
            ],
            dtype: NcType::Float,
            attributes: vec![],
            data_offset: 100,
            _data_size: 0,
            is_record_var: true,
            record_size: 8,
        };

        let arr: ArrayD<f32> = read_record_variable(&file_data, &var, 3, 8).unwrap();
        assert_eq!(arr.shape(), &[3, 2]);
        assert_eq!(arr[[0, 0]], 1.0);
        assert_eq!(arr[[0, 1]], 2.0);
        assert_eq!(arr[[1, 0]], 3.0);
        assert_eq!(arr[[2, 1]], 6.0);
    }

    #[test]
    fn record_variable_into_copies_values() {
        let mut file_data = vec![0u8; 200];
        let base = 100usize;
        let record_values: Vec<Vec<f32>> = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]];
        for (rec, vals) in record_values.iter().enumerate() {
            for (i, &v) in vals.iter().enumerate() {
                let offset = base + rec * 8 + i * 4;
                file_data[offset..offset + 4].copy_from_slice(&v.to_be_bytes());
            }
        }

        let var = NcVariable {
            name: "temp".to_string(),
            dimensions: vec![
                NcDimension {
                    name: "time".to_string(),
                    size: 0,
                    is_unlimited: true,
                },
                NcDimension {
                    name: "x".to_string(),
                    size: 2,
                    is_unlimited: false,
                },
            ],
            dtype: NcType::Float,
            attributes: vec![],
            data_offset: 100,
            _data_size: 0,
            is_record_var: true,
            record_size: 8,
        };

        let mut dst = [0.0f32; 6];
        read_record_variable_into(&file_data, &var, 3, 8, &mut dst).unwrap();
        assert_eq!(dst, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn read_record_variable_into_rejects_wrong_destination_len() {
        let var = NcVariable {
            name: "temp".to_string(),
            dimensions: vec![
                NcDimension {
                    name: "time".to_string(),
                    size: 0,
                    is_unlimited: true,
                },
                NcDimension {
                    name: "x".to_string(),
                    size: 2,
                    is_unlimited: false,
                },
            ],
            dtype: NcType::Float,
            attributes: vec![],
            data_offset: 0,
            _data_size: 0,
            is_record_var: true,
            record_size: 8,
        };

        let mut dst = [0.0f32; 5];
        let err = read_record_variable_into(&[0; 24], &var, 3, 8, &mut dst).unwrap_err();
        assert!(matches!(err, Error::InvalidData(_)));
    }

    #[test]
    fn read_non_record_variable_rejects_element_count_overflow() {
        let var = NcVariable {
            name: "huge".to_string(),
            dimensions: vec![
                NcDimension {
                    name: "y".to_string(),
                    size: u64::MAX,
                    is_unlimited: false,
                },
                NcDimension {
                    name: "x".to_string(),
                    size: 2,
                    is_unlimited: false,
                },
            ],
            dtype: NcType::Float,
            attributes: vec![],
            data_offset: 0,
            _data_size: 0,
            is_record_var: false,
            record_size: 0,
        };

        let err = read_non_record_variable::<f32>(&[], &var).unwrap_err();
        assert!(matches!(err, Error::InvalidData(_)));
    }

    #[test]
    fn read_record_variable_rejects_elements_per_record_overflow() {
        let var = NcVariable {
            name: "huge_record".to_string(),
            dimensions: vec![
                NcDimension {
                    name: "time".to_string(),
                    size: 0,
                    is_unlimited: true,
                },
                NcDimension {
                    name: "y".to_string(),
                    size: usize::MAX as u64,
                    is_unlimited: false,
                },
                NcDimension {
                    name: "x".to_string(),
                    size: 2,
                    is_unlimited: false,
                },
            ],
            dtype: NcType::Float,
            attributes: vec![],
            data_offset: 0,
            _data_size: 0,
            is_record_var: true,
            record_size: 4,
        };

        let err = read_record_variable::<f32>(&[], &var, 1, 4).unwrap_err();
        assert!(matches!(err, Error::InvalidData(_)));
    }

    #[test]
    fn read_record_variable_rejects_record_offset_overflow() {
        let var = NcVariable {
            name: "huge_record".to_string(),
            dimensions: vec![
                NcDimension {
                    name: "time".to_string(),
                    size: 0,
                    is_unlimited: true,
                },
                NcDimension {
                    name: "x".to_string(),
                    size: 1,
                    is_unlimited: false,
                },
            ],
            dtype: NcType::Float,
            attributes: vec![],
            data_offset: u64::MAX,
            _data_size: 0,
            is_record_var: true,
            record_size: 4,
        };

        let err = read_record_variable::<f32>(&[], &var, 1, 4).unwrap_err();
        assert!(matches!(err, Error::InvalidData(_)));
    }
}
