//! NetCDF-4 (HDF5-backed) format support.
//!
//! This module maps the HDF5 data model to the NetCDF data model:
//! - HDF5 groups become NetCDF groups
//! - HDF5 datasets become NetCDF variables
//! - HDF5 attributes become NetCDF attributes
//! - Dimensions are reconstructed from `DIMENSION_LIST` and `_Netcdf4Dimid` attributes
//!
//! Requires the `netcdf4` feature (enabled by default).

pub mod attributes;
pub mod dimensions;
pub mod groups;
pub mod types;
pub mod variables;

use std::path::Path;
use std::sync::OnceLock;

use hdf5_reader::datatype_api::H5Type;
use hdf5_reader::storage::DynStorage;
use hdf5_reader::Hdf5File;
use ndarray::ArrayD;
#[cfg(feature = "rayon")]
use rayon::ThreadPool;

use crate::error::{Error, Result};
use crate::types::{checked_shape_elements, checked_usize_from_u64, NcGroup, NcType};

/// Dispatch on `NcType` to read data and promote to `f64`.
///
/// `$dtype` must be an expression of type `&NcType`.
/// `$read_expr` is a macro-like callback: for each numeric type `$T`,
/// the macro evaluates `$read_expr` with `$T` substituted in.
///
/// Usage:
/// ```ignore
/// dispatch_read_as_f64!(&var.dtype, |$T| dataset.read_array::<$T>())
/// ```
macro_rules! dispatch_read_as_f64 {
    ($dtype:expr, |$T:ident| $read_expr:expr) => {{
        use crate::types::NcType;
        match $dtype {
            NcType::Byte => {
                type $T = i8;
                let arr = $read_expr?;
                Ok(arr.mapv(|v| v as f64))
            }
            NcType::Short => {
                type $T = i16;
                let arr = $read_expr?;
                Ok(arr.mapv(|v| v as f64))
            }
            NcType::Int => {
                type $T = i32;
                let arr = $read_expr?;
                Ok(arr.mapv(|v| v as f64))
            }
            NcType::Float => {
                type $T = f32;
                let arr = $read_expr?;
                Ok(arr.mapv(|v| v as f64))
            }
            NcType::Double => {
                type $T = f64;
                Ok($read_expr?)
            }
            NcType::UByte => {
                type $T = u8;
                let arr = $read_expr?;
                Ok(arr.mapv(|v| v as f64))
            }
            NcType::UShort => {
                type $T = u16;
                let arr = $read_expr?;
                Ok(arr.mapv(|v| v as f64))
            }
            NcType::UInt => {
                type $T = u32;
                let arr = $read_expr?;
                Ok(arr.mapv(|v| v as f64))
            }
            NcType::Int64 => {
                type $T = i64;
                let arr = $read_expr?;
                Ok(arr.mapv(|v| v as f64))
            }
            NcType::UInt64 => {
                type $T = u64;
                let arr = $read_expr?;
                Ok(arr.mapv(|v| v as f64))
            }
            NcType::Char => Err(Error::TypeMismatch {
                expected: "numeric type".to_string(),
                actual: "Char".to_string(),
            }),
            NcType::String => Err(Error::TypeMismatch {
                expected: "numeric type".to_string(),
                actual: "String".to_string(),
            }),
            other => Err(Error::TypeMismatch {
                expected: "numeric type".to_string(),
                actual: format!("{:?}", other),
            }),
        }
    }};
}

/// True when an HDF5 file carries none of NetCDF-4's markers: no `_NCProperties` root
/// attribute and no dimension-scale dataset in the root group.
fn is_plain_hdf5(hdf5: &Hdf5File) -> bool {
    let Ok(root) = hdf5.root_group() else {
        return false;
    };
    if root.attribute("_NCProperties").is_ok() {
        return false;
    }
    let Ok(datasets) = root.datasets() else {
        return false;
    };
    !datasets.iter().any(|ds| {
        ds.attribute("CLASS")
            .ok()
            .and_then(|a| a.read_string().ok())
            .is_some_and(|c| c == "DIMENSION_SCALE")
    })
}

/// An opened NetCDF-4 file (backed by HDF5).
pub struct Nc4File {
    hdf5: Hdf5File,
    metadata_mode: crate::NcMetadataMode,
    root_metadata: OnceLock<NcGroup>,
    metadata_tree: OnceLock<NcGroup>,
}

