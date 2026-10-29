//! HDF5 Datatype message (type 0x0003).
//!
//! The datatype message describes the type of each element in a dataset.
//! The first 4 bytes encode class (bits 0-3), version (bits 4-7), and
//! class-specific bit flags (bits 8-31). The remaining bytes carry
//! class-specific properties.
//!
//! Supported classes:
//! - 0: Fixed-point (integer)
//! - 1: Floating-point
//! - 2: Time (treated as opaque)
//! - 3: String
//! - 4: Bitfield
//! - 5: Opaque
//! - 6: Compound
//! - 7: Reference
//! - 8: Enum
//! - 9: Variable-length
//! - 10: Array

use crate::error::{ByteOrder, Error, Result};
use crate::io::Cursor;
pub use hdf5_core::{
    CompoundField, Datatype, DatatypeMessage, EnumMember, ReferenceType, StringEncoding,
    StringPadding, StringSize, VarLenKind,
};

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a datatype message starting at the current cursor position.
///
/// The `msg_size` is the total number of bytes allocated for this message
/// (used to skip any trailing padding).
pub fn parse(cursor: &mut Cursor<'_>, msg_size: usize) -> Result<DatatypeMessage> {
    let start = cursor.position();
    let (dt, size) = parse_datatype_description(cursor)?;

    let consumed = (cursor.position() - start) as usize;
    if consumed < msg_size {
        cursor.skip(msg_size - consumed)?;
    }

    Ok(DatatypeMessage { datatype: dt, size })
}

/// Maximum nesting depth for composite datatypes (compound members, enum,
/// variable-length, and array bases). A malformed file cannot recurse the
/// parser into a stack overflow.
const MAX_DATATYPE_NESTING_DEPTH: usize = 32;

/// Parse a single datatype description (the 4-byte header + properties).
///
/// This is also called recursively for compound members, arrays, enums, etc.
pub fn parse_datatype_description(cursor: &mut Cursor<'_>) -> Result<(Datatype, u32)> {
    parse_datatype_description_at_depth(cursor, 0)
}

fn parse_datatype_description_at_depth(
    cursor: &mut Cursor<'_>,
    depth: usize,
) -> Result<(Datatype, u32)> {
    if depth > MAX_DATATYPE_NESTING_DEPTH {
        return Err(Error::InvalidData(format!(
            "datatype nesting exceeds {MAX_DATATYPE_NESTING_DEPTH} levels"
        )));
    }
    let class_and_flags = cursor.read_u32_le()?;
    let class = (class_and_flags & 0x0F) as u8;
    let version = ((class_and_flags >> 4) & 0x0F) as u8;
    let class_flags = class_and_flags >> 8; // upper 24 bits
    let size = cursor.read_u32_le()?;

    let dt = match class {
        0 => parse_fixed_point(cursor, class_flags, size)?,
        1 => parse_floating_point(cursor, class_flags, size)?,
        2 => parse_time(cursor, size)?,
        3 => parse_string(class_flags, size)?,
        4 => parse_bitfield(cursor, class_flags, size)?,
        5 => parse_opaque(cursor, class_flags, size)?,
        6 => parse_compound(cursor, class_flags, size, version, depth)?,
        7 => parse_reference(class_flags, size)?,
        8 => parse_enum(cursor, class_flags, size, depth)?,
        9 => parse_varlen(cursor, class_flags, size, depth)?,
        10 => parse_array(cursor, size, version, depth)?,
        c => return Err(Error::UnsupportedDatatypeClass(c)),
    };

    Ok((dt, size))
}

// ---------------------------------------------------------------------------
// Class 0: Fixed-point (integer)
// ---------------------------------------------------------------------------

fn parse_fixed_point(cursor: &mut Cursor<'_>, flags: u32, size: u32) -> Result<Datatype> {
    // Bit 0 of class flags: byte order (0 = LE, 1 = BE)
    let byte_order = if (flags & 0x01) != 0 {
        ByteOrder::BigEndian
    } else {
        ByteOrder::LittleEndian
    };
    // Bit 3: signed (0 = unsigned, 1 = signed)
    let signed = (flags & 0x08) != 0;

    // Properties: bit offset (u16) + bit precision (u16)
    let _bit_offset = cursor.read_u16_le()?;
    let _bit_precision = cursor.read_u16_le()?;

    Ok(Datatype::FixedPoint {
        size: size as u8,
        signed,
        byte_order,
    })
}

