//! NetCDF data model re-exported from `netcdf-core`.

pub use netcdf_core::{
    NcAttrValue, NcAttribute, NcCompoundField, NcDimension, NcEnumMember, NcFormat, NcGroup,
    NcIntegerValue, NcMetadataMode, NcSliceInfo, NcSliceInfoElem, NcType, NcVariable,
};

pub(crate) fn checked_usize_from_u64(value: u64, context: &str) -> crate::Result<usize> {
    netcdf_core::checked_usize_from_u64(value, context).map_err(Into::into)
}

pub(crate) fn checked_mul_u64(lhs: u64, rhs: u64, context: &str) -> crate::Result<u64> {
    netcdf_core::checked_mul_u64(lhs, rhs, context).map_err(Into::into)
}

pub(crate) fn checked_shape_elements(shape: &[u64], context: &str) -> crate::Result<u64> {
    netcdf_core::checked_shape_elements(shape, context).map_err(Into::into)
}
