//! Pure-Rust NetCDF file reader.
//!
//! Supports:
//! - **CDF-1** (classic): `CDF\x01` magic
//! - **CDF-2** (64-bit offset): `CDF\x02` magic
//! - **CDF-5** (64-bit data): `CDF\x05` magic
//! - **NetCDF-4** (HDF5-backed): `\x89HDF\r\n\x1a\n` magic (requires `netcdf4` feature)
//!
//! PnetCDF-produced CDF-1/2/5 files are supported as files. NetCDF-C files
//! created through parallel NetCDF-4/HDF5 APIs are supported when the final
//! HDF5 file uses supported HDF5 features. This crate does not implement
//! MPI communicators, `nc_open_par`, `nc_create_par`, collective/independent
//! access modes, PnetCDF subfiling, or write APIs.
//!
//! # Example
//!
//! ```no_run
//! use netcdf_reader::NcFile;
//!
//! let file = NcFile::open("example.nc").unwrap();
//! println!("format: {:?}", file.format());
//! for var in file.variables().unwrap() {
//!     println!("  variable: {} shape={:?}", var.name(), var.shape());
//! }
//! ```

pub mod classic;
pub mod error;
pub mod masked;
pub mod types;
pub mod unpack;

#[cfg(feature = "netcdf4")]
pub mod nc4;
#[cfg(feature = "netcdf4")]
pub mod user_defined;

#[cfg(feature = "cf")]
pub mod cf;

pub use error::{Error, Result};
#[cfg(feature = "netcdf4")]
pub use hdf5_reader::storage::DynStorage;
#[cfg(feature = "netcdf4")]
pub use hdf5_reader::{
    BlockCacheStats, BlockCacheStorage, BytesStorage, ChunkCacheStats, DatasetChunk,
    DatasetChunkIterator, ExternalFileResolver, ExternalLinkResolver, FileStorage,
    FilesystemExternalFileResolver, FilesystemExternalLinkResolver, MmapStorage,
    RangeRequestStorage, Storage, StorageBuffer,
};
pub use types::*;
#[cfg(feature = "netcdf4")]
pub use user_defined::{
    NcArrayValue, NcCompoundFieldView, NcCompoundValueField, NcEnumValue, NcValue, NcValueView,
};

use std::fs::File;
use std::io::Read;
use std::path::Path;
#[cfg(feature = "netcdf4")]
use std::sync::Arc;

use memmap2::Mmap;
use ndarray::ArrayD;
#[cfg(feature = "rayon")]
use rayon::ThreadPool;

/// Trait alias for types readable from both classic and NetCDF-4 files.
///
/// This unifies `classic::data::NcReadType` (for CDF-1/2/5) and
/// `hdf5_reader::H5Type` (for NetCDF-4/HDF5) so that `NcFile::read_variable`
/// works across all formats with a single type parameter.
#[cfg(feature = "netcdf4")]
pub trait NcReadable: classic::data::NcReadType + hdf5_reader::H5Type {}
#[cfg(feature = "netcdf4")]
impl<T: classic::data::NcReadType + hdf5_reader::H5Type> NcReadable for T {}

#[cfg(not(feature = "netcdf4"))]
pub trait NcReadable: classic::data::NcReadType {}
#[cfg(not(feature = "netcdf4"))]
impl<T: classic::data::NcReadType> NcReadable for T {}

/// An opened NetCDF file.
pub struct NcFile {
    format: NcFormat,
    inner: NcFileInner,
}

enum NcFileInner {
    Classic(classic::ClassicFile),
    #[cfg(feature = "netcdf4")]
    Nc4(Box<nc4::Nc4File>),
}

/// HDF5 magic bytes: `\x89HDF\r\n\x1a\n`
const HDF5_MAGIC: [u8; 8] = [0x89, b'H', b'D', b'F', 0x0D, 0x0A, 0x1A, 0x0A];

/// Detect the NetCDF format from the first bytes of a file.
fn detect_format(data: &[u8]) -> Result<NcFormat> {
    if data.len() < 4 {
        return Err(Error::InvalidMagic);
    }

    // Check for CDF magic: "CDF" followed by version byte.
    if data[0] == b'C' && data[1] == b'D' && data[2] == b'F' {
        return match data[3] {
            1 => Ok(NcFormat::Classic),
            2 => Ok(NcFormat::Offset64),
            5 => Ok(NcFormat::Cdf5),
            v => Err(Error::UnsupportedVersion(v)),
        };
    }

    // Check for HDF5 magic (8 bytes).
    if data.len() >= 8 && data[..8] == HDF5_MAGIC {
        return Ok(NcFormat::Nc4);
    }

    Err(Error::InvalidMagic)
}

fn read_magic_prefix(reader: &mut impl Read) -> std::io::Result<([u8; 8], usize)> {
    let mut magic = [0u8; 8];
    let mut read_len = 0;
    while read_len < magic.len() {
        let n = reader.read(&mut magic[read_len..])?;
        if n == 0 {
            break;
        }
        read_len += n;
    }
    Ok((magic, read_len))
}

#[cfg(feature = "cf")]
fn parent_group_path(path: &str) -> &str {
    let trimmed = path.trim_matches('/');
    trimmed
        .rsplit_once('/')
        .map(|(group_path, _)| group_path)
        .unwrap_or("")
}