impl Nc4File {
    pub(crate) fn from_hdf5(hdf5: Hdf5File, metadata_mode: crate::NcMetadataMode) -> Result<Self> {
        // A plain HDF5 file (not written through NetCDF-4) has no `_NCProperties` attribute and
        // no dimension scales. netCDF-C reads such files with phony dimensions; strict NetCDF-4
        // validation would reject every dataset for lacking `DIMENSION_LIST`, so they are read
        // in lossy mode (dimensions named by size). NetCDF-4 files keep the requested mode.
        let metadata_mode =
            if metadata_mode == crate::NcMetadataMode::Strict && is_plain_hdf5(&hdf5) {
                crate::NcMetadataMode::Lossy
            } else {
                metadata_mode
            };
        Ok(Nc4File {
            hdf5,
            metadata_mode,
            root_metadata: OnceLock::new(),
            metadata_tree: OnceLock::new(),
        })
    }

    /// Open a NetCDF-4 file from disk.
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with_options(path, crate::NcOpenOptions::default())
    }

    /// Open a NetCDF-4 file from disk with custom options.
    pub fn open_with_options(path: &Path, options: crate::NcOpenOptions) -> Result<Self> {
        let metadata_mode = options.metadata_mode;
        let hdf5 = Hdf5File::open_with_options(
            path,
            hdf5_reader::OpenOptions {
                chunk_cache_bytes: options.chunk_cache_bytes,
                chunk_cache_slots: options.chunk_cache_slots,
                filter_registry: options.filter_registry,
                external_file_resolver: options.external_file_resolver,
                external_link_resolver: options.external_link_resolver,
            },
        )?;
        Nc4File::from_hdf5(hdf5, metadata_mode)
    }

    /// Open a NetCDF-4 file from in-memory bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        Self::from_bytes_with_options(data, crate::NcOpenOptions::default())
    }

    /// Open a NetCDF-4 file from in-memory bytes with custom options.
    pub fn from_bytes_with_options(data: &[u8], options: crate::NcOpenOptions) -> Result<Self> {
        let metadata_mode = options.metadata_mode;
        let hdf5 = Hdf5File::from_bytes_with_options(
            data,
            hdf5_reader::OpenOptions {
                chunk_cache_bytes: options.chunk_cache_bytes,
                chunk_cache_slots: options.chunk_cache_slots,
                filter_registry: options.filter_registry,
                external_file_resolver: options.external_file_resolver,
                external_link_resolver: options.external_link_resolver,
            },
        )?;
        Nc4File::from_hdf5(hdf5, metadata_mode)
    }

    /// Open a NetCDF-4 file from a custom random-access storage backend.
    pub fn from_storage(storage: DynStorage) -> Result<Self> {
        Self::from_storage_with_options(storage, crate::NcOpenOptions::default())
    }

    /// Open a NetCDF-4 file from a custom random-access storage backend with custom options.
    pub fn from_storage_with_options(
        storage: DynStorage,
        options: crate::NcOpenOptions,
    ) -> Result<Self> {
        let metadata_mode = options.metadata_mode;
        let hdf5 = Hdf5File::from_storage_with_options(
            storage,
            hdf5_reader::OpenOptions {
                chunk_cache_bytes: options.chunk_cache_bytes,
                chunk_cache_slots: options.chunk_cache_slots,
                filter_registry: options.filter_registry,
                external_file_resolver: options.external_file_resolver,
                external_link_resolver: options.external_link_resolver,
            },
        )?;
        Nc4File::from_hdf5(hdf5, metadata_mode)
    }

    /// The root group.
    pub fn root_group(&self) -> Result<&NcGroup> {
        if let Some(group) = self.metadata_tree.get() {
            return Ok(group);
        }
        let metadata_tree = groups::build_root_group(&self.hdf5, self.metadata_mode)?;
        let _ = self.metadata_tree.set(metadata_tree);
        Ok(self
            .metadata_tree
            .get()
            .expect("metadata tree must be initialized after successful build"))
    }

    /// Check if this file uses the classic data model (`_nc3_strict`).
    ///
    /// This checks the raw HDF5 root group attributes (before the internal
    /// attribute filter removes `_nc3_strict`).
    pub fn is_classic_model(&self) -> bool {
        self.hdf5
            .root_group()
            .ok()
            .and_then(|g| g.attribute("_nc3_strict").ok())
            .is_some()
    }

    pub fn dimensions(&self) -> Result<&[crate::types::NcDimension]> {
        Ok(&self.root_metadata()?.dimensions)
    }

    pub fn variables(&self) -> Result<&[crate::types::NcVariable]> {
        Ok(&self.root_metadata()?.variables)
    }

    pub fn global_attributes(&self) -> Result<&[crate::types::NcAttribute]> {
        Ok(&self.root_metadata()?.attributes)
    }

    pub fn group(&self, path: &str) -> Result<&NcGroup> {
        let normalized = normalize_group_path(path)?;
        let root = self.root_group()?;
        if normalized.is_empty() {
            return Ok(root);
        }
        root.group(normalized)
            .ok_or_else(|| Error::GroupNotFound(path.to_string()))
    }

    pub fn variable(&self, path: &str) -> Result<&crate::types::NcVariable> {
        self.root_group()?
            .variable(path)
            .ok_or_else(|| Error::VariableNotFound(path.to_string()))
    }

    pub fn dimension(&self, path: &str) -> Result<&crate::types::NcDimension> {
        self.root_group()?
            .dimension(path)
            .ok_or_else(|| Error::DimensionNotFound(path.to_string()))
    }

    pub fn global_attribute(&self, path: &str) -> Result<&crate::types::NcAttribute> {
        self.root_group()?
            .attribute(path)
            .ok_or_else(|| Error::AttributeNotFound(path.to_string()))
    }

    /// Read a variable's data as a typed array.
    ///
    /// Looks up the variable by path relative to the root group, then opens the
    /// matching HDF5 dataset and reads the data.
    pub fn read_variable<T: H5Type>(&self, path: &str) -> Result<ArrayD<T>> {
        let normalized = normalize_dataset_path(path)?;
        let dataset = self.hdf5.dataset(normalized)?;
        Ok(dataset.read_array::<T>()?)
    }

    /// Read a variable into a caller-provided typed buffer.
    pub fn read_variable_into<T: H5Type>(&self, path: &str, dst: &mut [T]) -> Result<()> {
        let normalized = normalize_dataset_path(path)?;
        let dataset = self.hdf5.dataset(normalized)?;
        Ok(dataset.read_into::<T>(dst)?)
    }

    /// Read a variable as logical raw bytes in HDF5 datatype byte order.
    pub fn read_variable_raw_bytes(&self, path: &str) -> Result<Vec<u8>> {
        let normalized = normalize_dataset_path(path)?;
        let dataset = self.hdf5.dataset(normalized)?;
        Ok(dataset.read_raw_bytes()?)
    }

    /// Read logical raw bytes into a caller-provided buffer.
    pub fn read_variable_raw_bytes_into(&self, path: &str, dst: &mut [u8]) -> Result<()> {
        let normalized = normalize_dataset_path(path)?;
        let dataset = self.hdf5.dataset(normalized)?;
        Ok(dataset.read_raw_bytes_into(dst)?)
    }

    /// Read a variable as logical raw bytes with numeric fields in native endian.
    pub fn read_variable_native_bytes(&self, path: &str) -> Result<Vec<u8>> {
        let normalized = normalize_dataset_path(path)?;
        let dataset = self.hdf5.dataset(normalized)?;
        Ok(dataset.read_native_bytes()?)
    }

    /// Read native-endian logical raw bytes into a caller-provided buffer.
    pub fn read_variable_native_bytes_into(&self, path: &str, dst: &mut [u8]) -> Result<()> {
        let normalized = normalize_dataset_path(path)?;
        let dataset = self.hdf5.dataset(normalized)?;
        Ok(dataset.read_native_bytes_into(dst)?)
    }

    /// Iterate decoded HDF5 chunks for a chunked variable.
    pub fn iter_variable_chunks(&self, path: &str) -> Result<hdf5_reader::DatasetChunkIterator> {
        let normalized = normalize_dataset_path(path)?;
        let dataset = self.hdf5.dataset(normalized)?;
        Ok(dataset.iter_chunks()?)
    }

    /// Return current chunk-cache statistics.
    pub fn chunk_cache_stats(&self) -> hdf5_reader::ChunkCacheStats {
        self.hdf5.chunk_cache_stats()
    }

    /// Read a string variable as a single string.
    pub fn read_variable_as_string(&self, path: &str) -> Result<String> {
        let mut strings = self.read_variable_as_strings(path)?;
        match strings.len() {
            1 => Ok(strings.swap_remove(0)),
            0 => Err(Error::InvalidData(format!(
                "variable '{}' contains no string elements",
                path
            ))),
            count => Err(Error::InvalidData(format!(
                "variable '{}' contains {count} string elements; use read_variable_as_strings()",
                path
            ))),
        }
    }

    /// Read a string variable as a flat vector of strings.
    pub fn read_variable_as_strings(&self, path: &str) -> Result<Vec<String>> {
        let normalized = normalize_dataset_path(path)?;
        let dataset = self.hdf5.dataset(normalized)?;
        let dtype = dataset_nc_type(&dataset)?;
        match dtype {
            NcType::String => Ok(dataset.read_strings()?),
            NcType::Char => {
                let variable = self.variable(path)?;
                let raw = dataset.read_raw_bytes()?;
                decode_char_variable_strings(variable, &raw)
            }
            _ => Err(Error::TypeMismatch {
                expected: "String or Char".to_string(),
                actual: format!("{dtype:?}"),
            }),
        }
    }

    /// Read a variable containing NetCDF-4 user-defined values.
    pub fn read_variable_user_defined(
        &self,
        path: &str,
    ) -> Result<ArrayD<crate::user_defined::NcValue>> {
        let normalized = normalize_dataset_path(path)?;
        let dataset = self.hdf5.dataset(normalized)?;
        crate::user_defined::read_dataset_values(&dataset)
    }

    /// Read a NetCDF-4 user-defined variable through a custom decoder.
    pub fn read_variable_user_defined_with<T, F>(&self, path: &str, decoder: F) -> Result<ArrayD<T>>
    where
        F: FnMut(crate::user_defined::NcValueView<'_>) -> Result<T>,
    {
        let normalized = normalize_dataset_path(path)?;
        let dataset = self.hdf5.dataset(normalized)?;
        crate::user_defined::read_dataset_with_decoder(&dataset, decoder)
    }

    #[cfg(feature = "rayon")]
    pub fn read_variable_parallel<T: H5Type>(&self, path: &str) -> Result<ArrayD<T>> {
        let normalized = normalize_dataset_path(path)?;
        let dataset = self.hdf5.dataset(normalized)?;
        Ok(dataset.read_array_parallel::<T>()?)
    }

    #[cfg(feature = "rayon")]
    pub fn read_variable_in_pool<T: H5Type>(
        &self,
        path: &str,
        pool: &ThreadPool,
    ) -> Result<ArrayD<T>> {
        let normalized = normalize_dataset_path(path)?;
        let dataset = self.hdf5.dataset(normalized)?;
        Ok(dataset.read_array_in_pool::<T>(pool)?)
    }
}