// ---------------------------------------------------------------------------
// Class 1: Floating-point
// ---------------------------------------------------------------------------

fn parse_floating_point(cursor: &mut Cursor<'_>, flags: u32, size: u32) -> Result<Datatype> {
    // Byte order: bit 0 low-order, bit 6 high-order
    //   00 = LE, 01 = BE, 10 = VAX (treated as LE for our purposes)
    let bo_lo = flags & 0x01;
    let bo_hi = (flags >> 6) & 0x01;
    let byte_order = match (bo_hi, bo_lo) {
        (0, 0) => ByteOrder::LittleEndian,
        (0, 1) => ByteOrder::BigEndian,
        // VAX order — map to little endian (close enough for decoding)
        _ => ByteOrder::LittleEndian,
    };

    // Properties: 12 bytes
    // bit offset (u16), bit precision (u16), exponent location (u8),
    // exponent size (u8), mantissa location (u8), mantissa size (u8),
    // exponent bias (u32)
    let _bit_offset = cursor.read_u16_le()?;
    let _bit_precision = cursor.read_u16_le()?;
    let _exp_location = cursor.read_u8()?;
    let _exp_size = cursor.read_u8()?;
    let _mant_location = cursor.read_u8()?;
    let _mant_size = cursor.read_u8()?;
    let _exp_bias = cursor.read_u32_le()?;

    Ok(Datatype::FloatingPoint {
        size: size as u8,
        byte_order,
    })
}

// ---------------------------------------------------------------------------
// Class 2: Time (rarely used, treat as opaque)
// ---------------------------------------------------------------------------