impl NcFile {
    /// Open a NetCDF file from a path.
    ///
    /// The format is auto-detected from the file's magic bytes.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_options(path, NcOpenOptions::default())
    }

    /// Open a NetCDF file from in-memory bytes.
    ///
    /// The format is auto-detected from the magic bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        Self::from_bytes_with_options(data, NcOpenOptions::default())
    }

    /// Open a NetCDF file from a custom random-access storage backend.
    ///
    /// Both NetCDF-4 and classic CDF-1/2/5 files stay range-backed.
    #[cfg(feature = "netcdf4")]
    pub fn from_storage(storage: DynStorage) -> Result<Self> {
        Self::from_storage_with_options(storage, NcOpenOptions::default())
    }

    /// Open a NetCDF file from a custom random-access storage backend with custom options.
    #[cfg(feature = "netcdf4")]
    pub fn from_storage_with_options(storage: DynStorage, options: NcOpenOptions) -> Result<Self> {
        let magic_len = storage.len().min(HDF5_MAGIC.len() as u64) as usize;
        let magic = storage.read_range(0, magic_len)?;
        let format = detect_format(magic.as_ref())?;

        match format {
            NcFormat::Classic | NcFormat::Offset64 | NcFormat::Cdf5 => {
                let classic = classic::ClassicFile::from_storage(storage, format)?;
                Ok(NcFile {
                    format,
                    inner: NcFileInner::Classic(classic),
                })
            }
            NcFormat::Nc4 | NcFormat::Nc4Classic => {
                let nc4 = nc4::Nc4File::from_storage_with_options(storage, options)?;
                let actual_format = if nc4.is_classic_model() {
                    NcFormat::Nc4Classic
                } else {
                    NcFormat::Nc4
                };
                Ok(NcFile {
                    format: actual_format,
                    inner: NcFileInner::Nc4(Box::new(nc4)),
                })
            }
        }
    }

    /// Open a NetCDF file from in-memory bytes with custom options.
    ///
    /// NC4 options are applied when the payload is HDF5-backed.
    pub fn from_bytes_with_options(data: &[u8], options: NcOpenOptions) -> Result<Self> {
        let format = detect_format(data)?;

        match format {
            NcFormat::Classic | NcFormat::Offset64 | NcFormat::Cdf5 => {
                let classic = classic::ClassicFile::from_bytes(data, format)?;
                Ok(NcFile {
                    format,
                    inner: NcFileInner::Classic(classic),
                })
            }
            NcFormat::Nc4 | NcFormat::Nc4Classic => {
                #[cfg(feature = "netcdf4")]
                {
                    let nc4 = nc4::Nc4File::from_bytes_with_options(data, options)?;
                    let actual_format = if nc4.is_classic_model() {
                        NcFormat::Nc4Classic
                    } else {
                        NcFormat::Nc4
                    };
                    Ok(NcFile {
                        format: actual_format,
                        inner: NcFileInner::Nc4(Box::new(nc4)),
                    })
                }
                #[cfg(not(feature = "netcdf4"))]
                {
                    let _ = options;
                    Err(Error::Nc4NotEnabled)
                }
            }
        }
    }

    /// The detected file format.
    pub fn format(&self) -> NcFormat {
        self.format
    }

    /// The root group of the file.
    ///
    /// Classic files have a single implicit root group containing all
    /// dimensions, variables, and global attributes. NetCDF-4 files
    /// can have nested sub-groups.
    pub fn root_group(&self) -> Result<&NcGroup> {
        match &self.inner {
            NcFileInner::Classic(c) => Ok(c.root_group()),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => n.root_group(),
        }
    }

    /// Convenience: dimensions in the root group.
    pub fn dimensions(&self) -> Result<&[NcDimension]> {
        match &self.inner {
            NcFileInner::Classic(c) => Ok(&c.root_group().dimensions),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => n.dimensions(),
        }
    }

    /// Convenience: variables in the root group.
    pub fn variables(&self) -> Result<&[NcVariable]> {
        match &self.inner {
            NcFileInner::Classic(c) => Ok(&c.root_group().variables),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => n.variables(),
        }
    }

    /// Convenience: global attributes (attributes of the root group).
    pub fn global_attributes(&self) -> Result<&[NcAttribute]> {
        match &self.inner {
            NcFileInner::Classic(c) => Ok(&c.root_group().attributes),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => n.global_attributes(),
        }
    }

    /// Find a group by path relative to the root group.
    pub fn group(&self, path: &str) -> Result<&NcGroup> {
        match &self.inner {
            NcFileInner::Classic(c) => c
                .root_group()
                .group(path)
                .ok_or_else(|| Error::GroupNotFound(path.to_string())),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => n.group(path),
        }
    }

    /// Find a variable by name or path relative to the root group.
    pub fn variable(&self, name: &str) -> Result<&NcVariable> {
        match &self.inner {
            NcFileInner::Classic(c) => c
                .root_group()
                .variable(name)
                .ok_or_else(|| Error::VariableNotFound(name.to_string())),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => n.variable(name),
        }
    }

    /// Find a dimension by name or path relative to the root group.
    pub fn dimension(&self, name: &str) -> Result<&NcDimension> {
        match &self.inner {
            NcFileInner::Classic(c) => c
                .root_group()
                .dimension(name)
                .ok_or_else(|| Error::DimensionNotFound(name.to_string())),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => n.dimension(name),
        }
    }

    /// Find the coordinate variable for a dimension name or path.
    pub fn coordinate_variable(&self, name: &str) -> Result<&NcVariable> {
        self.root_group()?
            .coordinate_variable(name)
            .ok_or_else(|| Error::VariableNotFound(format!("coordinate variable for {name}")))
    }

    /// Discover CF axes from coordinate variables in a group.
    #[cfg(feature = "cf")]
    pub fn cf_coordinate_axes(&self, group_path: &str) -> Result<Vec<cf::CfCoordinateAxis<'_>>> {
        let group = self.group(group_path)?;
        Ok(cf::discover_coordinate_axes(group))
    }

    /// Discover CF axes used by a variable from its coordinate variables.
    #[cfg(feature = "cf")]
    pub fn cf_variable_axes(&self, name: &str) -> Result<Vec<cf::CfCoordinateAxis<'_>>> {
        let variable = self.variable(name)?;
        let group = self.group(parent_group_path(name))?;
        Ok(cf::discover_variable_axes(variable, group))
    }

    /// Discover CF time coordinate variables in a group.
    #[cfg(feature = "cf")]
    pub fn cf_time_coordinates(&self, group_path: &str) -> Result<Vec<cf::CfTimeCoordinate<'_>>> {
        let group = self.group(group_path)?;
        cf::discover_time_coordinates(group)
    }

    /// Discover the CF time coordinate used by a variable, if one exists.
    #[cfg(feature = "cf")]
    pub fn cf_variable_time_coordinate(
        &self,
        name: &str,
    ) -> Result<Option<cf::CfTimeCoordinate<'_>>> {
        let variable = self.variable(name)?;
        let group = self.group(parent_group_path(name))?;
        cf::discover_variable_time_coordinate(variable, group)
    }

    /// Find a group attribute by name or path relative to the root group.
    pub fn global_attribute(&self, name: &str) -> Result<&NcAttribute> {
        match &self.inner {
            NcFileInner::Classic(c) => c
                .root_group()
                .attribute(name)
                .ok_or_else(|| Error::AttributeNotFound(name.to_string())),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => n.global_attribute(name),
        }
    }

    /// Read a variable's data as a typed array.
    ///
    /// Works for both classic (CDF-1/2/5) and NetCDF-4 files. NetCDF-4 nested
    /// variables can be addressed with paths like `group/subgroup/var`. The type
    /// parameter `T` must implement `NcReadable`, which is satisfied by:
    /// `i8, u8, i16, u16, i32, u32, i64, u64, f32, f64`.
    pub fn read_variable<T: NcReadable>(&self, name: &str) -> Result<ArrayD<T>> {
        match &self.inner {
            NcFileInner::Classic(c) => c.read_variable::<T>(name),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => Ok(n.read_variable::<T>(name)?),
        }
    }

    /// Read a variable into a caller-provided typed buffer.
    pub fn read_variable_into<T: NcReadable>(&self, name: &str, dst: &mut [T]) -> Result<()> {
        match &self.inner {
            NcFileInner::Classic(c) => c.read_variable_into::<T>(name, dst),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => Ok(n.read_variable_into::<T>(name, dst)?),
        }
    }

    /// Read a NetCDF-4 variable as logical raw bytes in HDF5 datatype byte order.
    #[cfg(feature = "netcdf4")]
    pub fn read_variable_raw_bytes(&self, name: &str) -> Result<Vec<u8>> {
        match &self.inner {
            NcFileInner::Classic(_) => Err(Error::TypeMismatch {
                expected: "NetCDF-4 variable".to_string(),
                actual: "classic NetCDF variable".to_string(),
            }),
            NcFileInner::Nc4(n) => Ok(n.read_variable_raw_bytes(name)?),
        }
    }

    /// Read a NetCDF-4 variable as logical raw bytes into a caller-provided buffer.
    #[cfg(feature = "netcdf4")]
    pub fn read_variable_raw_bytes_into(&self, name: &str, dst: &mut [u8]) -> Result<()> {
        match &self.inner {
            NcFileInner::Classic(_) => Err(Error::TypeMismatch {
                expected: "NetCDF-4 variable".to_string(),
                actual: "classic NetCDF variable".to_string(),
            }),
            NcFileInner::Nc4(n) => Ok(n.read_variable_raw_bytes_into(name, dst)?),
        }
    }

    /// Read a NetCDF-4 variable as logical raw bytes with numeric fields in native endian.
    #[cfg(feature = "netcdf4")]
    pub fn read_variable_native_bytes(&self, name: &str) -> Result<Vec<u8>> {
        match &self.inner {
            NcFileInner::Classic(_) => Err(Error::TypeMismatch {
                expected: "NetCDF-4 variable".to_string(),
                actual: "classic NetCDF variable".to_string(),
            }),
            NcFileInner::Nc4(n) => Ok(n.read_variable_native_bytes(name)?),
        }
    }

    /// Read native-endian logical raw bytes for a NetCDF-4 variable into a buffer.
    #[cfg(feature = "netcdf4")]
    pub fn read_variable_native_bytes_into(&self, name: &str, dst: &mut [u8]) -> Result<()> {
        match &self.inner {
            NcFileInner::Classic(_) => Err(Error::TypeMismatch {
                expected: "NetCDF-4 variable".to_string(),
                actual: "classic NetCDF variable".to_string(),
            }),
            NcFileInner::Nc4(n) => Ok(n.read_variable_native_bytes_into(name, dst)?),
        }
    }

    /// Iterate decoded HDF5 chunks for a NetCDF-4 variable.
    #[cfg(feature = "netcdf4")]
    pub fn iter_variable_chunks(&self, name: &str) -> Result<DatasetChunkIterator> {
        match &self.inner {
            NcFileInner::Classic(_) => Err(Error::TypeMismatch {
                expected: "NetCDF-4 chunked variable".to_string(),
                actual: "classic NetCDF variable".to_string(),
            }),
            NcFileInner::Nc4(n) => Ok(n.iter_variable_chunks(name)?),
        }
    }

    /// Return chunk-cache statistics for NetCDF-4 files.
    #[cfg(feature = "netcdf4")]
    pub fn chunk_cache_stats(&self) -> Option<ChunkCacheStats> {
        match &self.inner {
            NcFileInner::Classic(_) => None,
            NcFileInner::Nc4(n) => Some(n.chunk_cache_stats()),
        }
    }

    /// Read a variable using internal chunk-level parallelism when available.
    ///
    /// Classic formats fall back to `read_variable`.
    #[cfg(feature = "rayon")]
    pub fn read_variable_parallel<T: NcReadable>(&self, name: &str) -> Result<ArrayD<T>> {
        match &self.inner {
            NcFileInner::Classic(c) => c.read_variable_parallel::<T>(name),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => Ok(n.read_variable_parallel::<T>(name)?),
        }
    }

    /// Read a variable using the provided Rayon thread pool when available.
    ///
    /// Classic formats fall back to `read_variable`.
    #[cfg(feature = "rayon")]
    pub fn read_variable_in_pool<T: NcReadable>(
        &self,
        name: &str,
        pool: &ThreadPool,
    ) -> Result<ArrayD<T>> {
        match &self.inner {
            NcFileInner::Classic(c) => pool.install(|| c.read_variable_parallel::<T>(name)),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => Ok(n.read_variable_in_pool::<T>(name, pool)?),
        }
    }

    /// Access the underlying classic file (for reading data).
    ///
    /// Returns `None` if this is a NetCDF-4 file.
    pub fn as_classic(&self) -> Option<&classic::ClassicFile> {
        match &self.inner {
            NcFileInner::Classic(c) => Some(c),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(_) => None,
        }
    }

    /// Read a variable with automatic type promotion to f64.
    ///
    /// Reads in the native storage type (i8, i16, i32, f32, f64, u8, etc.)
    /// and promotes all values to f64. This avoids the `TypeMismatch` error
    /// that `read_variable::<f64>` produces for non-f64 variables.
    pub fn read_variable_as_f64(&self, name: &str) -> Result<ArrayD<f64>> {
        match &self.inner {
            NcFileInner::Classic(c) => c.read_variable_as_f64(name),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => n.read_variable_as_f64(name),
        }
    }

    /// Read a string variable as a single string.
    ///
    /// Use [`NcFile::read_variable_as_strings`] when the variable contains
    /// multiple string elements.
    pub fn read_variable_as_string(&self, name: &str) -> Result<String> {
        match &self.inner {
            NcFileInner::Classic(c) => c.read_variable_as_string(name),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => n.read_variable_as_string(name),
        }
    }

    /// Read a string or char variable as a flat vector of strings.
    ///
    /// Classic char arrays interpret the last dimension as the string length
    /// and flatten the leading dimensions.
    pub fn read_variable_as_strings(&self, name: &str) -> Result<Vec<String>> {
        match &self.inner {
            NcFileInner::Classic(c) => c.read_variable_as_strings(name),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => n.read_variable_as_strings(name),
        }
    }

    /// Read a NetCDF-4 user-defined variable into dynamic values.
    ///
    /// This supports enum, opaque, compound, fixed-size array, and non-string
    /// vlen datatypes. Primitive values nested inside those types are decoded
    /// according to the HDF5 datatype byte order.
    #[cfg(feature = "netcdf4")]
    pub fn read_variable_user_defined(&self, name: &str) -> Result<ArrayD<NcValue>> {
        match &self.inner {
            NcFileInner::Classic(_) => Err(Error::TypeMismatch {
                expected: "NetCDF-4 user-defined variable".to_string(),
                actual: "classic NetCDF variable".to_string(),
            }),
            NcFileInner::Nc4(n) => n.read_variable_user_defined(name),
        }
    }

    /// Read a NetCDF-4 user-defined variable through a caller-provided decoder.
    ///
    /// The decoder receives one [`NcValueView`] per logical variable element,
    /// allowing direct construction of application structs without allocating
    /// an intermediate [`NcValue`] tree.
    #[cfg(feature = "netcdf4")]
    pub fn read_variable_user_defined_with<T, F>(&self, name: &str, decoder: F) -> Result<ArrayD<T>>
    where
        F: FnMut(NcValueView<'_>) -> Result<T>,
    {
        match &self.inner {
            NcFileInner::Classic(_) => Err(Error::TypeMismatch {
                expected: "NetCDF-4 user-defined variable".to_string(),
                actual: "classic NetCDF variable".to_string(),
            }),
            NcFileInner::Nc4(n) => n.read_variable_user_defined_with(name, decoder),
        }
    }

    /// Read a variable and apply `scale_factor`/`add_offset` unpacking.
    ///
    /// Returns `actual = stored * scale_factor + add_offset`.
    /// If neither attribute is present, returns the raw data as f64.
    /// Uses type-promoting read so it works with any numeric storage type.
    pub fn read_variable_unpacked(&self, name: &str) -> Result<ArrayD<f64>> {
        let var = self.variable(name)?;
        let params = unpack::UnpackParams::from_variable(var);
        let mut data = self.read_variable_as_f64(name)?;
        if let Some(p) = params {
            p.apply(&mut data);
        }
        Ok(data)
    }

    /// Read a variable, replace `_FillValue`/`missing_value` with NaN,
    /// and mask values outside `valid_min`/`valid_max`/`valid_range`.
    /// Uses type-promoting read so it works with any numeric storage type.
    pub fn read_variable_masked(&self, name: &str) -> Result<ArrayD<f64>> {
        let var = self.variable(name)?;
        let params = masked::MaskParams::from_variable(var);
        let mut data = self.read_variable_as_f64(name)?;
        if let Some(p) = params {
            p.apply(&mut data);
        }
        Ok(data)
    }

    /// Read a variable with both masking and unpacking (CF spec order).
    ///
    /// Order: read → mask fill/missing → unpack (scale+offset).
    /// Uses type-promoting read so it works with any numeric storage type.
    pub fn read_variable_unpacked_masked(&self, name: &str) -> Result<ArrayD<f64>> {
        let var = self.variable(name)?;
        let mask_params = masked::MaskParams::from_variable(var);
        let unpack_params = unpack::UnpackParams::from_variable(var);
        let mut data = self.read_variable_as_f64(name)?;
        if let Some(p) = mask_params {
            p.apply(&mut data);
        }
        if let Some(p) = unpack_params {
            p.apply(&mut data);
        }
        Ok(data)
    }

    // ----- Slice API -----

    /// Read a slice (hyperslab) of a variable as a typed array.
    pub fn read_variable_slice<T: NcReadable>(
        &self,
        name: &str,
        selection: &NcSliceInfo,
    ) -> Result<ArrayD<T>> {
        match &self.inner {
            NcFileInner::Classic(c) => c.read_variable_slice::<T>(name, selection),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => Ok(n.read_variable_slice::<T>(name, selection)?),
        }
    }

    /// Read a slice (hyperslab) using chunk-level parallelism when available.
    ///
    /// For NetCDF-4 chunked datasets, overlapping chunks are decompressed in
    /// parallel via Rayon. Classic formats fall back to `read_variable_slice`.
    #[cfg(feature = "rayon")]
    pub fn read_variable_slice_parallel<T: NcReadable>(
        &self,
        name: &str,
        selection: &NcSliceInfo,
    ) -> Result<ArrayD<T>> {
        match &self.inner {
            NcFileInner::Classic(c) => c.read_variable_slice_parallel::<T>(name, selection),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => Ok(n.read_variable_slice_parallel::<T>(name, selection)?),
        }
    }

    /// Read a slice of a variable with automatic type promotion to f64.
    pub fn read_variable_slice_as_f64(
        &self,
        name: &str,
        selection: &NcSliceInfo,
    ) -> Result<ArrayD<f64>> {
        match &self.inner {
            NcFileInner::Classic(c) => c.read_variable_slice_as_f64(name, selection),
            #[cfg(feature = "netcdf4")]
            NcFileInner::Nc4(n) => n.read_variable_slice_as_f64(name, selection),
        }
    }

    /// Read a slice with `scale_factor`/`add_offset` unpacking.
    pub fn read_variable_slice_unpacked(
        &self,
        name: &str,
        selection: &NcSliceInfo,
    ) -> Result<ArrayD<f64>> {
        let var = self.variable(name)?;
        let params = unpack::UnpackParams::from_variable(var);
        let mut data = self.read_variable_slice_as_f64(name, selection)?;
        if let Some(p) = params {
            p.apply(&mut data);
        }
        Ok(data)
    }

    /// Read a slice with fill/missing value masking.
    pub fn read_variable_slice_masked(
        &self,
        name: &str,
        selection: &NcSliceInfo,
    ) -> Result<ArrayD<f64>> {
        let var = self.variable(name)?;
        let params = masked::MaskParams::from_variable(var);
        let mut data = self.read_variable_slice_as_f64(name, selection)?;
        if let Some(p) = params {
            p.apply(&mut data);
        }
        Ok(data)
    }

    /// Read a slice with both masking and unpacking (CF spec order).
    pub fn read_variable_slice_unpacked_masked(
        &self,
        name: &str,
        selection: &NcSliceInfo,
    ) -> Result<ArrayD<f64>> {
        let var = self.variable(name)?;
        let mask_params = masked::MaskParams::from_variable(var);
        let unpack_params = unpack::UnpackParams::from_variable(var);
        let mut data = self.read_variable_slice_as_f64(name, selection)?;
        if let Some(p) = mask_params {
            p.apply(&mut data);
        }
        if let Some(p) = unpack_params {
            p.apply(&mut data);
        }
        Ok(data)
    }

    // ----- Lazy Slice Iterator -----

    /// Create an iterator that yields one slice per index along a given dimension.
    ///
    /// Each call to `next()` reads one slice using the slice API. This is
    /// useful for iterating time steps, levels, etc. without loading the
    /// entire dataset into memory.
    pub fn iter_slices<T: NcReadable>(
        &self,
        name: &str,
        dim: usize,
    ) -> Result<NcSliceIterator<'_, T>> {
        let var = self.variable(name)?;
        let ndim = var.ndim();
        if dim >= ndim {
            return Err(Error::InvalidData(format!(
                "dimension index {} out of range for {}-dimensional variable '{}'",
                dim, ndim, name
            )));
        }
        let dim_size = var.dimensions[dim].size;
        Ok(NcSliceIterator {
            file: self,
            name: name.to_string(),
            dim,
            dim_size,
            current: 0,
            ndim,
            _marker: std::marker::PhantomData,
        })
    }
}

