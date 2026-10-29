//! NC type mapping and helper functions for the classic (CDF-1/2/5) format.

use crate::error::{Error, Result};
use crate::types::NcType;

// NetCDF classic type codes (from the binary header).
pub const NC_BYTE: u32 = 1;
pub const NC_CHAR: u32 = 2;
pub const NC_SHORT: u32 = 3;
pub const NC_INT: u32 = 4;
pub const NC_FLOAT: u32 = 5;
pub const NC_DOUBLE: u32 = 6;
// CDF-5 extended types.
pub const NC_UBYTE: u32 = 7;
pub const NC_USHORT: u32 = 8;
pub const NC_UINT: u32 = 9;
pub const NC_INT64: u32 = 10;
pub const NC_UINT64: u32 = 11;

/// Convert a classic NC type code to an `NcType`.
pub fn nc_type_from_code(code: u32) -> Result<NcType> {
    match code {
        NC_BYTE => Ok(NcType::Byte),
        NC_CHAR => Ok(NcType::Char),
        NC_SHORT => Ok(NcType::Short),
        NC_INT => Ok(NcType::Int),
        NC_FLOAT => Ok(NcType::Float),
        NC_DOUBLE => Ok(NcType::Double),
        NC_UBYTE => Ok(NcType::UByte),
        NC_USHORT => Ok(NcType::UShort),
        NC_UINT => Ok(NcType::UInt),
        NC_INT64 => Ok(NcType::Int64),
        NC_UINT64 => Ok(NcType::UInt64),
        _ => Err(Error::InvalidData(format!("unknown NC type code {}", code))),
    }
}

/// Size of one element for a classic NC type code.
pub fn nc_type_size(code: u32) -> Result<usize> {
    Ok(nc_type_from_code(code)?.size()?)
}

/// Compute the amount of padding needed to reach a 4-byte boundary.
pub fn padding_to_4(len: usize) -> usize {
    let rem = len % 4;
    if rem == 0 {
        0
    } else {
        4 - rem
    }
}

/// Round up to the next 4-byte boundary.
pub fn pad_to_4(len: usize) -> usize {
    len + padding_to_4(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nc_type_codes_map_supported_values() {
        assert_eq!(nc_type_from_code(1).unwrap(), NcType::Byte);
        assert_eq!(nc_type_from_code(2).unwrap(), NcType::Char);
        assert_eq!(nc_type_from_code(3).unwrap(), NcType::Short);
        assert_eq!(nc_type_from_code(4).unwrap(), NcType::Int);
        assert_eq!(nc_type_from_code(5).unwrap(), NcType::Float);
        assert_eq!(nc_type_from_code(6).unwrap(), NcType::Double);
        assert_eq!(nc_type_from_code(7).unwrap(), NcType::UByte);
        assert_eq!(nc_type_from_code(8).unwrap(), NcType::UShort);
        assert_eq!(nc_type_from_code(9).unwrap(), NcType::UInt);
        assert_eq!(nc_type_from_code(10).unwrap(), NcType::Int64);
        assert_eq!(nc_type_from_code(11).unwrap(), NcType::UInt64);
        assert!(nc_type_from_code(0).is_err());
        assert!(nc_type_from_code(12).is_err());
    }

    #[test]
    fn padding() {
        assert_eq!(padding_to_4(0), 0);
        assert_eq!(padding_to_4(1), 3);
        assert_eq!(padding_to_4(2), 2);
        assert_eq!(padding_to_4(3), 1);
        assert_eq!(padding_to_4(4), 0);
        assert_eq!(padding_to_4(5), 3);
        assert_eq!(pad_to_4(0), 0);
        assert_eq!(pad_to_4(1), 4);
        assert_eq!(pad_to_4(3), 4);
        assert_eq!(pad_to_4(4), 4);
        assert_eq!(pad_to_4(5), 8);
    }
}