fn parse_time(cursor: &mut Cursor<'_>, size: u32) -> Result<Datatype> {
    // Properties: bit precision (u16)
    let _bit_precision = cursor.read_u16_le()?;
    Ok(Datatype::Opaque {
        size,
        tag: "HDF5_TIME".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Class 3: String
// ---------------------------------------------------------------------------

fn parse_string(flags: u32, size: u32) -> Result<Datatype> {
    // Bits 0-3: padding type
    let padding = string_padding_from_bits(flags & 0x0F);

    // Bits 4-7: character set
    let encoding = string_encoding_from_bits((flags >> 4) & 0x0F);

    // No additional property bytes for string class.

    let string_size = if size == 0 {
        // Size 0 can indicate variable-length when used with vlen wrapper,
        // but for the string class itself we treat it as Variable.
        StringSize::Variable
    } else {
        StringSize::Fixed(size)
    };

    Ok(Datatype::String {
        size: string_size,
        encoding,
        padding,
    })
}

// ---------------------------------------------------------------------------
// Class 4: Bitfield
// ---------------------------------------------------------------------------

fn parse_bitfield(cursor: &mut Cursor<'_>, flags: u32, size: u32) -> Result<Datatype> {
    let byte_order = if (flags & 0x01) != 0 {
        ByteOrder::BigEndian
    } else {
        ByteOrder::LittleEndian
    };

    // Properties: bit offset (u16) + bit precision (u16)
    let _bit_offset = cursor.read_u16_le()?;
    let _bit_precision = cursor.read_u16_le()?;

    Ok(Datatype::Bitfield {
        size: size as u8,
        byte_order,
    })
}

// ---------------------------------------------------------------------------
// Class 5: Opaque
// ---------------------------------------------------------------------------

fn parse_opaque(cursor: &mut Cursor<'_>, flags: u32, size: u32) -> Result<Datatype> {
    // The class flags encode the length of the tag (in the lower bits).
    let tag_len = (flags & 0xFF) as usize;

    let tag = if tag_len > 0 {
        let tag_bytes = cursor.read_bytes(tag_len)?;
        // Trim trailing nulls
        let end = tag_bytes.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        String::from_utf8_lossy(&tag_bytes[..end]).into_owned()
    } else {
        String::new()
    };

    // Pad to 8-byte alignment
    let padded = (tag_len + 7) & !7;
    if padded > tag_len {
        cursor.skip(padded - tag_len)?;
    }

    Ok(Datatype::Opaque { size, tag })
}

// ---------------------------------------------------------------------------
// Class 6: Compound
// ---------------------------------------------------------------------------

fn parse_compound(
    cursor: &mut Cursor<'_>,
    flags: u32,
    size: u32,
    version: u8,
    depth: usize,
) -> Result<Datatype> {
    // Lower 16 bits of class flags = number of members
    let n_members = (flags & 0xFFFF) as usize;
    let byte_offset_size = compound_member_offset_size(size);

    let mut fields = Vec::with_capacity(n_members);

    for _ in 0..n_members {
        let name_start = cursor.position();
        let name = cursor.read_null_terminated_string()?;

        if version < 3 {
            // V1/V2: the name, null terminator included, is padded to a multiple of
            // eight bytes counted from the start of the name, not from the start of the
            // buffer. A member whose datatype description is not a multiple of eight
            // bytes long (a fixed-length string's is 8, a float's 20) leaves the next
            // name off the buffer's 8-byte grid.
            let used = (cursor.position() - name_start) as usize;
            cursor.skip(used.next_multiple_of(8) - used)?;
        }

        let byte_offset = if version == 1 {
            // V1: byte offset is `size of offsets` (4 bytes)
            cursor.read_u32_le()?
        } else if version >= 3 {
            cursor.read_uvar(byte_offset_size)? as u32
        } else {
            // V2/V3: byte offset is 4 bytes
            cursor.read_u32_le()?
        };

        if version == 1 {
            // V1: dimensionality (1 byte), reserved (3 bytes), dim perm (4 bytes),
            // reserved (4 bytes), dim sizes (4 * 4 = 16 bytes)
            let _dimensionality = cursor.read_u8()?;
            cursor.skip(3)?; // reserved
            cursor.skip(4)?; // dimension permutation
            cursor.skip(4)?; // reserved
            cursor.skip(16)?; // 4 dimension sizes (each u32)
        }

        let (member_dt, _member_size) = parse_datatype_description_at_depth(cursor, depth + 1)?;

        fields.push(CompoundField {
            name,
            byte_offset,
            datatype: member_dt,
        });
    }

    Ok(Datatype::Compound { size, fields })
}

fn compound_member_offset_size(size: u32) -> usize {
    match size {
        0..=0xFF => 1,
        0x100..=0xFFFF => 2,
        0x1_0000..=0xFF_FFFF => 3,
        _ => 4,
    }
}

// ---------------------------------------------------------------------------
// Class 7: Reference
// ---------------------------------------------------------------------------

fn parse_reference(flags: u32, size: u32) -> Result<Datatype> {
    // Bit 0-3: reference type (0 = object, 1 = dataset region)
    let ref_type = match flags & 0x0F {
        0 => ReferenceType::Object,
        1 => ReferenceType::DatasetRegion,
        _ => ReferenceType::Object,
    };

    // No property bytes for reference class.

    Ok(Datatype::Reference {
        ref_type,
        size: size as u8,
    })
}

// ---------------------------------------------------------------------------
// Class 8: Enum
// ---------------------------------------------------------------------------

fn parse_enum(cursor: &mut Cursor<'_>, flags: u32, size: u32, depth: usize) -> Result<Datatype> {
    let n_members = (flags & 0xFFFF) as usize;

    // Base type
    let (base_dt, _base_size) = parse_datatype_description_at_depth(cursor, depth + 1)?;

    // Member names (null-terminated)
    let mut names = Vec::with_capacity(n_members);
    for _ in 0..n_members {
        names.push(cursor.read_null_terminated_string()?);
    }

    // Member values (each is `size` bytes, matching the base type size)
    let member_value_size = size as usize;
    let mut members = Vec::with_capacity(n_members);
    for name in names {
        let value = cursor.read_bytes(member_value_size)?.to_vec();
        members.push(EnumMember { name, value });
    }

    Ok(Datatype::Enum {
        base: Box::new(base_dt),
        members,
    })
}

// ---------------------------------------------------------------------------
// Class 9: Variable-length
// ---------------------------------------------------------------------------

fn parse_varlen(cursor: &mut Cursor<'_>, flags: u32, _size: u32, depth: usize) -> Result<Datatype> {
    // Bits 0-3: type (0 = sequence, 1 = string)
    let kind = match flags & 0x0F {
        0 => VarLenKind::Sequence,
        1 => VarLenKind::String,
        other => VarLenKind::Unknown(other as u8),
    };
    // Bits 4-7: padding type (for strings)
    let padding = string_padding_from_bits((flags >> 4) & 0x0F);
    // Bits 8-11: character set (for strings)
    let encoding = string_encoding_from_bits((flags >> 8) & 0x0F);

    // Base type follows
    let (base_dt, _base_size) = parse_datatype_description_at_depth(cursor, depth + 1)?;

    Ok(Datatype::VarLen {
        base: Box::new(base_dt),
        kind,
        encoding,
        padding,
    })
}

fn string_padding_from_bits(bits: u32) -> StringPadding {
    match bits {
        0 => StringPadding::NullTerminate,
        1 => StringPadding::NullPad,
        2 => StringPadding::SpacePad,
        _ => StringPadding::NullTerminate,
    }
}

fn string_encoding_from_bits(bits: u32) -> StringEncoding {
    match bits {
        0 => StringEncoding::Ascii,
        1 => StringEncoding::Utf8,
        _ => StringEncoding::Ascii,
    }
}

// ---------------------------------------------------------------------------
// Class 10: Array
// ---------------------------------------------------------------------------

fn parse_array(cursor: &mut Cursor<'_>, _size: u32, version: u8, depth: usize) -> Result<Datatype> {
    let rank = cursor.read_u8()? as usize;

    if version < 3 {
        // Version 1/2: 3 reserved bytes after rank
        cursor.skip(3)?;
    }

    let mut dims = Vec::with_capacity(rank);
    for _ in 0..rank {
        dims.push(cursor.read_u32_le()? as u64);
    }

    if version < 3 {
        // Version 1: permutation indices (rank * u32) — skip them
        cursor.skip(rank * 4)?;
    }

    // Base type
    let (base_dt, _base_size) = parse_datatype_description_at_depth(cursor, depth + 1)?;

    Ok(Datatype::Array {
        base: Box::new(base_dt),
        dims,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build the 4-byte class+version+flags word.
    fn class_word(class: u8, version: u8, flags: u32) -> u32 {
        (class as u32) | ((version as u32) << 4) | (flags << 8)
    }

    /// Encode `levels` nested v3 array datatypes around a 4-byte fixed-point
    /// base.
    fn nested_array_datatype(levels: usize) -> Vec<u8> {
        let mut data = Vec::new();
        for _ in 0..levels {
            data.extend_from_slice(&class_word(10, 3, 0x00).to_le_bytes());
            data.extend_from_slice(&4u32.to_le_bytes());
            data.push(1); // rank
            data.extend_from_slice(&2u32.to_le_bytes()); // dim
        }
        // Fixed-point base.
        data.extend_from_slice(&class_word(0, 1, 0x00).to_le_bytes());
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&32u16.to_le_bytes());
        data
    }

    #[test]
    fn deeply_nested_datatype_is_rejected() {
        let data = nested_array_datatype(64);
        let mut cursor = Cursor::new(&data);
        let err = parse(&mut cursor, data.len()).unwrap_err();
        assert!(
            err.to_string().contains("nesting exceeds"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn moderately_nested_datatype_parses() {
        let data = nested_array_datatype(4);
        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, data.len()).unwrap();
        let mut dt = &msg.datatype;
        for _ in 0..4 {
            match dt {
                Datatype::Array { base, dims } => {
                    assert_eq!(dims.as_slice(), &[2]);
                    dt = base;
                }
                other => panic!("expected Array, got {other:?}"),
            }
        }
        assert!(matches!(dt, Datatype::FixedPoint { size: 4, .. }));
    }

    #[test]
    fn parse_u32_le() {
        let mut data = Vec::new();
        // class=0 (fixed-point), version=1, flags: bit0=0 (LE), bit3=0 (unsigned)
        data.extend_from_slice(&class_word(0, 1, 0x00).to_le_bytes());
        // size = 4
        data.extend_from_slice(&4u32.to_le_bytes());
        // properties: bit offset=0, bit precision=32
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&32u16.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, data.len()).unwrap();
        assert_eq!(msg.size, 4);
        match &msg.datatype {
            Datatype::FixedPoint {
                size,
                signed,
                byte_order,
            } => {
                assert_eq!(*size, 4);
                assert!(!*signed);
                assert_eq!(*byte_order, ByteOrder::LittleEndian);
            }
            other => panic!("expected FixedPoint, got {:?}", other),
        }
    }

    #[test]
    fn parse_i64_be() {
        let mut data = Vec::new();
        // class=0 (fixed-point), version=1, flags: bit0=1 (BE), bit3=1 (signed)
        data.extend_from_slice(&class_word(0, 1, 0x09).to_le_bytes());
        // size = 8
        data.extend_from_slice(&8u32.to_le_bytes());
        // properties: bit offset=0, bit precision=64
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&64u16.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, data.len()).unwrap();
        assert_eq!(msg.size, 8);
        match &msg.datatype {
            Datatype::FixedPoint {
                size,
                signed,
                byte_order,
            } => {
                assert_eq!(*size, 8);
                assert!(*signed);
                assert_eq!(*byte_order, ByteOrder::BigEndian);
            }
            other => panic!("expected FixedPoint, got {:?}", other),
        }
    }

    #[test]
    fn parse_f32_le() {
        let mut data = Vec::new();
        // class=1 (float), version=1, flags: bit0=0 (LE), bit6=0
        data.extend_from_slice(&class_word(1, 1, 0x20).to_le_bytes());
        // size = 4
        data.extend_from_slice(&4u32.to_le_bytes());
        // properties: bit offset=0, bit precision=32
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&32u16.to_le_bytes());
        // exp location=23, exp size=8
        data.push(23);
        data.push(8);
        // mant location=0, mant size=23
        data.push(0);
        data.push(23);
        // exp bias=127
        data.extend_from_slice(&127u32.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, data.len()).unwrap();
        assert_eq!(msg.size, 4);
        match &msg.datatype {
            Datatype::FloatingPoint { size, byte_order } => {
                assert_eq!(*size, 4);
                assert_eq!(*byte_order, ByteOrder::LittleEndian);
            }
            other => panic!("expected FloatingPoint, got {:?}", other),
        }
    }

    #[test]
    fn parse_f64_be() {
        let mut data = Vec::new();
        // class=1 (float), version=1, flags: bit0=1 (BE), bit6=0
        data.extend_from_slice(&class_word(1, 1, 0x01).to_le_bytes());
        // size = 8
        data.extend_from_slice(&8u32.to_le_bytes());
        // properties
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&64u16.to_le_bytes());
        data.push(52);
        data.push(11);
        data.push(0);
        data.push(52);
        data.extend_from_slice(&1023u32.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, data.len()).unwrap();
        assert_eq!(msg.size, 8);
        match &msg.datatype {
            Datatype::FloatingPoint { size, byte_order } => {
                assert_eq!(*size, 8);
                assert_eq!(*byte_order, ByteOrder::BigEndian);
            }
            other => panic!("expected FloatingPoint, got {:?}", other),
        }
    }

    #[test]
    fn parse_string_fixed_ascii() {
        let mut data = Vec::new();
        // class=3 (string), version=1, flags: padding=0 (null-term), charset=0 (ascii)
        data.extend_from_slice(&class_word(3, 1, 0x00).to_le_bytes());
        // size = 32
        data.extend_from_slice(&32u32.to_le_bytes());
        // No property bytes for strings.

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, data.len()).unwrap();
        assert_eq!(msg.size, 32);
        match &msg.datatype {
            Datatype::String {
                size,
                encoding,
                padding,
            } => {
                assert_eq!(*size, StringSize::Fixed(32));
                assert_eq!(*encoding, StringEncoding::Ascii);
                assert_eq!(*padding, StringPadding::NullTerminate);
            }
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn parse_string_utf8_space_pad() {
        let mut data = Vec::new();
        // class=3, version=1, flags: padding=2 (space-pad), charset=1 (utf8)
        // padding bits 0-3 = 2, charset bits 4-7 = 1
        let flags: u32 = 0x02 | (0x01 << 4);
        data.extend_from_slice(&class_word(3, 1, flags).to_le_bytes());
        data.extend_from_slice(&16u32.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, data.len()).unwrap();
        match &msg.datatype {
            Datatype::String {
                size,
                encoding,
                padding,
            } => {
                assert_eq!(*size, StringSize::Fixed(16));
                assert_eq!(*encoding, StringEncoding::Utf8);
                assert_eq!(*padding, StringPadding::SpacePad);
            }
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn parse_varlen_string_preserves_flags() {
        let mut data = Vec::new();
        // class=9, version=1, flags: kind=string, padding=space-pad, charset=utf8
        let flags: u32 = 0x01 | (0x02 << 4) | (0x01 << 8);
        data.extend_from_slice(&class_word(9, 1, flags).to_le_bytes());
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&class_word(0, 1, 0x00).to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&8u16.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, data.len()).unwrap();
        match &msg.datatype {
            Datatype::VarLen {
                base,
                kind,
                encoding,
                padding,
            } => {
                assert_eq!(*kind, VarLenKind::String);
                assert_eq!(*encoding, StringEncoding::Utf8);
                assert_eq!(*padding, StringPadding::SpacePad);
                assert!(matches!(
                    base.as_ref(),
                    Datatype::FixedPoint {
                        size: 1,
                        signed: false,
                        byte_order: ByteOrder::LittleEndian
                    }
                ));
            }
            other => panic!("expected VarLen, got {:?}", other),
        }
    }

    #[test]
    fn parse_varlen_sequence_preserves_kind() {
        let mut data = Vec::new();
        data.extend_from_slice(&class_word(9, 1, 0x00).to_le_bytes());
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&class_word(0, 1, 0x00).to_le_bytes());
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&32u16.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, data.len()).unwrap();
        match &msg.datatype {
            Datatype::VarLen { kind, base, .. } => {
                assert_eq!(*kind, VarLenKind::Sequence);
                assert!(matches!(
                    base.as_ref(),
                    Datatype::FixedPoint {
                        size: 4,
                        signed: false,
                        byte_order: ByteOrder::LittleEndian
                    }
                ));
            }
            other => panic!("expected VarLen, got {:?}", other),
        }
    }

    #[test]
    fn parse_reference_object() {
        let mut data = Vec::new();
        // class=7, version=1, flags: ref_type=0 (object)
        data.extend_from_slice(&class_word(7, 1, 0x00).to_le_bytes());
        data.extend_from_slice(&8u32.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, data.len()).unwrap();
        match &msg.datatype {
            Datatype::Reference { ref_type, size } => {
                assert_eq!(*ref_type, ReferenceType::Object);
                assert_eq!(*size, 8);
            }
            other => panic!("expected Reference, got {:?}", other),
        }
    }

    #[test]
    fn parse_reference_region() {
        let mut data = Vec::new();
        // class=7, version=1, flags: ref_type=1 (dataset region)
        data.extend_from_slice(&class_word(7, 1, 0x01).to_le_bytes());
        data.extend_from_slice(&12u32.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, data.len()).unwrap();
        match &msg.datatype {
            Datatype::Reference { ref_type, size } => {
                assert_eq!(*ref_type, ReferenceType::DatasetRegion);
                assert_eq!(*size, 12);
            }
            other => panic!("expected Reference, got {:?}", other),
        }
    }

    /// A version 1 compound member: its name padded to a multiple of eight bytes, the
    /// byte offset, then dimensionality, reserved, permutation, reserved and four
    /// dimension sizes, then the member's own datatype description.
    fn compound_v1_member(data: &mut Vec<u8>, name: &str, offset: u32, member: &[u8]) {
        let mut padded = name.as_bytes().to_vec();
        padded.push(0);
        padded.resize(padded.len().next_multiple_of(8), 0);
        data.extend_from_slice(&padded);
        data.extend_from_slice(&offset.to_le_bytes());
        data.extend_from_slice(&[0u8; 28]);
        data.extend_from_slice(member);
    }

    #[test]
    fn parse_compound_v1_pads_names_relative_to_their_start() {
        // A float description is 20 bytes and a fixed string's is 8, so after the first
        // two members the third name starts four bytes off the buffer's 8-byte grid
        // (as in HEC-RAS attribute tables). Its padding must still be counted from the
        // start of the name.
        let mut float = Vec::new();
        float.extend_from_slice(&class_word(1, 1, 0x20).to_le_bytes());
        float.extend_from_slice(&4u32.to_le_bytes());
        float.extend_from_slice(&0u16.to_le_bytes());
        float.extend_from_slice(&32u16.to_le_bytes());
        float.extend_from_slice(&[23, 8, 0, 23]);
        float.extend_from_slice(&127u32.to_le_bytes());
        let mut string = Vec::new();
        string.extend_from_slice(&class_word(3, 1, 0x00).to_le_bytes());
        string.extend_from_slice(&16u32.to_le_bytes());
        let mut int = Vec::new();
        int.extend_from_slice(&class_word(0, 1, 0x08).to_le_bytes());
        int.extend_from_slice(&4u32.to_le_bytes());
        int.extend_from_slice(&0u16.to_le_bytes());
        int.extend_from_slice(&32u16.to_le_bytes());

        let mut data = Vec::new();
        data.extend_from_slice(&class_word(6, 1, 3).to_le_bytes());
        data.extend_from_slice(&24u32.to_le_bytes());
        compound_v1_member(&mut data, "x", 0, &float);
        compound_v1_member(&mut data, "name", 4, &string);
        compound_v1_member(&mut data, "count", 20, &int);

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, data.len()).unwrap();
        match &msg.datatype {
            Datatype::Compound { size, fields } => {
                assert_eq!(*size, 24);
                let names: Vec<_> = fields
                    .iter()
                    .map(|f| (f.name.as_str(), f.byte_offset))
                    .collect();
                assert_eq!(names, [("x", 0), ("name", 4), ("count", 20)]);
                assert!(matches!(
                    fields[2].datatype,
                    Datatype::FixedPoint {
                        size: 4,
                        signed: true,
                        ..
                    }
                ));
            }
            other => panic!("expected Compound, got {:?}", other),
        }
        assert_eq!(cursor.position() as usize, data.len());
    }

    #[test]
    fn parse_compound_v3_variable_member_offsets() {
        let mut data = Vec::new();
        data.extend_from_slice(&class_word(6, 3, 2).to_le_bytes());
        data.extend_from_slice(&16u32.to_le_bytes());

        data.extend_from_slice(b"dataset\0");
        data.push(0x00);
        data.extend_from_slice(&class_word(7, 1, 0x00).to_le_bytes());
        data.extend_from_slice(&8u32.to_le_bytes());

        data.extend_from_slice(b"dimension\0");
        data.push(0x08);
        data.extend_from_slice(&class_word(0, 1, 0x00).to_le_bytes());
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&32u16.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, data.len()).unwrap();
        match &msg.datatype {
            Datatype::Compound { size, fields } => {
                assert_eq!(*size, 16);
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "dataset");
                assert_eq!(fields[0].byte_offset, 0);
                assert_eq!(fields[1].name, "dimension");
                assert_eq!(fields[1].byte_offset, 8);
            }
            other => panic!("expected Compound, got {:?}", other),
        }
    }

    #[test]
    fn parse_enum_u8() {
        let mut data = Vec::new();
        // class=8 (enum), version=3, flags: n_members=2
        data.extend_from_slice(&class_word(8, 3, 2).to_le_bytes());
        // size = 1
        data.extend_from_slice(&1u32.to_le_bytes());

        // Base type: u8 (class=0, version=1, flags=0, size=1, props: offset=0 precision=8)
        data.extend_from_slice(&class_word(0, 1, 0).to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&8u16.to_le_bytes());

        // Member names
        data.extend_from_slice(b"OFF\0");
        data.extend_from_slice(b"ON\0");

        // Member values (1 byte each, matching size=1)
        data.push(0x00);
        data.push(0x01);

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, data.len()).unwrap();
        match &msg.datatype {
            Datatype::Enum { base, members } => {
                assert!(matches!(
                    base.as_ref(),
                    Datatype::FixedPoint {
                        size: 1,
                        signed: false,
                        ..
                    }
                ));
                assert_eq!(members.len(), 2);
                assert_eq!(members[0].name, "OFF");
                assert_eq!(members[0].value, vec![0x00]);
                assert_eq!(members[1].name, "ON");
                assert_eq!(members[1].value, vec![0x01]);
            }
            other => panic!("expected Enum, got {:?}", other),
        }
    }

    #[test]
    fn bitfield_datatype_parses_size_and_byte_order() {
        let mut data = Vec::new();
        // class=4 (bitfield), version=1, flags: bit0=0 (LE)
        data.extend_from_slice(&class_word(4, 1, 0x00).to_le_bytes());
        data.extend_from_slice(&2u32.to_le_bytes());
        // properties: bit offset=0, bit precision=16
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&16u16.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, data.len()).unwrap();
        match &msg.datatype {
            Datatype::Bitfield { size, byte_order } => {
                assert_eq!(*size, 2);
                assert_eq!(*byte_order, ByteOrder::LittleEndian);
            }
            other => panic!("expected Bitfield, got {:?}", other),
        }
    }

    #[test]
    fn unsupported_class() {
        let mut data = Vec::new();
        // class=15 (invalid), version=1, flags=0
        data.extend_from_slice(&class_word(15, 1, 0).to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        assert!(parse(&mut cursor, data.len()).is_err());
    }
}