/// Configuration options for opening a NetCDF file.
pub struct NcOpenOptions {
    /// Maximum bytes for the chunk cache (NC4 only). Default: 64 MiB.
    pub chunk_cache_bytes: usize,
    /// Maximum number of chunk cache slots (NC4 only). Default: 521.
    pub chunk_cache_slots: usize,
    /// NetCDF-4 metadata reconstruction policy. Default: strict.
    pub metadata_mode: NcMetadataMode,
    /// Custom filter registry (NC4 only).
    #[cfg(feature = "netcdf4")]
    pub filter_registry: Option<hdf5_reader::FilterRegistry>,
    /// Resolver for HDF5 external raw data files (NC4 only). If `None`,
    /// external raw data files are not resolved.
    #[cfg(feature = "netcdf4")]
    pub external_file_resolver: Option<Arc<dyn hdf5_reader::ExternalFileResolver>>,
    /// Resolver for HDF5 external links (NC4 only).
    #[cfg(feature = "netcdf4")]
    pub external_link_resolver: Option<Arc<dyn hdf5_reader::ExternalLinkResolver>>,
}

impl Default for NcOpenOptions {
    fn default() -> Self {
        NcOpenOptions {
            chunk_cache_bytes: 64 * 1024 * 1024,
            chunk_cache_slots: 521,
            metadata_mode: NcMetadataMode::Strict,
            #[cfg(feature = "netcdf4")]
            filter_registry: None,
            #[cfg(feature = "netcdf4")]
            external_file_resolver: None,
            #[cfg(feature = "netcdf4")]
            external_link_resolver: None,
        }
    }
}