fn decode_char_variable_strings(
    variable: &crate::types::NcVariable,
    bytes: &[u8],
) -> Result<Vec<String>> {
    let shape = variable.shape();
    if shape.len() <= 1 {
        return Ok(vec![decode_char_string(bytes)]);
    }

    let string_len = checked_usize_from_u64(
        *shape
            .last()
            .ok_or_else(|| Error::InvalidData("char variable missing string axis".into()))?,
        "char string length",
    )?;
    let string_count_u64 = checked_shape_elements(&shape[..shape.len() - 1], "char string count")?;
    let string_count = checked_usize_from_u64(string_count_u64, "char string count")?;
    let expected_bytes = string_count.checked_mul(string_len).ok_or_else(|| {
        Error::InvalidData("char string byte count exceeds platform usize".to_string())
    })?;

    if bytes.len() < expected_bytes {
        return Err(Error::InvalidData(format!(
            "char variable '{}' data too short: need {} bytes, have {}",
            variable.name,
            expected_bytes,
            bytes.len()
        )));
    }

    if string_len == 0 {
        return Ok(vec![String::new(); string_count]);
    }

    Ok(bytes[..expected_bytes]
        .chunks_exact(string_len)
        .map(decode_char_string)
        .collect())
}

fn decode_char_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches('\0')
        .to_string()
}

