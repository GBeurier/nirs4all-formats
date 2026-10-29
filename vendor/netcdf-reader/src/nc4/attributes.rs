//! Filter and convert HDF5 attributes to NetCDF-4 attributes.
//!
//! NetCDF-4 uses several internal HDF5 attributes that should not be exposed
//! to users:
//! - `_NCProperties`: file provenance metadata
//! - `_Netcdf4Dimid`: dimension ID assignment
//! - `_Netcdf4Coordinates`: coordinate variable tracking
//! - `DIMENSION_LIST`: dimension-to-variable references
//! - `REFERENCE_LIST`: variable-to-dimension back-references
//! - `CLASS`: dimension scale marker
//! - `NAME`: dimension scale name

use hdf5_reader::group::Group;
use hdf5_reader::messages::datatype::{Datatype, VarLenKind};

use crate::error::{Error, Result};
use crate::types::{NcAttrValue, NcAttribute};

/// Internal attribute names that should be hidden from the NetCDF user API.
const INTERNAL_ATTRIBUTES: &[&str] = &[
    "_NCProperties",
    "_Netcdf4Dimid",
    "_Netcdf4Coordinates",
    "_nc3_strict",
    "DIMENSION_LIST",
    "REFERENCE_LIST",
    "CLASS",
    "NAME",
];

/// Returns true if the given attribute name is an internal NetCDF-4/HDF5 attribute
/// that should not be exposed to users.
pub fn is_internal_attribute(name: &str) -> bool {
    INTERNAL_ATTRIBUTES.contains(&name)
}

/// Extract user-visible attributes from an HDF5 group, filtering out
/// internal NetCDF-4 attributes.
pub fn extract_group_attributes(
    group: &Group,
    metadata_mode: crate::NcMetadataMode,
) -> Result<Vec<NcAttribute>> {
    collect_visible_attributes(&group.attributes()?, metadata_mode)
}

/// Extract user-visible attributes from an HDF5 dataset, filtering out
/// internal attributes.
pub fn extract_variable_attributes(
    dataset: &hdf5_reader::Dataset,
    metadata_mode: crate::NcMetadataMode,
) -> Result<Vec<NcAttribute>> {
    collect_visible_attributes(&dataset.attributes(), metadata_mode)
}

fn collect_visible_attributes(
    attrs: &[hdf5_reader::Attribute],
    metadata_mode: crate::NcMetadataMode,
) -> Result<Vec<NcAttribute>> {
    let mut nc_attrs = Vec::new();
    for attr in attrs {
        if is_internal_attribute(&attr.name) {
            continue;
        }
        if let Some(nc_attr) = convert_attribute(attr, metadata_mode)? {
            nc_attrs.push(nc_attr);
        }
    }
    Ok(nc_attrs)
}

pub fn convert_visible_attribute(
    attr: &hdf5_reader::Attribute,
    metadata_mode: crate::NcMetadataMode,
) -> Result<Option<NcAttribute>> {
    if is_internal_attribute(&attr.name) {
        return Ok(None);
    }
    convert_attribute(attr, metadata_mode)
}

/// Convert an HDF5 attribute to a NetCDF attribute.
fn convert_attribute(
    attr: &hdf5_reader::Attribute,
    metadata_mode: crate::NcMetadataMode,
) -> Result<Option<NcAttribute>> {
    let Some(value) = convert_attribute_value(attr, metadata_mode)? else {
        return Ok(None);
    };

    Ok(Some(NcAttribute {
        name: attr.name.clone(),
        value,
    }))
}

/// Convert an HDF5 attribute's value to an NcAttrValue.
fn convert_attribute_value(
    attr: &hdf5_reader::Attribute,
    metadata_mode: crate::NcMetadataMode,
) -> Result<Option<NcAttrValue>> {
    let strict = metadata_mode == crate::NcMetadataMode::Strict;

    let unsupported = || {
        Error::InvalidData(format!(
            "attribute '{}' uses unsupported NetCDF-4 datatype {:?}",
            attr.name, attr.datatype
        ))
    };

    let read_attr = |result: hdf5_reader::error::Result<NcAttrValue>| match result {
        Ok(value) => Ok(Some(value)),
        Err(err) if strict => Err(Error::InvalidData(format!(
            "attribute '{}' could not be decoded: {err}",
            attr.name
        ))),
        Err(_) => Ok(None),
    };

    match &attr.datatype {
        Datatype::FixedPoint { size, signed, .. } => match (size, signed) {
            (1, true) => read_attr(attr.read_1d::<i8>().map(NcAttrValue::Bytes)),
            (1, false) => read_attr(attr.read_1d::<u8>().map(NcAttrValue::UBytes)),
            (2, true) => read_attr(attr.read_1d::<i16>().map(NcAttrValue::Shorts)),
            (2, false) => read_attr(attr.read_1d::<u16>().map(NcAttrValue::UShorts)),
            (4, true) => read_attr(attr.read_1d::<i32>().map(NcAttrValue::Ints)),
            (4, false) => read_attr(attr.read_1d::<u32>().map(NcAttrValue::UInts)),
            (8, true) => read_attr(attr.read_1d::<i64>().map(NcAttrValue::Int64s)),
            (8, false) => read_attr(attr.read_1d::<u64>().map(NcAttrValue::UInt64s)),
            _ if strict => Err(unsupported()),
            _ => Ok(None),
        },
        Datatype::FloatingPoint { size, .. } => match size {
            4 => read_attr(attr.read_1d::<f32>().map(NcAttrValue::Floats)),
            8 => read_attr(attr.read_1d::<f64>().map(NcAttrValue::Doubles)),
            _ if strict => Err(unsupported()),
            _ => Ok(None),
        },
        Datatype::String { .. } => {
            if attr.shape.is_empty() {
                read_attr(attr.read_string().map(NcAttrValue::Chars))
            } else {
                read_attr(attr.read_strings().map(NcAttrValue::Strings))
            }
        }
        Datatype::VarLen {
            base,
            kind: VarLenKind::String,
            ..
        } if base.is_vlen_string_base() => {
            if attr.shape.is_empty() {
                read_attr(attr.read_string().map(NcAttrValue::Chars))
            } else {
                read_attr(attr.read_strings().map(NcAttrValue::Strings))
            }
        }
        _ if strict => Err(unsupported()),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use hdf5_reader::Attribute;
    use hdf5_reader::ByteOrder;

    use super::*;

    #[test]
    fn strict_mode_rejects_unsupported_attribute_types() {
        let attr = Attribute {
            name: "opaque_attr".to_string(),
            datatype: Datatype::Opaque {
                size: 4,
                tag: "test".to_string(),
            },
            shape: vec![1],
            raw_data: vec![0, 0, 0, 0],
            decoded_strings: None,
        };

        let err = convert_attribute_value(&attr, crate::NcMetadataMode::Strict).unwrap_err();
        assert!(matches!(err, Error::InvalidData(_)));
    }

    #[test]
    fn lossy_mode_skips_unsupported_attribute_types() {
        let attr = Attribute {
            name: "bad_int".to_string(),
            datatype: Datatype::FixedPoint {
                size: 16,
                signed: true,
                byte_order: ByteOrder::LittleEndian,
            },
            shape: vec![1],
            raw_data: vec![0; 16],
            decoded_strings: None,
        };

        assert!(convert_attribute_value(&attr, crate::NcMetadataMode::Lossy)
            .unwrap()
            .is_none());
    }
}