impl NcFile {
    /// Open a NetCDF file with custom options.
    pub fn open_with_options(path: impl AsRef<Path>, options: NcOpenOptions) -> Result<Self> {
        let path = path.as_ref();
        let mut file = File::open(path)?;
        let (magic, n) = read_magic_prefix(&mut file)?;
        let format = detect_format(&magic[..n])?;

        match format {
            NcFormat::Classic | NcFormat::Offset64 | NcFormat::Cdf5 => {
                let file = File::open(path)?;
                // SAFETY: read-only mapping; caller must not modify the file concurrently.
                let mmap = unsafe { Mmap::map(&file)? };
                let classic = classic::ClassicFile::from_mmap(mmap, format)?;
                Ok(NcFile {
                    format,
                    inner: NcFileInner::Classic(classic),
                })
            }
            NcFormat::Nc4 | NcFormat::Nc4Classic => {
                #[cfg(feature = "netcdf4")]
                {
                    let metadata_mode = options.metadata_mode;
                    let hdf5 = hdf5_reader::Hdf5File::open_with_options(
                        path,
                        hdf5_reader::OpenOptions {
                            chunk_cache_bytes: options.chunk_cache_bytes,
                            chunk_cache_slots: options.chunk_cache_slots,
                            filter_registry: options.filter_registry,
                            external_file_resolver: options.external_file_resolver,
                            external_link_resolver: options.external_link_resolver,
                        },
                    )?;
                    let nc4 = nc4::Nc4File::from_hdf5(hdf5, metadata_mode)?;
                    let actual_format = if nc4.is_classic_model() {
                        NcFormat::Nc4Classic
                    } else {
                        NcFormat::Nc4
                    };
                    Ok(NcFile {
                        format: actual_format,
                        inner: NcFileInner::Nc4(Box::new(nc4)),
                    })
                }
                #[cfg(not(feature = "netcdf4"))]
                {
                    let _ = options;
                    Err(Error::Nc4NotEnabled)
                }
            }
        }
    }
}

