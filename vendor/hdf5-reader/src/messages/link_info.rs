//! HDF5 Link Info message (type 0x002A / 0x000A depending on spec version).
//!
//! Provides information about how links within a group are stored:
//! compactly (inline in the object header) or densely (in a fractal heap
//! indexed by a v2 B-tree).

use crate::error::{Error, Result};
use crate::io::Cursor;

/// Parsed link info message.
#[derive(Debug, Clone)]
pub struct LinkInfoMessage {
    /// Whether link creation order is tracked.
    pub creation_order_tracked: bool,
    /// Whether a creation-order B-tree index exists.
    pub creation_order_indexed: bool,
    /// Maximum creation order index value, if tracked.
    pub max_creation_index: Option<u64>,
    /// Address of the fractal heap for link names.
    pub fractal_heap_address: u64,
    /// Address of the v2 B-tree for name-indexed lookups.
    pub btree_name_index_address: u64,
    /// Address of the v2 B-tree for creation-order lookups, if indexed.
    pub btree_creation_order_address: Option<u64>,
}

/// Parse a link info message.
pub fn parse(
    cursor: &mut Cursor<'_>,
    offset_size: u8,
    _length_size: u8,
    msg_size: usize,
) -> Result<LinkInfoMessage> {
    let start = cursor.position();
    let version = cursor.read_u8()?;

    if version != 0 {
        return Err(Error::InvalidData(format!(
            "unsupported link info version: {}",
            version
        )));
    }

    let flags = cursor.read_u8()?;
    let creation_order_tracked = (flags & 0x01) != 0;
    let creation_order_indexed = (flags & 0x02) != 0;

    let max_creation_index = if creation_order_tracked {
        Some(cursor.read_u64_le()?)
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

    Ok(LinkInfoMessage {
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
    fn parse_link_info_no_order() {
        let mut data = vec![
            0x00, // version
            0x00, // flags: no creation order
        ];
        // fractal heap address
        data.extend_from_slice(&0xA000u64.to_le_bytes());
        // btree name index address
        data.extend_from_slice(&0xB000u64.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert!(!msg.creation_order_tracked);
        assert!(!msg.creation_order_indexed);
        assert!(msg.max_creation_index.is_none());
        assert_eq!(msg.fractal_heap_address, 0xA000);
        assert_eq!(msg.btree_name_index_address, 0xB000);
        assert!(msg.btree_creation_order_address.is_none());
    }

    #[test]
    fn parse_link_info_with_order() {
        let mut data = vec![
            0x00, // version
            0x03, // flags: creation order tracked + indexed
        ];
        // max creation index
        data.extend_from_slice(&99u64.to_le_bytes());
        // fractal heap address
        data.extend_from_slice(&0xC000u64.to_le_bytes());
        // btree name index
        data.extend_from_slice(&0xD000u64.to_le_bytes());
        // btree creation order
        data.extend_from_slice(&0xE000u64.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert!(msg.creation_order_tracked);
        assert!(msg.creation_order_indexed);
        assert_eq!(msg.max_creation_index, Some(99));
        assert_eq!(msg.fractal_heap_address, 0xC000);
        assert_eq!(msg.btree_name_index_address, 0xD000);
        assert_eq!(msg.btree_creation_order_address, Some(0xE000));
    }
}
