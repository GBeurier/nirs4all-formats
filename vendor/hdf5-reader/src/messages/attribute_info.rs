//! HDF5 Attribute Info message (type 0x0015).
//!
//! Provides information about how attributes are stored on an object:
//! compactly (inline) or densely (in a fractal heap + v2 B-tree).

use crate::error::{Error, Result};
use crate::io::Cursor;

/// Parsed attribute info message.
#[derive(Debug, Clone)]
pub struct AttributeInfoMessage {
    /// Whether attribute creation order is tracked.
    pub creation_order_tracked: bool,
    /// Whether attribute creation order is indexed.
    pub creation_order_indexed: bool,
    /// Maximum creation order index, if tracked.
    pub max_creation_index: Option<u64>,
    /// Address of the fractal heap for attribute data.
    pub fractal_heap_address: u64,
    /// Address of the v2 B-tree for name-indexed lookups.
    pub btree_name_index_address: u64,
    /// Address of the v2 B-tree for creation-order lookups, if indexed.
    pub btree_creation_order_address: Option<u64>,
}

/// Parse an attribute info message.
pub fn parse(
    cursor: &mut Cursor<'_>,
    offset_size: u8,
    _length_size: u8,
    msg_size: usize,
) -> Result<AttributeInfoMessage> {
    let start = cursor.position();
    let version = cursor.read_u8()?;

    if version != 0 {
        return Err(Error::InvalidData(format!(
            "unsupported attribute info version: {}",
            version
        )));
    }

    let flags = cursor.read_u8()?;
    let creation_order_tracked = (flags & 0x01) != 0;
    let creation_order_indexed = (flags & 0x02) != 0;

    let max_creation_index = if creation_order_tracked {
        Some(cursor.read_u16_le()? as u64)
    } else {
        None
    };

    let fractal_heap_address = cursor.read_offset(offset_size)?;
    let btree_name_index_address = cursor.read_offset(offset_size)?;

    let btree_creation_order_address = if creation_order_indexed {
        Some(cursor.read_offset(offset_size)?)
    } else {
        None
    };

    let consumed = (cursor.position() - start) as usize;
    if consumed < msg_size {
        cursor.skip(msg_size - consumed)?;
    }

    Ok(AttributeInfoMessage {
        creation_order_tracked,
        creation_order_indexed,
        max_creation_index,
        fractal_heap_address,
        btree_name_index_address,
        btree_creation_order_address,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_attr_info_simple() {
        let mut data = vec![
            0x00, // version
            0x00, // flags
        ];
        data.extend_from_slice(&0xF000u64.to_le_bytes());
        data.extend_from_slice(&0xF100u64.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert!(!msg.creation_order_tracked);
        assert_eq!(msg.fractal_heap_address, 0xF000);
        assert_eq!(msg.btree_name_index_address, 0xF100);
    }
}
