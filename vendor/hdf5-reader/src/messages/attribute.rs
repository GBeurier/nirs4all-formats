//! HDF5 Attribute message (type 0x000C).
//!
//! Attributes are small named data items attached to any HDF5 object.
//! Each attribute has a name, datatype, dataspace, and raw data.

use crate::error::{Error, Result};
use crate::io::Cursor;

use super::dataspace::{self, DataspaceMessage};
use super::datatype::{self, Datatype};

/// Parsed attribute message.
#[derive(Debug, Clone)]
pub struct AttributeMessage {
    /// Attribute name.
    pub name: String,
    /// Datatype of the attribute value.
    pub datatype: Datatype,
    /// Dataspace describing the shape of the attribute data.
    pub dataspace: DataspaceMessage,
    /// Raw attribute data (un-decoded bytes).
    pub raw_data: Vec<u8>,
}

/// Parse an attribute message.
///
/// Attribute messages embed inline datatype and dataspace descriptions
/// followed by the raw data bytes.
pub fn parse(
    cursor: &mut Cursor<'_>,
    offset_size: u8,
    length_size: u8,
    msg_size: usize,
) -> Result<AttributeMessage> {
    let start = cursor.position();
    let version = cursor.read_u8()?;

    let result = match version {
        1 => parse_v1(cursor, offset_size, length_size),
        2 => parse_v2(cursor, offset_size, length_size),
        3 => parse_v3(cursor, offset_size, length_size),
        v => Err(Error::UnsupportedAttributeVersion(v)),
    };

    result.and_then(|msg| {
        let consumed = (cursor.position() - start) as usize;
        if consumed < msg_size {
            cursor.skip(msg_size - consumed)?;
        }
        Ok(msg)
    })
}

fn parse_v1(cursor: &mut Cursor<'_>, offset_size: u8, length_size: u8) -> Result<AttributeMessage> {
    let _reserved = cursor.read_u8()?;
    let name_size = cursor.read_u16_le()? as usize;
    let datatype_size = cursor.read_u16_le()? as usize;
    let dataspace_size = cursor.read_u16_le()? as usize;

    // Name — padded to 8-byte boundary
    let name = cursor.read_fixed_string(name_size)?;
    let name_padded = (name_size + 7) & !7;
    if name_padded > name_size {
        cursor.skip(name_padded - name_size)?;
    }

    // Datatype — padded to 8
    let dt_msg = datatype::parse(cursor, datatype_size)?;
    let dt_consumed = datatype_size; // parse() already handles its own size
    let dt_padded = (dt_consumed + 7) & !7;
    if dt_padded > dt_consumed {
        cursor.skip(dt_padded - dt_consumed)?;
    }

    // Dataspace — padded to 8
    let ds_msg = dataspace::parse(cursor, offset_size, length_size, dataspace_size)?;
    let ds_consumed = dataspace_size;
    let ds_padded = (ds_consumed + 7) & !7;
    if ds_padded > ds_consumed {
        cursor.skip(ds_padded - ds_consumed)?;
    }

    // Raw data — remaining bytes are the attribute data
    let data_size = attribute_raw_data_size(&ds_msg, dt_msg.size)?;
    let raw_data = if data_size > 0 {
        cursor.read_bytes(data_size)?.to_vec()
    } else {
        vec![]
    };

    Ok(AttributeMessage {
        name,
        datatype: dt_msg.datatype,
        dataspace: ds_msg,
        raw_data,
    })
}

fn parse_v2(cursor: &mut Cursor<'_>, offset_size: u8, length_size: u8) -> Result<AttributeMessage> {
    let _flags = cursor.read_u8()?;
    let name_size = cursor.read_u16_le()? as usize;
    let datatype_size = cursor.read_u16_le()? as usize;
    let dataspace_size = cursor.read_u16_le()? as usize;

    // Name — NOT padded in v2
    let name = cursor.read_fixed_string(name_size)?;

    // Datatype
    let dt_msg = datatype::parse(cursor, datatype_size)?;

    // Dataspace
    let ds_msg = dataspace::parse(cursor, offset_size, length_size, dataspace_size)?;

    // Raw data
    let data_size = attribute_raw_data_size(&ds_msg, dt_msg.size)?;
    let raw_data = if data_size > 0 {
        cursor.read_bytes(data_size)?.to_vec()
    } else {
        vec![]
    };

    Ok(AttributeMessage {
        name,
        datatype: dt_msg.datatype,
        dataspace: ds_msg,
        raw_data,
    })
}

fn parse_v3(cursor: &mut Cursor<'_>, offset_size: u8, length_size: u8) -> Result<AttributeMessage> {
    let flags = cursor.read_u8()?;
    let name_size = cursor.read_u16_le()? as usize;
    let datatype_size = cursor.read_u16_le()? as usize;
    let dataspace_size = cursor.read_u16_le()? as usize;
    let _name_encoding = cursor.read_u8()?;

    if (flags & 0x03) != 0 {
        return Err(Error::InvalidData(
            "shared datatype/dataspace in attribute v3 is not supported".to_string(),
        ));
    }

    // Name — NOT padded in v3
    let name = cursor.read_fixed_string(name_size)?;

    // Datatype
    let dt_msg = datatype::parse(cursor, datatype_size)?;

    // Dataspace
    let ds_msg = dataspace::parse(cursor, offset_size, length_size, dataspace_size)?;

    // Raw data
    let data_size = attribute_raw_data_size(&ds_msg, dt_msg.size)?;
    let raw_data = if data_size > 0 {
        cursor.read_bytes(data_size)?.to_vec()
    } else {
        vec![]
    };

    Ok(AttributeMessage {
        name,
        datatype: dt_msg.datatype,
        dataspace: ds_msg,
        raw_data,
    })
}