impl Nc4File {
    fn root_metadata(&self) -> Result<&NcGroup> {
        if let Some(group) = self.root_metadata.get() {
            return Ok(group);
        }
        let root_metadata = groups::build_root_group_metadata(&self.hdf5, self.metadata_mode)?;
        let _ = self.root_metadata.set(root_metadata);
        Ok(self
            .root_metadata
            .get()
            .expect("root metadata must be initialized after successful build"))
    }
}

impl Nc4File {
    /// Read a variable with automatic type promotion to f64.
    ///
    /// Reads in the native HDF5 type and promotes to f64 via `mapv`.
    pub fn read_variable_as_f64(&self, path: &str) -> Result<ArrayD<f64>> {
        let normalized = normalize_dataset_path(path)?;
        let dataset = self.hdf5.dataset(normalized)?;
        let dtype = dataset_nc_type(&dataset)?;
        dispatch_read_as_f64!(&dtype, |T| dataset.read_array::<T>())
    }

    /// Read a slice of a variable with automatic type promotion to f64.
    pub fn read_variable_slice_as_f64(
        &self,
        path: &str,
        selection: &crate::types::NcSliceInfo,
    ) -> Result<ArrayD<f64>> {
        let normalized = normalize_dataset_path(path)?;
        let dataset = self.hdf5.dataset(normalized)?;
        let hdf5_sel = to_hdf5_slice_info(selection);
        let dtype = dataset_nc_type(&dataset)?;
        dispatch_read_as_f64!(&dtype, |T| dataset.read_slice::<T>(&hdf5_sel))
    }

