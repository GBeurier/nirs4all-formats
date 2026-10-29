//! HDF5 Symbol Table Entry and Symbol Table Node (SNOD).
//!
//! Symbol table entries appear in the superblock (root group) and inside
//! symbol table nodes. Each entry maps a link-name offset (within a local
//! heap) to an object header address, with optional scratch-pad data for
//! group B-tree/heap addresses or symbolic link values.
//!
//! Symbol table nodes (SNOD) are the leaf containers for these entries and
//! are stored at addresses referenced by v1 B-tree group nodes.

use crate::error::{Error, Result};
use crate::io::Cursor;

// ---------------------------------------------------------------------------
// Symbol Table Entry
// ---------------------------------------------------------------------------

/// A symbol table entry (used in v0/v1 superblock and in SNOD nodes).
///
/// When `cache_type` is 1 (group), the scratch-pad space contains the address
/// of the B-tree and local heap for the group. When `cache_type` is 2, the
/// scratch-pad contains a symbolic link offset. These are extracted during
/// parsing and cached for convenience.
#[derive(Debug, Clone)]
pub struct SymbolTableEntry {
    /// Offset of the link name within the local heap data segment.
    pub link_name_offset: u64,
    /// Address of the object header for the target object.
    pub object_header_address: u64,
    /// Cache type: 0 = nothing cached, 1 = group info, 2 = symbolic link.
    pub cache_type: u32,
    /// Raw scratch-pad bytes (always 16 bytes in the file).
    pub scratch: [u8; 16],
    /// B-tree address extracted from scratch when `cache_type == 1`.
    cached_btree_address: Option<u64>,
    /// Local heap address extracted from scratch when `cache_type == 1`.
    cached_heap_address: Option<u64>,
}

impl SymbolTableEntry {
    /// Parse a single symbol table entry from the cursor.
    ///
    /// The format is:
    /// - Link name offset (`offset_size` bytes)
    /// - Object header address (`offset_size` bytes)
    /// - Cache type (u32 LE)
    /// - Reserved (u32 LE)
    /// - 16 bytes scratch-pad
    pub fn parse(cursor: &mut Cursor, offset_size: u8, _length_size: u8) -> Result<Self> {
        let link_name_offset = cursor.read_offset(offset_size)?;
        let object_header_address = cursor.read_offset(offset_size)?;
        let cache_type = cursor.read_u32_le()?;
        let _reserved = cursor.read_u32_le()?;

        let scratch_bytes = cursor.read_bytes(16)?;
        let mut scratch = [0u8; 16];
        scratch.copy_from_slice(scratch_bytes);

        // Extract cached group addresses from scratch when cache_type == 1.
        let (cached_btree_address, cached_heap_address) = if cache_type == 1 {
            let mut sc = Cursor::new(&scratch);
            let btree = sc.read_offset(offset_size)?;
            let heap = sc.read_offset(offset_size)?;
            (Some(btree), Some(heap))
        } else {
            (None, None)
        };

        Ok(SymbolTableEntry {
            link_name_offset,
            object_header_address,
            cache_type,
            scratch,
            cached_btree_address,
            cached_heap_address,
        })
    }

    /// Address of the B-tree for this group (only valid when `cache_type == 1`).
    pub fn btree_address(&self) -> Option<u64> {
        self.cached_btree_address
    }

    /// Address of the local heap for this group (only valid when `cache_type == 1`).
    pub fn local_heap_address(&self) -> Option<u64> {
        self.cached_heap_address
    }
}

// ---------------------------------------------------------------------------
// Symbol Table Node (SNOD)
// ---------------------------------------------------------------------------

/// Signature bytes for a Symbol Table Node: ASCII `SNOD`.
const SNOD_SIGNATURE: [u8; 4] = *b"SNOD";

/// A symbol table node (SNOD) containing one or more symbol table entries.
///
/// These nodes are the leaves of v1 group B-trees and contain the actual
/// link-name-to-object mappings for groups stored in the old format.
#[derive(Debug, Clone)]
pub struct SymbolTableNode {
    /// The entries contained in this node.
    pub entries: Vec<SymbolTableEntry>,
}

