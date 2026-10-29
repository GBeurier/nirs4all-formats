//! Classic (CDF-1/2/5) NetCDF file format support.
//!
//! This module handles the original NetCDF binary format (CDF-1 classic, CDF-2
//! 64-bit offset, and CDF-5 64-bit data). All multi-byte values are big-endian.

pub mod data;
pub mod header;
pub(crate) mod storage;
pub mod types;
pub mod variable;

use std::fs::File;
use std::path::Path;

use memmap2::Mmap;

use crate::error::{Error, Result};
use crate::types::NcGroup;
use crate::NcFormat;

use storage::ClassicStorage;

/// An opened classic-format NetCDF file (CDF-1, CDF-2, or CDF-5).
pub struct ClassicFile {
    pub(crate) format: NcFormat,
    pub(crate) root_group: NcGroup,
    pub(crate) storage: ClassicStorage,
    pub(crate) numrecs: u64,
}

impl ClassicFile {
    /// Open a classic NetCDF file from disk using memory-mapping.
    pub fn open(path: &Path, format: NcFormat) -> Result<Self> {
        let file = File::open(path)?;
        // SAFETY: read-only mapping; caller must not modify the file concurrently.
        let mmap = unsafe { Mmap::map(&file)? };
        let is_streaming = header::has_streaming_numrecs(&mmap, format);
        let header = header::parse_header(&mmap, format)?;
        let storage = ClassicStorage::from_mmap(mmap);
        let (root_group, numrecs) = finalize_header(header, storage.len(), is_streaming)?;

        Ok(ClassicFile {
            format,
            root_group,
            storage,
            numrecs,
        })
    }

    /// Open a classic NetCDF file from in-memory bytes.
    pub fn from_bytes(bytes: &[u8], format: NcFormat) -> Result<Self> {
        let is_streaming = header::has_streaming_numrecs(bytes, format);
        let header = header::parse_header(bytes, format)?;
        let storage = ClassicStorage::from_bytes(bytes.to_vec());
        let (root_group, numrecs) = finalize_header(header, storage.len(), is_streaming)?;

        Ok(ClassicFile {
            format,
            root_group,
            storage,
            numrecs,
        })
    }

    /// Open a classic NetCDF file from an existing memory map (avoids double mmap).
    pub fn from_mmap(mmap: Mmap, format: NcFormat) -> Result<Self> {
        let is_streaming = header::has_streaming_numrecs(&mmap, format);
        let header = header::parse_header(&mmap, format)?;
        let storage = ClassicStorage::from_mmap(mmap);
        let (root_group, numrecs) = finalize_header(header, storage.len(), is_streaming)?;

        Ok(ClassicFile {
            format,
            root_group,
            storage,
            numrecs,
        })
    }

    /// Open a classic NetCDF file from a random-access storage backend.
    #[cfg(feature = "netcdf4")]
    pub fn from_storage(
        storage: hdf5_reader::storage::DynStorage,
        format: NcFormat,
    ) -> Result<Self> {
        let storage = ClassicStorage::from_range(storage);
        let (header, is_streaming) = parse_header_from_storage(&storage, format)?;
        let (root_group, numrecs) = finalize_header(header, storage.len(), is_streaming)?;

        Ok(ClassicFile {
            format,
            root_group,
            storage,
            numrecs,
        })
    }

    /// The file format (Classic, Offset64, or Cdf5).
    pub fn format(&self) -> NcFormat {
        self.format
    }

    /// The root group containing all dimensions, variables, and global attributes.
    pub fn root_group(&self) -> &NcGroup {
        &self.root_group
    }

    /// Number of records in the unlimited dimension.
    pub fn numrecs(&self) -> u64 {
        self.numrecs
    }
}

fn finalize_header(
    mut header: header::ClassicHeader,
    storage_len: u64,
    is_streaming: bool,
) -> Result<(NcGroup, u64)> {
    reject_unsupported_classic_features(&header)?;

    if is_streaming {
        header.numrecs = infer_streaming_numrecs(&header, storage_len)?;
        header::apply_unlimited_dimension_size(
            &mut header.dimensions,
            &mut header.variables,
            header.numrecs,
        );
    } else {
        validate_record_extent(&header, storage_len)?;
    }

    let numrecs = header.numrecs;
    let root_group = NcGroup {
        name: "/".to_string(),
        dimensions: header.dimensions,
        variables: header.variables,
        attributes: header.global_attributes,
        groups: Vec::new(), // Classic format has no sub-groups.
    };

    Ok((root_group, numrecs))
}

/// Reject headers whose declared record count cannot fit in the file, so a
/// tiny crafted header cannot drive huge record-buffer allocations later.
fn validate_record_extent(header: &header::ClassicHeader, storage_len: u64) -> Result<u64> {
    let Some(record_data_start) = header
        .variables
        .iter()
        .filter(|var| var.is_record_var)
        .map(|var| var.data_offset)
        .min()
    else {
        return Ok(0);
    };

    let record_stride = data::compute_record_stride(&header.variables)?;
    let record_bytes = header
        .numrecs
        .checked_mul(record_stride)
        .and_then(|bytes| bytes.checked_add(record_data_start))
        .ok_or_else(|| {
            Error::InvalidData("classic record data extent overflows u64".to_string())
        })?;
    if record_bytes > storage_len {
        return Err(Error::InvalidData(format!(
            "classic header declares {} records ({record_bytes} bytes) but the file has only \
             {storage_len} bytes",
            header.numrecs
        )));
    }
    Ok(record_bytes)
}

fn infer_streaming_numrecs(header: &header::ClassicHeader, storage_len: u64) -> Result<u64> {
    let Some(record_data_start) = header
        .variables
        .iter()
        .filter(|var| var.is_record_var)
        .map(|var| var.data_offset)
        .min()
    else {
        return Ok(0);
    };

    let record_stride = data::compute_record_stride(&header.variables)?;
    if record_stride == 0 || storage_len <= record_data_start {
        return Ok(0);
    }

    Ok((storage_len - record_data_start) / record_stride)
}

fn reject_unsupported_classic_features(header: &header::ClassicHeader) -> Result<()> {
    let has_subfiling_marker = header
        .global_attributes
        .iter()
        .any(|attr| is_subfiling_attribute_name(&attr.name))
        || header.variables.iter().any(|var| {
            var.attributes
                .iter()
                .any(|attr| is_subfiling_attribute_name(&attr.name))
        });

    if has_subfiling_marker {
        return Err(crate::Error::UnsupportedFeature(
            "PnetCDF subfiling datasets require a virtual multi-file storage adapter".to_string(),
        ));
    }

    Ok(())
}

fn is_subfiling_attribute_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("_pnetcdf_subfiling") || lower.starts_with("subfiling")
}

#[cfg(feature = "netcdf4")]
fn parse_header_from_storage(
    storage: &ClassicStorage,
    format: NcFormat,
) -> Result<(header::ClassicHeader, bool)> {
    let mut len = storage.initial_header_len();

    loop {
        let prefix = storage.read_header_prefix(len)?;
        match header::parse_header(prefix.as_ref(), format) {
            Ok(header) => {
                let is_streaming = header::has_streaming_numrecs(prefix.as_ref(), format);
                return Ok((header, is_streaming));
            }
            Err(crate::Error::UnexpectedEof { .. }) if (prefix.len() as u64) < storage.len() => {
                let current = prefix.len().max(1);
                len = current.saturating_mul(2);
            }
            Err(err) => return Err(err),
        }
    }
}
