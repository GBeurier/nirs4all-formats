//! HDF5 Group Info message (type 0x000A).
//!
//! Contains hints for the library about group storage thresholds and
//! estimated entry counts. Used for v2-style groups.

use crate::error::{Error, Result};
use crate::io::Cursor;

/// Parsed group info message.
#[derive(Debug, Clone)]
pub struct GroupInfoMessage {
    /// Maximum number of links to store compactly (in header messages).
    pub max_compact_links: Option<u16>,
    /// Minimum number of links before switching to dense storage.
    pub min_dense_links: Option<u16>,
    /// Estimated number of entries in this group.
    pub est_num_entries: Option<u16>,
    /// Estimated average name length for entries.
    pub est_name_len: Option<u16>,
}

/// Parse a group info message.
pub fn parse(
    cursor: &mut Cursor<'_>,
    _offset_size: u8,
    _length_size: u8,
    msg_size: usize,
) -> Result<GroupInfoMessage> {
    let start = cursor.position();
    let version = cursor.read_u8()?;

    if version != 0 {
        return Err(Error::InvalidData(format!(
            "unsupported group info version: {}",
            version
        )));
    }

    let flags = cursor.read_u8()?;

    // Bit 0: link phase change values present
    let (max_compact_links, min_dense_links) = if (flags & 0x01) != 0 {
        let max_compact = cursor.read_u16_le()?;
        let min_dense = cursor.read_u16_le()?;
        (Some(max_compact), Some(min_dense))
    } else {
        (None, None)
    };

    // Bit 1: estimated entry information present
    let (est_num_entries, est_name_len) = if (flags & 0x02) != 0 {
        let num = cursor.read_u16_le()?;
        let len = cursor.read_u16_le()?;
        (Some(num), Some(len))
    } else {
        (None, None)
    };

    let consumed = (cursor.position() - start) as usize;
    if consumed < msg_size {
        cursor.skip(msg_size - consumed)?;
    }

    Ok(GroupInfoMessage {
        max_compact_links,
        min_dense_links,
        est_num_entries,
        est_name_len,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_group_info_all_present() {
        let mut data = vec![
            0x00, // version
            0x03, // flags: both bits set
        ];
        data.extend_from_slice(&8u16.to_le_bytes()); // max compact
        data.extend_from_slice(&6u16.to_le_bytes()); // min dense
        data.extend_from_slice(&4u16.to_le_bytes()); // est entries
        data.extend_from_slice(&18u16.to_le_bytes()); // est name len

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert_eq!(msg.max_compact_links, Some(8));
        assert_eq!(msg.min_dense_links, Some(6));
        assert_eq!(msg.est_num_entries, Some(4));
        assert_eq!(msg.est_name_len, Some(18));
    }

    #[test]
    fn parse_group_info_empty() {
        let data = vec![0x00, 0x00];
        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert!(msg.max_compact_links.is_none());
        assert!(msg.est_num_entries.is_none());
    }
}