impl SymbolTableNode {
    /// Parse a symbol table node at the current cursor position.
    ///
    /// The format is:
    /// - Signature: `SNOD` (4 bytes)
    /// - Version: 1 (1 byte)
    /// - Reserved: 1 byte
    /// - Number of entries (u16 LE)
    /// - That many `SymbolTableEntry` values
    pub fn parse(cursor: &mut Cursor, offset_size: u8, length_size: u8) -> Result<Self> {
        let sig = cursor.read_bytes(4)?;
        if sig != SNOD_SIGNATURE {
            return Err(Error::InvalidSymbolTableNodeSignature);
        }

        let version = cursor.read_u8()?;
        if version != 1 {
            return Err(Error::UnsupportedSymbolTableNodeVersion(version));
        }

        let _reserved = cursor.read_u8()?;
        let num_entries = cursor.read_u16_le()?;

        let mut entries = Vec::with_capacity(num_entries as usize);
        for _ in 0..num_entries {
            entries.push(SymbolTableEntry::parse(cursor, offset_size, length_size)?);
        }

        Ok(SymbolTableNode { entries })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build raw bytes for a symbol table entry with the given fields.
    fn build_entry_bytes(
        link_name_offset: u64,
        obj_header_addr: u64,
        cache_type: u32,
        scratch: &[u8; 16],
        offset_size: u8,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        // link name offset
        match offset_size {
            4 => buf.extend_from_slice(&(link_name_offset as u32).to_le_bytes()),
            8 => buf.extend_from_slice(&link_name_offset.to_le_bytes()),
            _ => panic!("test only supports 4/8 byte offsets"),
        }
        // object header address
        match offset_size {
            4 => buf.extend_from_slice(&(obj_header_addr as u32).to_le_bytes()),
            8 => buf.extend_from_slice(&obj_header_addr.to_le_bytes()),
            _ => panic!("test only supports 4/8 byte offsets"),
        }
        // cache type
        buf.extend_from_slice(&cache_type.to_le_bytes());
        // reserved
        buf.extend_from_slice(&0u32.to_le_bytes());
        // scratch pad
        buf.extend_from_slice(scratch);
        buf
    }

    #[test]
    fn parse_entry_no_cache() {
        let scratch = [0u8; 16];
        let data = build_entry_bytes(42, 0x1000, 0, &scratch, 8);

        let mut cursor = Cursor::new(&data);
        let entry = SymbolTableEntry::parse(&mut cursor, 8, 8).unwrap();

        assert_eq!(entry.link_name_offset, 42);
        assert_eq!(entry.object_header_address, 0x1000);
        assert_eq!(entry.cache_type, 0);
        assert!(entry.btree_address().is_none());
        assert!(entry.local_heap_address().is_none());
    }

    #[test]
    fn parse_entry_group_cache_8byte() {
        // Build scratch with btree=0x2000 and heap=0x3000 (8-byte offsets)
        let mut scratch = [0u8; 16];
        scratch[..8].copy_from_slice(&0x2000u64.to_le_bytes());
        scratch[8..16].copy_from_slice(&0x3000u64.to_le_bytes());

        let data = build_entry_bytes(0, 0x1000, 1, &scratch, 8);

        let mut cursor = Cursor::new(&data);
        let entry = SymbolTableEntry::parse(&mut cursor, 8, 8).unwrap();

        assert_eq!(entry.cache_type, 1);
        assert_eq!(entry.btree_address(), Some(0x2000));
        assert_eq!(entry.local_heap_address(), Some(0x3000));
    }

    #[test]
    fn parse_entry_group_cache_4byte() {
        // Build scratch with btree=0x400 and heap=0x800 (4-byte offsets)
        let mut scratch = [0u8; 16];
        scratch[..4].copy_from_slice(&0x400u32.to_le_bytes());
        scratch[4..8].copy_from_slice(&0x800u32.to_le_bytes());

        let data = build_entry_bytes(0, 0x100, 1, &scratch, 4);

        let mut cursor = Cursor::new(&data);
        let entry = SymbolTableEntry::parse(&mut cursor, 4, 4).unwrap();

        assert_eq!(entry.cache_type, 1);
        assert_eq!(entry.btree_address(), Some(0x400));
        assert_eq!(entry.local_heap_address(), Some(0x800));
    }

    #[test]
    fn parse_snod_basic() {
        let mut data = Vec::new();
        // Signature
        data.extend_from_slice(b"SNOD");
        // Version
        data.push(1);
        // Reserved
        data.push(0);
        // Number of entries = 2
        data.extend_from_slice(&2u16.to_le_bytes());

        // Entry 1: cache_type=0
        let scratch1 = [0u8; 16];
        data.extend(build_entry_bytes(0, 0x1000, 0, &scratch1, 8));

        // Entry 2: cache_type=1, btree=0xA000, heap=0xB000
        let mut scratch2 = [0u8; 16];
        scratch2[..8].copy_from_slice(&0xA000u64.to_le_bytes());
        scratch2[8..16].copy_from_slice(&0xB000u64.to_le_bytes());
        data.extend(build_entry_bytes(16, 0x2000, 1, &scratch2, 8));

        let mut cursor = Cursor::new(&data);
        let node = SymbolTableNode::parse(&mut cursor, 8, 8).unwrap();

        assert_eq!(node.entries.len(), 2);
        assert_eq!(node.entries[0].link_name_offset, 0);
        assert_eq!(node.entries[0].object_header_address, 0x1000);
        assert_eq!(node.entries[0].cache_type, 0);

        assert_eq!(node.entries[1].link_name_offset, 16);
        assert_eq!(node.entries[1].object_header_address, 0x2000);
        assert_eq!(node.entries[1].btree_address(), Some(0xA000));
        assert_eq!(node.entries[1].local_heap_address(), Some(0xB000));
    }

    #[test]
    fn snod_bad_signature() {
        let data = b"XNOD\x01\x00\x00\x00";
        let mut cursor = Cursor::new(data);
        assert!(matches!(
            SymbolTableNode::parse(&mut cursor, 8, 8),
            Err(Error::InvalidSymbolTableNodeSignature)
        ));
    }

    #[test]
    fn snod_bad_version() {
        let data = b"SNOD\x02\x00\x00\x00";
        let mut cursor = Cursor::new(data);
        assert!(matches!(
            SymbolTableNode::parse(&mut cursor, 8, 8),
            Err(Error::UnsupportedSymbolTableNodeVersion(2))
        ));
    }
}
