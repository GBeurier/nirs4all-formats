//! HDF5 Link message (type 0x0006).
//!
//! Used in v2+ object headers to describe group membership. Each link
//! message names one child and specifies its target (hard link to an
//! object header address, soft link path, or external file + path).

use crate::error::{Error, Result};
use crate::io::Cursor;
pub use hdf5_core::{LinkMessage, LinkTarget};

/// Parse a link message (version 1).
pub fn parse(
    cursor: &mut Cursor<'_>,
    offset_size: u8,
    _length_size: u8,
    msg_size: usize,
) -> Result<LinkMessage> {
    let start = cursor.position();
    let version = cursor.read_u8()?;

    if version != 1 {
        return Err(Error::UnsupportedLinkVersion(version));
    }

    let flags = cursor.read_u8()?;

    // Bit 3: link type field present
    let link_type = if (flags & 0x08) != 0 {
        cursor.read_u8()?
    } else {
        0 // default = hard link
    };

    // Bit 2: creation order present
    let creation_order = if (flags & 0x04) != 0 {
        Some(cursor.read_u64_le()?)
    } else {
        None
    };

    // Bit 4: link name encoding (0 = ASCII, 1 = UTF-8)
    // We handle both the same way since our strings are already UTF-8.
    let _name_encoding = if (flags & 0x10) != 0 {
        cursor.read_u8()?
    } else {
        0
    };

    // Name length — size depends on bits 0-1 of flags
    let name_len_size = 1 << (flags & 0x03);
    let name_len = cursor.read_uvar(name_len_size)? as usize;

    let name = cursor.read_fixed_string(name_len)?;

    // Link target
    let target = match link_type {
        0 => {
            // Hard link
            let address = cursor.read_offset(offset_size)?;
            LinkTarget::Hard { address }
        }
        1 => {
            // Soft link
            let path_len = cursor.read_u16_le()? as usize;
            let path = cursor.read_fixed_string(path_len)?;
            LinkTarget::Soft { path }
        }
        64 => {
            // External link
            let info_len = cursor.read_u16_le()? as usize;
            let info_start = cursor.position();
            // Version + flags of the external link info
            let _ext_flags = cursor.read_u8()?;
            let filename = cursor.read_null_terminated_string()?;
            let path = cursor.read_null_terminated_string()?;
            // Skip any remaining bytes in the info block
            let info_consumed = (cursor.position() - info_start) as usize;
            if info_consumed < info_len {
                cursor.skip(info_len - info_consumed)?;
            }
            LinkTarget::External { filename, path }
        }
        t => return Err(Error::UnsupportedLinkType(t)),
    };

    let consumed = (cursor.position() - start) as usize;
    if consumed < msg_size {
        cursor.skip(msg_size - consumed)?;
    }

    Ok(LinkMessage {
        name,
        target,
        creation_order,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hard_link() {
        let mut data = vec![
            0x01, // version 1
            0x00, // flags: name_len_size=1 byte, no extras
        ];
        // name length (1 byte) = 5
        data.push(0x05);
        // name = "hello"
        data.extend_from_slice(b"hello");
        // hard link address (8 bytes)
        data.extend_from_slice(&0x3000u64.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert_eq!(msg.name, "hello");
        assert!(msg.creation_order.is_none());
        match &msg.target {
            LinkTarget::Hard { address } => assert_eq!(*address, 0x3000),
            other => panic!("expected Hard, got {:?}", other),
        }
    }

    #[test]
    fn parse_soft_link() {
        let mut data = vec![
            0x01, // version 1
            0x08, // flags: bit 3 = link type present, name_len_size=1
        ];
        // link type = 1 (soft)
        data.push(0x01);
        // name length = 3
        data.push(0x03);
        // name = "lnk"
        data.extend_from_slice(b"lnk");
        // soft link: path length (u16) + path
        data.extend_from_slice(&7u16.to_le_bytes());
        data.extend_from_slice(b"/a/b/c\0");

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert_eq!(msg.name, "lnk");
        match &msg.target {
            LinkTarget::Soft { path } => assert_eq!(path, "/a/b/c"),
            other => panic!("expected Soft, got {:?}", other),
        }
    }

    #[test]
    fn parse_hard_link_with_creation_order() {
        let mut data = vec![
            0x01, // version 1
            0x04, // flags: bit 2 = creation order present
        ];
        // creation order (u64) = 42
        data.extend_from_slice(&42u64.to_le_bytes());
        // name length = 1
        data.push(0x01);
        // name = "x"
        data.push(b'x');
        // hard link address
        data.extend_from_slice(&0x5000u64.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert_eq!(msg.name, "x");
        assert_eq!(msg.creation_order, Some(42));
    }
}
