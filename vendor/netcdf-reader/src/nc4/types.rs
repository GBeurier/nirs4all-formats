//! Map HDF5 datatypes to NetCDF-4 types.
//!
//! HDF5 has a richer type system than NetCDF-4. This module maps the subset
//! of HDF5 types that are valid in NetCDF-4 to `NcType`:
//!
//! | HDF5 Datatype                 | NcType   |
//! |-------------------------------|----------|
//! | FixedPoint { size=1, signed } | Byte     |
//! | FixedPoint { size=1, !signed} | UByte    |
//! | FixedPoint { size=2, signed } | Short    |
//! | FixedPoint { size=2, !signed} | UShort   |
//! | FixedPoint { size=4, signed } | Int      |
//! | FixedPoint { size=4, !signed} | UInt     |
//! | FixedPoint { size=8, signed } | Int64    |
//! | FixedPoint { size=8, !signed} | UInt64   |
//! | FloatingPoint { size=4 }      | Float    |
//! | FloatingPoint { size=8 }      | Double   |
//! | String (any)                  | String   |
//! | Enum { base=byte }            | Enum     |
//! | Compound { .. }               | Compound |
//! | Opaque { .. }                 | Opaque   |
//! | Array { base, dims }          | Array    |
//! | VarLen { kind=String, base=u8 } | String*  |
//! | VarLen { base }               | VLen     |
//!
//! * Some NetCDF-4 string variables are stored as HDF5 vlen bytes.

use hdf5_reader::messages::datatype::{Datatype, StringSize, VarLenKind};
use hdf5_reader::ByteOrder;

use crate::error::{Error, Result};
use crate::types::{NcCompoundField, NcEnumMember, NcIntegerValue, NcType};

