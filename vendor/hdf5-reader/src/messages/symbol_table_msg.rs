//! HDF5 Symbol Table message (type 0x0011).
//!
//! This is only found in v0/v1 object headers and points to the B-tree and
//! local heap used for group membership in the older format.

use crate::error::Result;
use crate::io::Cursor;

/// Parsed symbol table message.
#[derive(Debug, Clone)]
pub struct SymbolTableMessage {
    /// Address of the v1 B-tree for this group.
    pub btree_address: u64,
    /// Address of the local heap for name storage.
    pub heap_address: u64,
}

/// Parse a symbol table message.
pub fn parse(
    cursor: &mut Cursor<'_>,
    offset_size: u8,
    _length_size: u8,
    _msg_size: usize,
) -> Result<SymbolTableMessage> {
    let btree_address = cursor.read_offset(offset_size)?;
    let heap_address = cursor.read_offset(offset_size)?;

    Ok(SymbolTableMessage {
        btree_address,
        heap_address,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_symbol_table_msg() {
        let mut data = Vec::new();
        // btree address = 0x1000
        data.extend_from_slice(&0x1000u64.to_le_bytes());
        // heap address = 0x2000
        data.extend_from_slice(&0x2000u64.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert_eq!(msg.btree_address, 0x1000);
        assert_eq!(msg.heap_address, 0x2000);
    }
}
