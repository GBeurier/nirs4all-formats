//! HDF5 B-tree 'K' Values message (type 0x0013).
//!
//! Found in superblock extension objects, this message provides the 'K'
//! (branching factor) value for indexed storage internal nodes.

use crate::error::Result;
use crate::io::Cursor;

/// Parsed B-tree K values message.
#[derive(Debug, Clone)]
pub struct BTreeKMessage {
    /// Internal node K value for indexed (chunked) storage B-trees.
    pub indexed_storage_internal_k: u16,
}

/// Parse a B-tree K values message.
pub fn parse(
    cursor: &mut Cursor<'_>,
    _offset_size: u8,
    _length_size: u8,
    msg_size: usize,
) -> Result<BTreeKMessage> {
    let start = cursor.position();

    let _version = cursor.read_u8()?;
    let indexed_storage_internal_k = cursor.read_u16_le()?;

    let consumed = (cursor.position() - start) as usize;
    if consumed < msg_size {
        cursor.skip(msg_size - consumed)?;
    }

    Ok(BTreeKMessage {
        indexed_storage_internal_k,
    })
}