/// Map an HDF5 datatype to a NetCDF type.
pub fn hdf5_to_nc_type(dtype: &Datatype) -> Result<NcType> {
    match dtype {
        Datatype::FixedPoint { size, signed, .. } => match (size, signed) {
            (1, true) => Ok(NcType::Byte),
            (1, false) => Ok(NcType::UByte),
            (2, true) => Ok(NcType::Short),
            (2, false) => Ok(NcType::UShort),
            (4, true) => Ok(NcType::Int),
            (4, false) => Ok(NcType::UInt),
            (8, true) => Ok(NcType::Int64),
            (8, false) => Ok(NcType::UInt64),
            _ => Err(Error::InvalidData(format!(
                "unsupported HDF5 integer size {} for NetCDF-4",
                size
            ))),
        },
        Datatype::FloatingPoint { size, .. } => match size {
            4 => Ok(NcType::Float),
            8 => Ok(NcType::Double),
            _ => Err(Error::InvalidData(format!(
                "unsupported HDF5 float size {} for NetCDF-4",
                size
            ))),
        },
        Datatype::String {
            size: StringSize::Fixed(1),
            ..
        } => Ok(NcType::Char),
        Datatype::String { .. } => Ok(NcType::String),
        Datatype::Enum { base, members } => Ok(NcType::Enum {
            base: Box::new(hdf5_to_nc_type(base)?),
            members: members
                .iter()
                .map(|member| {
                    Ok(NcEnumMember {
                        name: member.name.clone(),
                        value: decode_enum_integer(base, &member.value)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        }),
        Datatype::Compound { size, fields } => {
            let mut nc_fields = Vec::with_capacity(fields.len());
            for f in fields {
                nc_fields.push(NcCompoundField {
                    name: f.name.clone(),
                    offset: f.byte_offset as u64,
                    dtype: hdf5_to_nc_type(&f.datatype)?,
                });
            }
            Ok(NcType::Compound {
                size: *size,
                fields: nc_fields,
            })
        }
        Datatype::Opaque { size, tag } => Ok(NcType::Opaque {
            size: *size,
            tag: tag.clone(),
        }),
        Datatype::Array { base, dims } => {
            let base_nc = hdf5_to_nc_type(base)?;
            Ok(NcType::Array {
                base: Box::new(base_nc),
                dims: dims.clone(),
            })
        }
        Datatype::VarLen {
            base,
            kind: VarLenKind::String,
            ..
        } if base.is_vlen_string_base() => Ok(NcType::String),
        Datatype::VarLen { base, .. } => {
            let base_nc = hdf5_to_nc_type(base)?;
            Ok(NcType::VLen {
                base: Box::new(base_nc),
            })
        }
        _ => Err(Error::InvalidData(format!(
            "HDF5 datatype {:?} has no NetCDF-4 equivalent",
            dtype
        ))),
    }
}

pub(crate) fn decode_enum_integer(base: &Datatype, bytes: &[u8]) -> Result<NcIntegerValue> {
    match base {
        Datatype::FixedPoint {
            size,
            signed,
            byte_order,
        } => decode_fixed_point_integer(bytes, *size, *signed, *byte_order),
        other => Err(Error::InvalidData(format!(
            "NetCDF-4 enum base type must be integer, got {other:?}"
        ))),
    }
}

pub(crate) fn decode_fixed_point_integer(
    bytes: &[u8],
    size: u8,
    signed: bool,
    byte_order: ByteOrder,
) -> Result<NcIntegerValue> {
    fn read<const N: usize>(bytes: &[u8], byte_order: ByteOrder) -> Result<[u8; N]> {
        if bytes.len() < N {
            return Err(Error::InvalidData(format!(
                "integer value too short: need {} bytes, have {}",
                N,
                bytes.len()
            )));
        }
        let mut out = [0u8; N];
        out.copy_from_slice(&bytes[..N]);
        #[cfg(target_endian = "little")]
        if byte_order == ByteOrder::BigEndian {
            out.reverse();
        }
        #[cfg(target_endian = "big")]
        if byte_order == ByteOrder::LittleEndian {
            out.reverse();
        }
        Ok(out)
    }

    match (size, signed) {
        (1, true) => Ok(NcIntegerValue::I8(i8::from_ne_bytes(read::<1>(
            bytes, byte_order,
        )?))),
        (1, false) => Ok(NcIntegerValue::U8(u8::from_ne_bytes(read::<1>(
            bytes, byte_order,
        )?))),
        (2, true) => Ok(NcIntegerValue::I16(i16::from_ne_bytes(read::<2>(
            bytes, byte_order,
        )?))),
        (2, false) => Ok(NcIntegerValue::U16(u16::from_ne_bytes(read::<2>(
            bytes, byte_order,
        )?))),
        (4, true) => Ok(NcIntegerValue::I32(i32::from_ne_bytes(read::<4>(
            bytes, byte_order,
        )?))),
        (4, false) => Ok(NcIntegerValue::U32(u32::from_ne_bytes(read::<4>(
            bytes, byte_order,
        )?))),
        (8, true) => Ok(NcIntegerValue::I64(i64::from_ne_bytes(read::<8>(
            bytes, byte_order,
        )?))),
        (8, false) => Ok(NcIntegerValue::U64(u64::from_ne_bytes(read::<8>(
            bytes, byte_order,
        )?))),
        _ => Err(Error::InvalidData(format!(
            "unsupported NetCDF-4 enum integer size {size}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hdf5_reader::error::ByteOrder;

    #[test]
    fn integer_types() {
        let bo = ByteOrder::LittleEndian;
        assert_eq!(
            hdf5_to_nc_type(&Datatype::FixedPoint {
                size: 1,
                signed: true,
                byte_order: bo
            })
            .unwrap(),
            NcType::Byte
        );
        assert_eq!(
            hdf5_to_nc_type(&Datatype::FixedPoint {
                size: 1,
                signed: false,
                byte_order: bo
            })
            .unwrap(),
            NcType::UByte
        );
        assert_eq!(
            hdf5_to_nc_type(&Datatype::FixedPoint {
                size: 4,
                signed: true,
                byte_order: bo
            })
            .unwrap(),
            NcType::Int
        );
        assert_eq!(
            hdf5_to_nc_type(&Datatype::FixedPoint {
                size: 8,
                signed: false,
                byte_order: bo
            })
            .unwrap(),
            NcType::UInt64
        );
    }

    #[test]
    fn float_types() {
        let bo = ByteOrder::LittleEndian;
        assert_eq!(
            hdf5_to_nc_type(&Datatype::FloatingPoint {
                size: 4,
                byte_order: bo
            })
            .unwrap(),
            NcType::Float
        );
        assert_eq!(
            hdf5_to_nc_type(&Datatype::FloatingPoint {
                size: 8,
                byte_order: bo
            })
            .unwrap(),
            NcType::Double
        );
    }

    #[test]
    fn varlen_u8_maps_to_string() {
        let bo = ByteOrder::LittleEndian;
        assert_eq!(
            hdf5_to_nc_type(&Datatype::VarLen {
                base: Box::new(Datatype::FixedPoint {
                    size: 1,
                    signed: false,
                    byte_order: bo,
                }),
                kind: VarLenKind::String,
                encoding: hdf5_reader::StringEncoding::Utf8,
                padding: hdf5_reader::StringPadding::NullTerminate,
            })
            .unwrap(),
            NcType::String
        );
    }

    #[test]
    fn fixed_one_byte_string_maps_to_char() {
        assert_eq!(
            hdf5_to_nc_type(&Datatype::String {
                size: StringSize::Fixed(1),
                encoding: hdf5_reader::StringEncoding::Ascii,
                padding: hdf5_reader::StringPadding::NullPad,
            })
            .unwrap(),
            NcType::Char
        );
    }

    #[test]
    fn sequence_varlen_u8_maps_to_vlen() {
        let bo = ByteOrder::LittleEndian;
        assert_eq!(
            hdf5_to_nc_type(&Datatype::VarLen {
                base: Box::new(Datatype::FixedPoint {
                    size: 1,
                    signed: false,
                    byte_order: bo,
                }),
                kind: VarLenKind::Sequence,
                encoding: hdf5_reader::StringEncoding::Ascii,
                padding: hdf5_reader::StringPadding::NullTerminate,
            })
            .unwrap(),
            NcType::VLen {
                base: Box::new(NcType::UByte)
            }
        );
    }
}