/// Lazy iterator over slices of a variable along a given dimension.
pub struct NcSliceIterator<'f, T: NcReadable> {
    file: &'f NcFile,
    name: String,
    dim: usize,
    dim_size: u64,
    current: u64,
    ndim: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<'f, T: NcReadable> Iterator for NcSliceIterator<'f, T> {
    type Item = Result<ArrayD<T>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.dim_size {
            return None;
        }
        let mut selections = Vec::with_capacity(self.ndim);
        for d in 0..self.ndim {
            if d == self.dim {
                selections.push(NcSliceInfoElem::Index(self.current));
            } else {
                selections.push(NcSliceInfoElem::Slice {
                    start: 0,
                    end: u64::MAX,
                    step: 1,
                });
            }
        }
        let selection = NcSliceInfo { selections };
        self.current += 1;
        Some(self.file.read_variable_slice::<T>(&self.name, &selection))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining_u64 = self.dim_size.saturating_sub(self.current);
        let remaining = remaining_u64.min(usize::MAX as u64) as usize;
        (remaining, Some(remaining))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "netcdf4")]
    use std::sync::Arc;
    #[cfg(feature = "netcdf4")]
    use std::sync::Mutex;

    #[test]
    fn detect_cdf1() {
        let data = b"CDF\x01rest_of_file";
        assert_eq!(detect_format(data).unwrap(), NcFormat::Classic);
    }

    #[test]
    fn detect_cdf2() {
        let data = b"CDF\x02rest_of_file";
        assert_eq!(detect_format(data).unwrap(), NcFormat::Offset64);
    }

    #[test]
    fn detect_cdf5() {
        let data = b"CDF\x05rest_of_file";
        assert_eq!(detect_format(data).unwrap(), NcFormat::Cdf5);
    }

    #[test]
    fn detect_hdf5() {
        let mut data = vec![0x89, b'H', b'D', b'F', 0x0D, 0x0A, 0x1A, 0x0A];
        data.extend_from_slice(b"rest_of_file");
        assert_eq!(detect_format(&data).unwrap(), NcFormat::Nc4);
    }

    #[test]
    fn detect_invalid_magic() {
        let data = b"XXXX";
        assert!(matches!(
            detect_format(data).unwrap_err(),
            Error::InvalidMagic
        ));
    }

    #[test]
    fn detect_unsupported_version() {
        let data = b"CDF\x03";
        assert!(matches!(
            detect_format(data).unwrap_err(),
            Error::UnsupportedVersion(3)
        ));
    }

    #[test]
    fn detect_too_short() {
        let data = b"CD";
        assert!(matches!(
            detect_format(data).unwrap_err(),
            Error::InvalidMagic
        ));
    }

    #[test]
    fn from_bytes_minimal_cdf1() {
        // Minimal valid CDF-1 file: magic + numrecs + absent dim/att/var lists.
        let mut data = Vec::new();
        data.extend_from_slice(b"CDF\x01");
        data.extend_from_slice(&0u32.to_be_bytes()); // numrecs = 0
                                                     // dim_list: ABSENT
        data.extend_from_slice(&0u32.to_be_bytes()); // tag = 0
        data.extend_from_slice(&0u32.to_be_bytes()); // count = 0
                                                     // att_list: ABSENT
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        // var_list: ABSENT
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());

        let file = NcFile::from_bytes(&data).unwrap();
        assert_eq!(file.format(), NcFormat::Classic);
        assert!(file.dimensions().unwrap().is_empty());
        assert!(file.variables().unwrap().is_empty());
        assert!(file.global_attributes().unwrap().is_empty());
    }

    #[cfg(feature = "netcdf4")]
    struct CountingStorage {
        data: Arc<[u8]>,
        reads: Mutex<Vec<(u64, usize)>>,
    }

    #[cfg(feature = "netcdf4")]
    impl CountingStorage {
        fn new(data: Vec<u8>) -> Self {
            Self {
                data: Arc::<[u8]>::from(data),
                reads: Mutex::new(Vec::new()),
            }
        }

        fn reads(&self) -> Vec<(u64, usize)> {
            self.reads.lock().unwrap().clone()
        }
    }

    #[cfg(feature = "netcdf4")]
    impl hdf5_reader::Storage for CountingStorage {
        fn len(&self) -> u64 {
            self.data.len() as u64
        }

        fn read_range(
            &self,
            offset: u64,
            len: usize,
        ) -> hdf5_reader::error::Result<hdf5_reader::StorageBuffer> {
            self.reads.lock().unwrap().push((offset, len));
            let start = usize::try_from(offset)
                .map_err(|_| hdf5_reader::error::Error::OffsetOutOfBounds(offset))?;
            let end = start
                .checked_add(len)
                .ok_or(hdf5_reader::error::Error::OffsetOutOfBounds(offset))?;
            if end > self.data.len() {
                return Err(hdf5_reader::error::Error::UnexpectedEof {
                    offset,
                    needed: len as u64,
                    available: self.len().saturating_sub(offset),
                });
            }
            Ok(hdf5_reader::StorageBuffer::from_vec(
                self.data[start..end].to_vec(),
            ))
        }
    }

    #[cfg(feature = "netcdf4")]
    fn classic_large_offset_fixture() -> Vec<u8> {
        fn write_name(buf: &mut Vec<u8>, name: &str) {
            buf.extend_from_slice(&(name.len() as u32).to_be_bytes());
            buf.extend_from_slice(name.as_bytes());
            while buf.len() % 4 != 0 {
                buf.push(0);
            }
        }

        const DATA_OFFSET: usize = 70 * 1024;
        let mut buf = Vec::new();
        buf.extend_from_slice(b"CDF\x01");
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0x0000_000Au32.to_be_bytes());
        buf.extend_from_slice(&1u32.to_be_bytes());
        write_name(&mut buf, "x");
        buf.extend_from_slice(&4u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0x0000_000Bu32.to_be_bytes());
        buf.extend_from_slice(&1u32.to_be_bytes());
        write_name(&mut buf, "data");
        buf.extend_from_slice(&1u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&4u32.to_be_bytes());
        buf.extend_from_slice(&16u32.to_be_bytes());
        buf.extend_from_slice(&(DATA_OFFSET as u32).to_be_bytes());

        buf.resize(DATA_OFFSET, 0);
        for value in [10i32, 20, 30, 40] {
            buf.extend_from_slice(&value.to_be_bytes());
        }
        buf
    }

    fn write_cdf1_name(buf: &mut Vec<u8>, name: &str) {
        buf.extend_from_slice(&(name.len() as u32).to_be_bytes());
        buf.extend_from_slice(name.as_bytes());
        while buf.len() % 4 != 0 {
            buf.push(0);
        }
    }

    fn write_cdf5_count(buf: &mut Vec<u8>, value: u64) {
        buf.extend_from_slice(&value.to_be_bytes());
    }

    fn write_cdf5_name(buf: &mut Vec<u8>, name: &str) {
        write_cdf5_count(buf, name.len() as u64);
        buf.extend_from_slice(name.as_bytes());
        while buf.len() % 4 != 0 {
            buf.push(0);
        }
    }

    fn cdf5_huge_dimension_fixture() -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"CDF\x05");
        write_cdf5_count(&mut buf, 0);

        buf.extend_from_slice(&0x0000_000Au32.to_be_bytes());
        write_cdf5_count(&mut buf, 1);
        write_cdf5_name(&mut buf, "n");
        write_cdf5_count(&mut buf, u64::MAX);

        buf.extend_from_slice(&0u32.to_be_bytes());
        write_cdf5_count(&mut buf, 0);

        buf.extend_from_slice(&0x0000_000Bu32.to_be_bytes());
        write_cdf5_count(&mut buf, 1);
        write_cdf5_name(&mut buf, "big");
        write_cdf5_count(&mut buf, 1);
        write_cdf5_count(&mut buf, 0);
        buf.extend_from_slice(&0u32.to_be_bytes());
        write_cdf5_count(&mut buf, 0);
        buf.extend_from_slice(&4u32.to_be_bytes());
        write_cdf5_count(&mut buf, 4);
        let offset_pos = buf.len();
        buf.extend_from_slice(&0u64.to_be_bytes());

        let data_offset = buf.len() as u64;
        buf[offset_pos..offset_pos + 8].copy_from_slice(&data_offset.to_be_bytes());
        buf.extend_from_slice(&123i32.to_be_bytes());
        buf
    }

    fn subfiling_marker_fixture() -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"CDF\x01");
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());

        buf.extend_from_slice(&0x0000_000Cu32.to_be_bytes());
        buf.extend_from_slice(&1u32.to_be_bytes());
        write_cdf1_name(&mut buf, "_PnetCDF_SubFiling_enabled");
        buf.extend_from_slice(&4u32.to_be_bytes());
        buf.extend_from_slice(&1u32.to_be_bytes());
        buf.extend_from_slice(&1i32.to_be_bytes());

        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf
    }

    fn streaming_cdf1_record_fixture(values: &[i32], trailing: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"CDF\x01");
        buf.extend_from_slice(&u32::MAX.to_be_bytes());

        buf.extend_from_slice(&0x0000_000Au32.to_be_bytes());
        buf.extend_from_slice(&1u32.to_be_bytes());
        write_cdf1_name(&mut buf, "time");
        buf.extend_from_slice(&0u32.to_be_bytes());

        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());

        buf.extend_from_slice(&0x0000_000Bu32.to_be_bytes());
        buf.extend_from_slice(&1u32.to_be_bytes());
        write_cdf1_name(&mut buf, "temp");
        buf.extend_from_slice(&1u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&4u32.to_be_bytes());
        buf.extend_from_slice(&4u32.to_be_bytes());
        let offset_pos = buf.len();
        buf.extend_from_slice(&0u32.to_be_bytes());

        let data_offset = buf.len() as u32;
        buf[offset_pos..offset_pos + 4].copy_from_slice(&data_offset.to_be_bytes());

        for value in values {
            buf.extend_from_slice(&value.to_be_bytes());
        }
        buf.extend_from_slice(trailing);
        buf
    }

    fn streaming_cdf2_record_fixture(values: &[i32]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"CDF\x02");
        buf.extend_from_slice(&u32::MAX.to_be_bytes());

        buf.extend_from_slice(&0x0000_000Au32.to_be_bytes());
        buf.extend_from_slice(&1u32.to_be_bytes());
        write_cdf1_name(&mut buf, "time");
        buf.extend_from_slice(&0u32.to_be_bytes());

        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());

        buf.extend_from_slice(&0x0000_000Bu32.to_be_bytes());
        buf.extend_from_slice(&1u32.to_be_bytes());
        write_cdf1_name(&mut buf, "temp");
        buf.extend_from_slice(&1u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&4u32.to_be_bytes());
        buf.extend_from_slice(&4u32.to_be_bytes());
        let offset_pos = buf.len();
        buf.extend_from_slice(&0u64.to_be_bytes());

        let data_offset = buf.len() as u64;
        buf[offset_pos..offset_pos + 8].copy_from_slice(&data_offset.to_be_bytes());

        for value in values {
            buf.extend_from_slice(&value.to_be_bytes());
        }
        buf
    }

    fn streaming_cdf1_two_record_var_fixture(records: &[(i32, i16)]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"CDF\x01");
        buf.extend_from_slice(&u32::MAX.to_be_bytes());

        buf.extend_from_slice(&0x0000_000Au32.to_be_bytes());
        buf.extend_from_slice(&1u32.to_be_bytes());
        write_cdf1_name(&mut buf, "time");
        buf.extend_from_slice(&0u32.to_be_bytes());

        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());

        buf.extend_from_slice(&0x0000_000Bu32.to_be_bytes());
        buf.extend_from_slice(&2u32.to_be_bytes());

        write_cdf1_name(&mut buf, "temp");
        buf.extend_from_slice(&1u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&4u32.to_be_bytes());
        buf.extend_from_slice(&4u32.to_be_bytes());
        let temp_offset_pos = buf.len();
        buf.extend_from_slice(&0u32.to_be_bytes());

        write_cdf1_name(&mut buf, "flag");
        buf.extend_from_slice(&1u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&3u32.to_be_bytes());
        buf.extend_from_slice(&2u32.to_be_bytes());
        let flag_offset_pos = buf.len();
        buf.extend_from_slice(&0u32.to_be_bytes());

        let data_offset = buf.len() as u32;
        let flag_offset = data_offset + 4;
        buf[temp_offset_pos..temp_offset_pos + 4].copy_from_slice(&data_offset.to_be_bytes());
        buf[flag_offset_pos..flag_offset_pos + 4].copy_from_slice(&flag_offset.to_be_bytes());

        for &(temp, flag) in records {
            buf.extend_from_slice(&temp.to_be_bytes());
            buf.extend_from_slice(&flag.to_be_bytes());
            buf.extend_from_slice(&[0, 0]);
        }
        buf
    }

    #[test]
    fn cdf5_huge_dimension_can_slice_but_full_read_errors_cleanly() {
        let file = NcFile::from_bytes(&cdf5_huge_dimension_fixture()).unwrap();
        let selection = NcSliceInfo {
            selections: vec![NcSliceInfoElem::Index(0)],
        };

        let sliced: ndarray::ArrayD<i32> = file.read_variable_slice("big", &selection).unwrap();
        assert_eq!(sliced.as_slice().unwrap(), &[123]);

        let err = file.read_variable::<i32>("big").unwrap_err();
        assert!(matches!(err, Error::InvalidData(_)));
    }

    #[test]
    fn classic_subfiling_marker_returns_unsupported_feature() {
        let err = match NcFile::from_bytes(&subfiling_marker_fixture()) {
            Ok(_) => panic!("subfiling marker should be rejected"),
            Err(err) => err,
        };
        assert!(matches!(
            err,
            Error::UnsupportedFeature(message) if message.contains("PnetCDF subfiling")
        ));
    }

    #[test]
    fn streaming_cdf1_numrecs_are_derived_from_file_length() {
        let file = NcFile::from_bytes(&streaming_cdf1_record_fixture(&[10, 20, 30], &[])).unwrap();

        assert_eq!(file.as_classic().unwrap().numrecs(), 3);
        assert_eq!(file.dimension("time").unwrap().size, 3);
        assert_eq!(file.variable("temp").unwrap().shape(), vec![3]);

        let values: ndarray::ArrayD<i32> = file.read_variable("temp").unwrap();
        assert_eq!(values.as_slice().unwrap(), &[10, 20, 30]);
    }

    #[test]
    fn streaming_cdf1_numrecs_ignore_trailing_partial_record() {
        let file =
            NcFile::from_bytes(&streaming_cdf1_record_fixture(&[10, 20], &[0xAA, 0xBB])).unwrap();

        assert_eq!(file.as_classic().unwrap().numrecs(), 2);
        assert_eq!(file.variable("temp").unwrap().shape(), vec![2]);

        let values: ndarray::ArrayD<i32> = file.read_variable("temp").unwrap();
        assert_eq!(values.as_slice().unwrap(), &[10, 20]);
    }

    #[test]
    fn streaming_cdf2_numrecs_are_derived_from_file_length() {
        let file = NcFile::from_bytes(&streaming_cdf2_record_fixture(&[10, 20, 30])).unwrap();

        assert_eq!(file.format(), NcFormat::Offset64);
        assert_eq!(file.as_classic().unwrap().numrecs(), 3);
        assert_eq!(file.dimension("time").unwrap().size, 3);
        assert_eq!(file.variable("temp").unwrap().shape(), vec![3]);

        let values: ndarray::ArrayD<i32> = file.read_variable("temp").unwrap();
        assert_eq!(values.as_slice().unwrap(), &[10, 20, 30]);
    }

    #[test]
    fn streaming_cdf1_numrecs_use_full_stride_for_multiple_record_variables() {
        let file = NcFile::from_bytes(&streaming_cdf1_two_record_var_fixture(&[
            (10, 1),
            (20, 2),
            (30, 3),
        ]))
        .unwrap();

        assert_eq!(file.as_classic().unwrap().numrecs(), 3);
        assert_eq!(file.variable("temp").unwrap().shape(), vec![3]);
        assert_eq!(file.variable("flag").unwrap().shape(), vec![3]);

        let temps: ndarray::ArrayD<i32> = file.read_variable("temp").unwrap();
        let flags: ndarray::ArrayD<i16> = file.read_variable("flag").unwrap();
        assert_eq!(temps.as_slice().unwrap(), &[10, 20, 30]);
        assert_eq!(flags.as_slice().unwrap(), &[1, 2, 3]);
    }

    #[cfg(feature = "netcdf4")]
    #[test]
    fn streaming_cdf1_from_storage_derives_numrecs() {
        let file = NcFile::from_storage(Arc::new(BytesStorage::new(
            streaming_cdf1_record_fixture(&[10, 20, 30], &[]),
        )))
        .unwrap();

        assert_eq!(file.as_classic().unwrap().numrecs(), 3);
        assert_eq!(file.dimension("time").unwrap().size, 3);

        let values: ndarray::ArrayD<i32> = file.read_variable("temp").unwrap();
        assert_eq!(values.as_slice().unwrap(), &[10, 20, 30]);
    }

    #[cfg(feature = "netcdf4")]
    #[test]
    fn classic_from_storage_keeps_open_range_backed() {
        let data = classic_large_offset_fixture();
        let full_len = data.len();
        let storage = Arc::new(CountingStorage::new(data));

        let file = NcFile::from_storage(storage.clone()).unwrap();
        assert_eq!(file.format(), NcFormat::Classic);
        assert!(!storage
            .reads()
            .iter()
            .any(|&(offset, len)| offset == 0 && len == full_len));

        let values: ndarray::ArrayD<i32> = file.read_variable("data").unwrap();
        assert_eq!(values.as_slice().unwrap(), &[10, 20, 30, 40]);
        assert!(storage
            .reads()
            .iter()
            .any(|&(offset, len)| { offset == (70 * 1024) as u64 && len == 16 }));
    }

    #[cfg(feature = "netcdf4")]
    #[test]
    fn classic_from_storage_slice_reads_planned_range() {
        let storage = Arc::new(CountingStorage::new(classic_large_offset_fixture()));
        let file = NcFile::from_storage(storage.clone()).unwrap();
        let selection = NcSliceInfo {
            selections: vec![NcSliceInfoElem::Slice {
                start: 1,
                end: 3,
                step: 1,
            }],
        };

        let values: ndarray::ArrayD<i32> = file.read_variable_slice("data", &selection).unwrap();
        assert_eq!(values.as_slice().unwrap(), &[20, 30]);
        assert!(storage
            .reads()
            .iter()
            .any(|&(offset, len)| { offset == (70 * 1024 + 4) as u64 && len == 8 }));
    }

    #[cfg(feature = "netcdf4")]
    #[test]
    fn from_storage_minimal_cdf1() {
        // Minimal valid CDF-1 file: magic + numrecs + absent dim/att/var lists.
        let mut data = Vec::new();
        data.extend_from_slice(b"CDF\x01");
        data.extend_from_slice(&0u32.to_be_bytes()); // numrecs = 0
                                                     // dim_list: ABSENT
        data.extend_from_slice(&0u32.to_be_bytes()); // tag = 0
        data.extend_from_slice(&0u32.to_be_bytes()); // count = 0
                                                     // att_list: ABSENT
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        // var_list: ABSENT
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());

        let file = NcFile::from_storage(Arc::new(BytesStorage::new(data))).unwrap();
        assert_eq!(file.format(), NcFormat::Classic);
        assert!(file.dimensions().unwrap().is_empty());
        assert!(file.variables().unwrap().is_empty());
        assert!(file.global_attributes().unwrap().is_empty());
    }

    #[cfg(feature = "netcdf4")]
    #[test]
    fn from_storage_short_input_reports_invalid_magic() {
        let err = NcFile::from_storage(Arc::new(BytesStorage::new(vec![b'C', b'D'])))
            .err()
            .expect("short storage should not parse as NetCDF");
        assert!(matches!(err, Error::InvalidMagic));
    }

    #[test]
    fn from_bytes_cdf1_with_data() {
        // Build a CDF-1 file with one dimension, one global attribute, and one variable.
        let mut data = Vec::new();
        data.extend_from_slice(b"CDF\x01");
        data.extend_from_slice(&0u32.to_be_bytes()); // numrecs = 0

        // dim_list: 1 dimension "x" with size 3
        data.extend_from_slice(&0x0000_000Au32.to_be_bytes()); // NC_DIMENSION tag
        data.extend_from_slice(&1u32.to_be_bytes()); // nelems = 1
                                                     // name "x": length=1, "x", 3 bytes padding
        data.extend_from_slice(&1u32.to_be_bytes());
        data.push(b'x');
        data.extend_from_slice(&[0, 0, 0]); // padding to 4
                                            // dim size
        data.extend_from_slice(&3u32.to_be_bytes());

        // att_list: 1 attribute "title" = "test"
        data.extend_from_slice(&0x0000_000Cu32.to_be_bytes()); // NC_ATTRIBUTE tag
        data.extend_from_slice(&1u32.to_be_bytes()); // nelems = 1
                                                     // name "title"
        data.extend_from_slice(&5u32.to_be_bytes());
        data.extend_from_slice(b"title");
        data.extend_from_slice(&[0, 0, 0]); // padding
                                            // nc_type = NC_CHAR = 2
        data.extend_from_slice(&2u32.to_be_bytes());
        // nvalues = 4
        data.extend_from_slice(&4u32.to_be_bytes());
        data.extend_from_slice(b"test"); // exactly 4 bytes, no padding needed

        // var_list: 1 variable "vals" with dim x, type float
        data.extend_from_slice(&0x0000_000Bu32.to_be_bytes()); // NC_VARIABLE tag
        data.extend_from_slice(&1u32.to_be_bytes()); // nelems = 1
                                                     // name "vals"
        data.extend_from_slice(&4u32.to_be_bytes());
        data.extend_from_slice(b"vals");
        // ndims = 1
        data.extend_from_slice(&1u32.to_be_bytes());
        // dimid = 0
        data.extend_from_slice(&0u32.to_be_bytes());
        // att_list: absent
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        // nc_type = NC_FLOAT = 5
        data.extend_from_slice(&5u32.to_be_bytes());
        // vsize = 12 (3 floats * 4 bytes)
        data.extend_from_slice(&12u32.to_be_bytes());
        // begin (offset): we'll put data right after this header
        let data_offset = data.len() as u32 + 4; // +4 for this field itself
        data.extend_from_slice(&data_offset.to_be_bytes());

        // Now append the variable data: 3 floats
        data.extend_from_slice(&1.5f32.to_be_bytes());
        data.extend_from_slice(&2.5f32.to_be_bytes());
        data.extend_from_slice(&3.5f32.to_be_bytes());

        let file = NcFile::from_bytes(&data).unwrap();
        assert_eq!(file.format(), NcFormat::Classic);
        assert_eq!(file.dimensions().unwrap().len(), 1);
        assert_eq!(file.dimensions().unwrap()[0].name, "x");
        assert_eq!(file.dimensions().unwrap()[0].size, 3);

        assert_eq!(file.global_attributes().unwrap().len(), 1);
        assert_eq!(file.global_attributes().unwrap()[0].name, "title");
        assert_eq!(
            file.global_attributes().unwrap()[0]
                .value
                .as_string()
                .unwrap(),
            "test"
        );

        assert_eq!(file.variables().unwrap().len(), 1);
        let var = file.variable("vals").unwrap();
        assert_eq!(var.dtype(), &NcType::Float);
        assert_eq!(var.shape(), vec![3]);

        // Read the actual data through the classic file.
        let classic = file.as_classic().unwrap();
        let arr: ndarray::ArrayD<f32> = classic.read_variable("vals").unwrap();
        assert_eq!(arr.shape(), &[3]);
        assert_eq!(arr[[0]], 1.5f32);
        assert_eq!(arr[[1]], 2.5f32);
        assert_eq!(arr[[2]], 3.5f32);
    }

    #[test]
    fn variable_not_found() {
        let mut data = Vec::new();
        data.extend_from_slice(b"CDF\x01");
        data.extend_from_slice(&0u32.to_be_bytes());
        // All absent.
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());

        let file = NcFile::from_bytes(&data).unwrap();
        assert!(matches!(
            file.variable("nonexistent").unwrap_err(),
            Error::VariableNotFound(_)
        ));
    }

    #[test]
    fn group_not_found() {
        let mut data = Vec::new();
        data.extend_from_slice(b"CDF\x01");
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());

        let file = NcFile::from_bytes(&data).unwrap();
        assert!(matches!(
            file.group("nonexistent").unwrap_err(),
            Error::GroupNotFound(_)
        ));
    }
}
