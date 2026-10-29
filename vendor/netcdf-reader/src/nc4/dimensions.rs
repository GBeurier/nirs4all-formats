//! Reconstruct NetCDF-4 dimensions from HDF5 metadata.
//!
//! NetCDF-4 stores dimension information as:
//! - Scale datasets (one per dimension) with `CLASS=DIMENSION_SCALE` attribute
//! - `DIMENSION_LIST` attribute on each variable dataset referencing the scales
//! - `REFERENCE_LIST` attribute on scale datasets (back-references)
//! - `_Netcdf4Dimid` attribute to assign stable dimension IDs
//! - `NAME` attribute on scale datasets gives the dimension name
//!
//! Unlimited dimensions are represented by chunked datasets whose maximum
//! dimension in the dataspace is `H5S_UNLIMITED`.

use std::collections::HashMap;

use hdf5_reader::group::Group;

use crate::error::{Error, Result};
use crate::types::NcDimension;

fn leaf_name(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

pub(crate) fn is_dimension_without_variable_name(name: &str) -> bool {
    name.starts_with("This is a netCDF dimension but not a netCDF variable")
}

/// Extract dimensions from an HDF5 group.
///
/// Returns a tuple of:
/// - The list of dimensions (sorted by `_Netcdf4Dimid` if available)
/// - A map from dataset object-header address to the corresponding dimension
///
/// The address map is used by `extract_variables` to resolve `DIMENSION_LIST`
/// references back to the correct dimension by address rather than by size.
pub fn extract_dimensions(
    group: &Group,
    metadata_mode: crate::NcMetadataMode,
) -> Result<(Vec<NcDimension>, HashMap<u64, NcDimension>)> {
    let datasets = group.datasets()?;
    extract_dimensions_from_datasets(&datasets, metadata_mode)
}

pub fn extract_dimensions_from_datasets(
    datasets: &[hdf5_reader::Dataset],
    metadata_mode: crate::NcMetadataMode,
) -> Result<(Vec<NcDimension>, HashMap<u64, NcDimension>)> {
    let mut dims: Vec<(Option<i64>, NcDimension, u64)> = Vec::new();

    for ds in datasets {
        if let Some((dimid, dim, address)) = extract_dimension_entry(ds, metadata_mode)? {
            dims.push((dimid, dim, address));
        }
    }

    // Sort by _Netcdf4Dimid if available, otherwise preserve order
    dims.sort_by_key(|(id, _, _)| id.unwrap_or(i64::MAX));

    let addr_map: HashMap<u64, NcDimension> =
        dims.iter().map(|(_, d, addr)| (*addr, d.clone())).collect();

    let dim_list: Vec<NcDimension> = dims.into_iter().map(|(_, d, _)| d).collect();

    Ok((dim_list, addr_map))
}

fn extract_dimension_entry(
    ds: &hdf5_reader::Dataset,
    metadata_mode: crate::NcMetadataMode,
) -> Result<Option<(Option<i64>, NcDimension, u64)>> {
    let strict = metadata_mode == crate::NcMetadataMode::Strict;

    let is_dim_scale = match ds.attribute("CLASS") {
        Ok(attr) => match attr.read_string() {
            Ok(value) => value == "DIMENSION_SCALE",
            Err(err) if strict => {
                return Err(Error::InvalidData(format!(
                    "dataset '{}' has unreadable CLASS attribute: {err}",
                    ds.name()
                )))
            }
            Err(_) => false,
        },
        Err(_) => false,
    };

    if !is_dim_scale {
        return Ok(None);
    }

    let name = match ds.attribute("NAME") {
        Ok(attr) => match attr.read_string() {
            Ok(value) => {
                if is_dimension_without_variable_name(&value) {
                    leaf_name(ds.name()).to_string()
                } else {
                    value
                }
            }
            Err(err) if strict => {
                return Err(Error::InvalidData(format!(
                    "dimension scale '{}' has unreadable NAME attribute: {err}",
                    ds.name()
                )))
            }
            Err(_) => leaf_name(ds.name()).to_string(),
        },
        Err(_) => leaf_name(ds.name()).to_string(),
    };

    let shape = ds.shape();
    let size = if shape.is_empty() { 0 } else { shape[0] };
    let is_unlimited = ds
        .max_dims()
        .is_some_and(|md| !md.is_empty() && md[0] == u64::MAX);
    let dimid = match ds.attribute("_Netcdf4Dimid") {
        Ok(attr) => match attr.read_scalar::<i32>() {
            Ok(id) => Some(id as i64),
            Err(err) if strict => {
                return Err(Error::InvalidData(format!(
                    "dimension scale '{}' has unreadable _Netcdf4Dimid attribute: {err}",
                    ds.name()
                )))
            }
            Err(_) => None,
        },
        Err(_) => None,
    };

    Ok(Some((
        dimid,
        NcDimension {
            name,
            size,
            is_unlimited,
        },
        ds.address(),
    )))
}