    /// Read a typed slice of a variable (NC4 delegation).
    pub fn read_variable_slice<T: H5Type>(
        &self,
        path: &str,
        selection: &crate::types::NcSliceInfo,
    ) -> Result<ArrayD<T>> {
        let normalized = normalize_dataset_path(path)?;
        let dataset = self.hdf5.dataset(normalized)?;
        let hdf5_sel = to_hdf5_slice_info(selection);
        Ok(dataset.read_slice::<T>(&hdf5_sel)?)
    }

    /// Read a typed slice of a variable using chunk-level parallelism.
    ///
    /// Chunked datasets decompress overlapping chunks in parallel via Rayon.
    /// Non-chunked layouts fall back to `read_variable_slice`.
    #[cfg(feature = "rayon")]
    pub fn read_variable_slice_parallel<T: H5Type>(
        &self,
        path: &str,
        selection: &crate::types::NcSliceInfo,
    ) -> Result<ArrayD<T>> {
        let normalized = normalize_dataset_path(path)?;
        let dataset = self.hdf5.dataset(normalized)?;
        let hdf5_sel = to_hdf5_slice_info(selection);
        Ok(dataset.read_slice_parallel::<T>(&hdf5_sel)?)
    }
}

fn to_hdf5_slice_info(selection: &crate::types::NcSliceInfo) -> hdf5_reader::SliceInfo {
    hdf5_reader::SliceInfo {
        selections: selection
            .selections
            .iter()
            .map(|s| match s {
                crate::types::NcSliceInfoElem::Index(idx) => {
                    hdf5_reader::SliceInfoElem::Index(*idx)
                }
                crate::types::NcSliceInfoElem::Slice { start, end, step } => {
                    hdf5_reader::SliceInfoElem::Slice {
                        start: *start,
                        end: *end,
                        step: *step,
                    }
                }
            })
            .collect(),
    }
}

fn normalize_dataset_path(path: &str) -> Result<&str> {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return Err(Error::VariableNotFound(path.to_string()));
    }
    Ok(trimmed)
}

fn normalize_group_path(path: &str) -> Result<&str> {
    Ok(path.trim_matches('/'))
}

fn dataset_nc_type(dataset: &hdf5_reader::Dataset) -> Result<NcType> {
    self::types::hdf5_to_nc_type(dataset.dtype()).map_err(|err| {
        Error::InvalidData(format!(
            "dataset '{}' cannot be mapped to a NetCDF-4 type: {err}",
            dataset.name()
        ))
    })
}