fn attribute_raw_data_size(ds_msg: &dataspace::DataspaceMessage, dtype_size: u32) -> Result<usize> {
    let elements = usize::try_from(ds_msg.num_elements()?).map_err(|_| {
        Error::InvalidData("attribute element count exceeds platform usize capacity".to_string())
    })?;
    elements.checked_mul(dtype_size as usize).ok_or_else(|| {
        Error::InvalidData("attribute raw data size exceeds platform usize capacity".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ByteOrder;
    use crate::messages::dataspace::DataspaceType;

    /// Build a minimal u32-LE datatype description (8 bytes total: 4 class/ver/flags + 4 size).
    fn u32_le_datatype() -> Vec<u8> {
        let mut buf = Vec::new();
        // class=0, version=1, flags=0 (LE unsigned)
        let class_word: u32 = 0x01 << 4;
        buf.extend_from_slice(&class_word.to_le_bytes());
        buf.extend_from_slice(&4u32.to_le_bytes()); // size=4
                                                    // properties: offset=0, precision=32
        buf.extend_from_slice(&0u16.to_le_bytes());
        buf.extend_from_slice(&32u16.to_le_bytes());
        buf
    }

    /// Build a scalar dataspace (v2, rank=0, type=scalar).
    fn scalar_dataspace() -> Vec<u8> {
        vec![0x02, 0x00, 0x00, 0x00]
    }

    #[test]
    fn parse_v1_scalar_u32_attr() {
        let dt = u32_le_datatype();
        let ds = scalar_dataspace();

        let mut data = vec![
            0x01, // version 1
            0x00, // reserved
        ];
        // name size = 5 ("temp\0")
        data.extend_from_slice(&5u16.to_le_bytes());
        // datatype size
        data.extend_from_slice(&(dt.len() as u16).to_le_bytes());
        // dataspace size
        data.extend_from_slice(&(ds.len() as u16).to_le_bytes());

        // Name "temp\0" padded to 8 bytes
        data.extend_from_slice(b"temp\0\0\0\0");

        // Datatype (12 bytes), padded to 16
        data.extend_from_slice(&dt);
        data.extend_from_slice(&[0u8; 4]); // padding to 16

        // Dataspace (4 bytes), padded to 8
        data.extend_from_slice(&ds);
        data.extend_from_slice(&[0u8; 4]); // padding to 8

        // Raw data: 1 scalar element * 4 bytes
        data.extend_from_slice(&42u32.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert_eq!(msg.name, "temp");
        assert_eq!(msg.dataspace.dataspace_type, DataspaceType::Scalar);
        assert_eq!(msg.raw_data, 42u32.to_le_bytes());
        match &msg.datatype {
            Datatype::FixedPoint {
                size: 4,
                signed: false,
                byte_order: ByteOrder::LittleEndian,
            } => {}
            other => panic!("unexpected datatype: {:?}", other),
        }
    }

    #[test]
    fn parse_v3_scalar_attr() {
        let dt = u32_le_datatype();
        let ds = scalar_dataspace();

        let mut data = vec![
            0x03, // version 3
            0x00, // flags
        ];
        // name size = 4 ("abc\0")
        data.extend_from_slice(&4u16.to_le_bytes());
        data.extend_from_slice(&(dt.len() as u16).to_le_bytes());
        data.extend_from_slice(&(ds.len() as u16).to_le_bytes());
        data.push(0x00); // ASCII name encoding

        // Name (not padded in v3)
        data.extend_from_slice(b"abc\0");

        // Datatype
        data.extend_from_slice(&dt);

        // Dataspace
        data.extend_from_slice(&ds);

        // Raw data
        data.extend_from_slice(&99u32.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert_eq!(msg.name, "abc");
        assert_eq!(msg.dataspace.dataspace_type, DataspaceType::Scalar);
        assert_eq!(msg.raw_data, 99u32.to_le_bytes());
    }

    #[test]
    fn parse_v3_utf8_name_attr() {
        let dt = u32_le_datatype();
        let ds = scalar_dataspace();

        let mut data = vec![
            0x03, // version 3
            0x00, // flags
        ];
        data.extend_from_slice(&2u16.to_le_bytes()); // "x\0"
        data.extend_from_slice(&(dt.len() as u16).to_le_bytes());
        data.extend_from_slice(&(ds.len() as u16).to_le_bytes());
        data.push(0x01); // UTF-8 name encoding
        data.extend_from_slice(b"x\0");
        data.extend_from_slice(&dt);
        data.extend_from_slice(&ds);
        data.extend_from_slice(&7u32.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert_eq!(msg.name, "x");
        assert_eq!(msg.dataspace.dataspace_type, DataspaceType::Scalar);
        assert_eq!(msg.raw_data, 7u32.to_le_bytes());
    }
}
