//! Map HDF5 datasets to NetCDF-4 variables.
//!
//! Each user-visible HDF5 dataset becomes an NcVariable. Dimension-only scale
//! datasets are internal metadata, but coordinate variables are also dimension
//! scales and must remain visible. Non-scale variable dimensions are determined
//! from the `DIMENSION_LIST` attribute, which contains object references to the
//! corresponding dimension-scale datasets.

use std::collections::HashMap;

use hdf5_reader::group::Group;

use crate::error::{Error, Result};
use crate::types::{NcDimension, NcVariable};

use super::attributes;
use super::types::hdf5_to_nc_type;

fn leaf_name(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

/// Extract variables from an HDF5 group.
///
/// Dimension-only scale datasets are internal metadata. Coordinate variables are
/// also dimension scales, so they are preserved as NcVariables.
///
/// `dim_addr_map` maps dimension-scale dataset addresses to their `NcDimension`,
/// used to resolve `DIMENSION_LIST` object references.
pub fn extract_variables(
    group: &Group,
    dimensions: &[NcDimension],
    dim_addr_map: &HashMap<u64, NcDimension>,
    metadata_mode: crate::NcMetadataMode,
) -> Result<Vec<NcVariable>> {
    let datasets = group.datasets()?;
    extract_variables_from_datasets(&datasets, group, dimensions, dim_addr_map, metadata_mode)
}

pub fn extract_variables_from_datasets(
    datasets: &[hdf5_reader::Dataset],
    group: &Group,
    dimensions: &[NcDimension],
    dim_addr_map: &HashMap<u64, NcDimension>,
    metadata_mode: crate::NcMetadataMode,
) -> Result<Vec<NcVariable>> {
    let mut variables = Vec::new();

    for ds in datasets {
        if let Some(variable) =
            extract_variable(ds, group, dimensions, dim_addr_map, metadata_mode)?
        {
            variables.push(variable);
        }
    }

    Ok(variables)
}

pub fn extract_variable(
    ds: &hdf5_reader::Dataset,
    group: &Group,
    dimensions: &[NcDimension],
    dim_addr_map: &HashMap<u64, NcDimension>,
    metadata_mode: crate::NcMetadataMode,
) -> Result<Option<NcVariable>> {
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

    if is_dim_scale && is_dimension_only_scale(ds, strict)? {
        return Ok(None);
    }

    let nc_type = match hdf5_to_nc_type(ds.dtype()) {
        Ok(t) => t,
        Err(err) if strict => {
            return Err(Error::InvalidData(format!(
                "dataset '{}' uses unsupported NetCDF-4 type: {err}",
                ds.name()
            )))
        }
        Err(_) => return Ok(None),
    };

    let var_dims = if is_dim_scale {
        resolve_coordinate_variable_dimensions(ds, dim_addr_map, metadata_mode)?
    } else {
        resolve_variable_dimensions(ds, group, dimensions, dim_addr_map, metadata_mode)?
    };
    let is_unlimited = var_dims.iter().any(|d| d.is_unlimited);
    let shape = ds.shape();
    let elem_size = u64::try_from(nc_type.size()?)
        .map_err(|_| Error::InvalidData("NetCDF-4 type size exceeds u64 capacity".to_string()))?;
    let (data_size, record_size) = compute_storage_sizes(shape, elem_size, is_unlimited)?;
    let var_attrs = attributes::extract_variable_attributes(ds, metadata_mode)?;

    Ok(Some(NcVariable {
        name: leaf_name(ds.name()).to_string(),
        dimensions: var_dims,
        dtype: nc_type,
        attributes: var_attrs,
        data_offset: ds.address(),
        _data_size: data_size,
        is_record_var: is_unlimited,
        record_size,
    }))
}

fn is_dimension_only_scale(ds: &hdf5_reader::Dataset, strict: bool) -> Result<bool> {
    match ds.attribute("NAME") {
        Ok(attr) => match attr.read_string() {
            Ok(value) => Ok(super::dimensions::is_dimension_without_variable_name(
                &value,
            )),
            Err(err) if strict => Err(Error::InvalidData(format!(
                "dimension scale '{}' has unreadable NAME attribute: {err}",
                ds.name()
            ))),
            Err(_) => Ok(false),
        },
        Err(_) => Ok(false),
    }
}

fn resolve_coordinate_variable_dimensions(
    ds: &hdf5_reader::Dataset,
    dim_addr_map: &HashMap<u64, NcDimension>,
    metadata_mode: crate::NcMetadataMode,
) -> Result<Vec<NcDimension>> {
    if let Some(dim) = dim_addr_map.get(&ds.address()) {
        Ok(vec![dim.clone()])
    } else if metadata_mode == crate::NcMetadataMode::Lossy {
        Ok(vec![NcDimension {
            name: leaf_name(ds.name()).to_string(),
            size: ds.shape().first().copied().unwrap_or(0),
            is_unlimited: ds
                .max_dims()
                .is_some_and(|md| !md.is_empty() && md[0] == u64::MAX),
        }])
    } else {
        Err(Error::InvalidData(format!(
            "coordinate variable '{}' is not registered as a dimension scale",
            ds.name()
        )))
    }
}

/// Resolve variable dimensions via the `DIMENSION_LIST` attribute.
///
/// `DIMENSION_LIST` is a VLen-of-object-reference attribute. Each entry is a
/// variable-length sequence of object references pointing to dimension-scale
/// datasets. We parse the raw attribute data to extract these references.
///
fn resolve_variable_dimensions(
    ds: &hdf5_reader::Dataset,
    group: &Group,
    dimensions: &[NcDimension],
    dim_addr_map: &HashMap<u64, NcDimension>,
    metadata_mode: crate::NcMetadataMode,
) -> Result<Vec<NcDimension>> {
    resolve_variable_dimensions_with_mode(ds, group, dimensions, dim_addr_map, metadata_mode)
}

fn resolve_variable_dimensions_with_mode(
    ds: &hdf5_reader::Dataset,
    group: &Group,
    dimensions: &[NcDimension],
    dim_addr_map: &HashMap<u64, NcDimension>,
    metadata_mode: crate::NcMetadataMode,
) -> Result<Vec<NcDimension>> {
    match resolve_variable_dimensions_from_dimlist(ds, group, dim_addr_map) {
        Ok(dims) => Ok(dims),
        Err(_err) if metadata_mode == crate::NcMetadataMode::Lossy => {
            Ok(resolve_variable_dimensions_by_size(ds, dimensions))
        }
        Err(err) => Err(err),
    }
}

/// Resolve variable dimensions via the `DIMENSION_LIST` attribute.
fn resolve_variable_dimensions_from_dimlist(
    ds: &hdf5_reader::Dataset,
    group: &Group,
    dim_addr_map: &HashMap<u64, NcDimension>,
) -> Result<Vec<NcDimension>> {
    let dim_addrs = resolve_dimension_scale_addresses(ds, group)?;
    let mut var_dims = Vec::with_capacity(dim_addrs.len());
    for dim_addr in dim_addrs {
        if let Some(dim) = dim_addr_map.get(&dim_addr) {
            var_dims.push(dim.clone());
        } else {
            return Err(Error::InvalidData(format!(
                "dataset '{}' references unknown dimension scale address {dim_addr:#x}",
                ds.name()
            )));
        }
    }

    // Apply unlimited status from the dataset's max_dims.
    if let Some(max_dims) = ds.max_dims() {
        for (i, md) in max_dims.iter().enumerate() {
            if *md == u64::MAX && i < var_dims.len() {
                var_dims[i].is_unlimited = true;
            }
        }
    }

    Ok(var_dims)
}

pub(crate) fn resolve_dimension_scale_addresses(
    ds: &hdf5_reader::Dataset,
    group: &Group,
) -> Result<Vec<u64>> {
    // A rank-0 variable has no dimensions to reference.
    let ndim = ds.ndim();
    if ndim == 0 {
        return Ok(Vec::new());
    }

    let attr = ds.attribute("DIMENSION_LIST").map_err(|_| {
        Error::InvalidData(format!(
            "dataset '{}' is missing required DIMENSION_LIST metadata",
            ds.name()
        ))
    })?;
    let raw_data = &attr.raw_data;
    let offset_size = group.offset_size();

    if raw_data.is_empty() {
        return Err(Error::InvalidData(format!(
            "dataset '{}' has empty DIMENSION_LIST metadata",
            ds.name()
        )));
    }

    let entry_size = 4 + usize::from(offset_size) + 4;
    if raw_data.len() < ndim * entry_size {
        return Err(Error::InvalidData(format!(
            "dataset '{}' has truncated DIMENSION_LIST metadata",
            ds.name()
        )));
    }

    let mut dim_addrs = Vec::with_capacity(ndim);
    let mut cursor = hdf5_reader::io::Cursor::new(raw_data);

    for _ in 0..ndim {
        let seq_len = cursor.read_u32_le().map_err(|err| {
            Error::InvalidData(format!(
                "dataset '{}' has invalid DIMENSION_LIST entry count: {err}",
                ds.name()
            ))
        })? as usize;
        let heap_addr = cursor.read_offset(offset_size).map_err(|err| {
            Error::InvalidData(format!(
                "dataset '{}' has invalid DIMENSION_LIST heap address: {err}",
                ds.name()
            ))
        })?;
        let heap_idx = cursor.read_u32_le().map_err(|err| {
            Error::InvalidData(format!(
                "dataset '{}' has invalid DIMENSION_LIST heap index: {err}",
                ds.name()
            ))
        })? as u16;

        if seq_len == 0 || hdf5_reader::io::Cursor::is_undefined_offset(heap_addr, offset_size) {
            return Err(Error::InvalidData(format!(
                "dataset '{}' has an unresolved DIMENSION_LIST reference",
                ds.name()
            )));
        }

        let collection = hdf5_reader::global_heap::GlobalHeapCollection::parse_at_storage(
            group.storage(),
            heap_addr,
            offset_size,
            group.length_size(),
        )
        .map_err(|err| {
            Error::InvalidData(format!(
                "dataset '{}' has unreadable DIMENSION_LIST heap object: {err}",
                ds.name()
            ))
        })?;

        let heap_obj = collection.get_object(heap_idx).ok_or_else(|| {
            Error::InvalidData(format!(
                "dataset '{}' references missing DIMENSION_LIST heap object {}",
                ds.name(),
                heap_idx
            ))
        })?;

        let refs = hdf5_reader::reference::read_object_references(&heap_obj.data, offset_size)
            .map_err(|err| {
                Error::InvalidData(format!(
                    "dataset '{}' has invalid DIMENSION_LIST references: {err}",
                    ds.name()
                ))
            })?;

        if refs.is_empty() {
            return Err(Error::InvalidData(format!(
                "dataset '{}' has empty DIMENSION_LIST references",
                ds.name()
            )));
        }

        dim_addrs.push(refs[0]);
    }

    Ok(dim_addrs)
}

fn compute_storage_sizes(shape: &[u64], elem_size: u64, is_unlimited: bool) -> Result<(u64, u64)> {
    let total_elements =
        crate::types::checked_shape_elements(shape, "NetCDF-4 variable element count")?;
    let data_size = crate::types::checked_mul_u64(
        total_elements,
        elem_size,
        "NetCDF-4 variable size in bytes",
    )?;

    let record_elements = if is_unlimited && shape.len() > 1 {
        crate::types::checked_shape_elements(&shape[1..], "NetCDF-4 record element count")?
    } else {
        1
    };
    let record_size =
        crate::types::checked_mul_u64(record_elements, elem_size, "NetCDF-4 record size in bytes")?;

    Ok((data_size, record_size))
}

/// Resolve dimensions for a variable by matching its shape against the
/// group's dimensions. Falls back to anonymous dimensions from the shape.
fn resolve_variable_dimensions_by_size(
    ds: &hdf5_reader::Dataset,
    dimensions: &[NcDimension],
) -> Vec<NcDimension> {
    let shape = ds.shape();

    // Try to match dimensions by size (simple heuristic when DIMENSION_LIST
    // isn't available or parseable). This matches dims in order.
    let mut var_dims = Vec::with_capacity(shape.len());
    let mut used = vec![false; dimensions.len()];

    for &dim_size in shape {
        let mut matched = false;
        for (i, dim) in dimensions.iter().enumerate() {
            if !used[i] && dim.size == dim_size {
                var_dims.push(dim.clone());
                used[i] = true;
                matched = true;
                break;
            }
        }
        if !matched {
            // An anonymous dimension named by its size (netCDF-C's phony dimensions); a second
            // axis of the same size in this variable gets its own name, so a 3 x 3 dataset has
            // two distinct axes.
            let base = format!("dim_{}", dim_size);
            let mut name = base.clone();
            let mut k = 2;
            while var_dims.iter().any(|d: &NcDimension| d.name == name) {
                name = format!("{base}_{k}");
                k += 1;
            }
            var_dims.push(NcDimension {
                name,
                size: dim_size,
                is_unlimited: false,
            });
        }
    }

    // Check if any matched dimension is unlimited and update dataspace info
    if let Some(max_dims) = ds.max_dims() {
        for (i, md) in max_dims.iter().enumerate() {
            if *md == u64::MAX && i < var_dims.len() {
                var_dims[i].is_unlimited = true;
            }
        }
    }

    var_dims
}

#[cfg(test)]
mod tests {
    use super::compute_storage_sizes;

    #[test]
    fn compute_storage_sizes_detects_overflow() {
        let err = compute_storage_sizes(&[u64::MAX, 2], 8, false).unwrap_err();
        assert!(matches!(err, crate::Error::InvalidData(_)));
    }

    #[test]
    fn compute_storage_sizes_record_dims() {
        let (data_size, record_size) = compute_storage_sizes(&[10, 3, 4], 4, true).unwrap();
        assert_eq!(data_size, 480);
        assert_eq!(record_size, 48);
    }
}
