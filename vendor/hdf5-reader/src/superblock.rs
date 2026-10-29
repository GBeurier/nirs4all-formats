use crate::checksum::jenkins_lookup3;
use crate::error::{Error, Result};
use crate::io::Cursor;
use crate::storage::Storage;
use crate::symbol_table::SymbolTableEntry;

/// HDF5 magic bytes: `\x89HDF\r\n\x1a\n`
pub const HDF5_MAGIC: [u8; 8] = [0x89, 0x48, 0x44, 0x46, 0x0d, 0x0a, 0x1a, 0x0a];

/// Parsed HDF5 superblock.
#[derive(Debug, Clone)]
pub struct Superblock {
    /// Superblock version (0, 1, 2, or 3).
    pub version: u8,
    /// Size of offsets (addresses) in bytes: 2, 4, or 8.
    pub offset_size: u8,
    /// Size of lengths in bytes: 2, 4, or 8.
    pub length_size: u8,
    /// Group leaf node K (v0/v1 only).
    pub group_leaf_node_k: u16,
    /// Group internal node K (v0/v1 only).
    pub group_internal_node_k: u16,
    /// Indexed storage internal node K (v1 only).
    pub indexed_storage_k: u16,
    /// File consistency flags.
    pub consistency_flags: u32,
    /// Base address for offsets (usually 0).
    pub base_address: u64,
    /// Address of the file free-space info (undefined = not present).
    pub free_space_address: u64,
    /// End-of-file address.
    pub eof_address: u64,
    /// Driver information block address (v0/v1 only).
    pub driver_info_address: u64,
    /// Root group symbol table entry (v0/v1).
    pub root_symbol_table_entry: Option<SymbolTableEntry>,
    /// Root group object header address (v2/v3).
    pub root_object_header_address: Option<u64>,
    /// Superblock extension address (v2/v3).
    pub extension_address: Option<u64>,
}

impl Superblock {
    /// Parse the superblock from a cursor positioned at byte 0 (or where the magic starts).
    ///
    /// The cursor should be positioned at the start of the file. The method will
    /// search for the magic bytes at position 0, 512, 1024, 2048, etc.
    pub fn parse(cursor: &mut Cursor<'_>) -> Result<Self> {
        // Search for magic at positions 0, 512, 1024, 2048, ...
        let magic_offset = find_magic(cursor)?;
        cursor.set_position(magic_offset + 8);

        let version = cursor.read_u8()?;
        match version {
            0 | 1 => Self::parse_v0_v1(cursor, version),
            2 | 3 => Self::parse_v2_v3(cursor, version, magic_offset),
            v => Err(Error::UnsupportedSuperblockVersion(v)),
        }
    }

    /// Parse the superblock from random-access storage.
    pub fn parse_from_storage(storage: &dyn Storage) -> Result<Self> {
        let magic_offset = find_magic_in_storage(storage)?;
        let remaining = storage.len().saturating_sub(magic_offset);
        let header_len = remaining.min(256) as usize;
        let header = storage.read_range(magic_offset, header_len)?;
        let mut cursor = Cursor::new(header.as_ref());
        cursor.set_position(8);

        let version = cursor.read_u8()?;
        match version {
            0 | 1 => Self::parse_v0_v1(&mut cursor, version),
            2 | 3 => Self::parse_v2_v3(&mut cursor, version, 0),
            v => Err(Error::UnsupportedSuperblockVersion(v)),
        }
    }

    fn parse_v0_v1(cursor: &mut Cursor<'_>, version: u8) -> Result<Self> {
        let _free_space_version = cursor.read_u8()?;
        let _root_group_version = cursor.read_u8()?;
        let _reserved1 = cursor.read_u8()?;
        let _shared_header_version = cursor.read_u8()?;

        let offset_size = cursor.read_u8()?;
        let length_size = cursor.read_u8()?;
        let _reserved2 = cursor.read_u8()?;

        let group_leaf_node_k = cursor.read_u16_le()?;
        let group_internal_node_k = cursor.read_u16_le()?;
        let consistency_flags = cursor.read_u32_le()?;

        let indexed_storage_k = if version == 1 {
            let k = cursor.read_u16_le()?;
            let _reserved = cursor.read_u16_le()?;
            k
        } else {
            0
        };

        let base_address = cursor.read_offset(offset_size)?;
        let free_space_address = cursor.read_offset(offset_size)?;
        let eof_address = cursor.read_offset(offset_size)?;
        let driver_info_address = cursor.read_offset(offset_size)?;

        let root_entry = SymbolTableEntry::parse(cursor, offset_size, length_size)?;

        Ok(Superblock {
            version,
            offset_size,
            length_size,
            group_leaf_node_k,
            group_internal_node_k,
            indexed_storage_k,
            consistency_flags,
            base_address,
            free_space_address,
            eof_address,
            driver_info_address,
            root_symbol_table_entry: Some(root_entry),
            root_object_header_address: None,
            extension_address: None,
        })
    }

    fn parse_v2_v3(cursor: &mut Cursor<'_>, version: u8, magic_offset: u64) -> Result<Self> {
        let offset_size = cursor.read_u8()?;
        let length_size = cursor.read_u8()?;
        let consistency_flags = cursor.read_u8()? as u32;

        let base_address = cursor.read_offset(offset_size)?;
        let extension_address = cursor.read_offset(offset_size)?;
        let eof_address = cursor.read_offset(offset_size)?;
        let root_object_header_address = cursor.read_offset(offset_size)?;

        let stored_checksum = cursor.read_u32_le()?;

        // Verify checksum: covers everything from magic to just before the checksum
        let checksum_start = magic_offset as usize;
        let checksum_end = cursor.position() as usize - 4;
        let computed = jenkins_lookup3(&cursor.data()[checksum_start..checksum_end]);
        if computed != stored_checksum {
            return Err(Error::ChecksumMismatch {
                expected: stored_checksum,
                actual: computed,
            });
        }

        let ext = if !Cursor::is_undefined_offset(extension_address, offset_size) {
            Some(extension_address)
        } else {
            None
        };

        Ok(Superblock {
            version,
            offset_size,
            length_size,
            group_leaf_node_k: 0,
            group_internal_node_k: 0,
            indexed_storage_k: 0,
            consistency_flags,
            base_address,
            free_space_address: u64::MAX,
            eof_address,
            driver_info_address: u64::MAX,
            root_symbol_table_entry: None,
            root_object_header_address: Some(root_object_header_address),
            extension_address: ext,
        })
    }

    /// Get the root group's object header address.
    pub fn root_object_header_address(&self) -> Result<u64> {
        if let Some(addr) = self.root_object_header_address {
            Ok(addr)
        } else if let Some(ref entry) = self.root_symbol_table_entry {
            Ok(entry.object_header_address)
        } else {
            Err(Error::InvalidData(
                "superblock has no root group reference".into(),
            ))
        }
    }

    /// For v0/v1 superblocks, get the B-tree address from the root symbol table entry's
    /// scratch-pad (used for root group navigation).
    pub fn root_btree_address(&self) -> Option<u64> {
        self.root_symbol_table_entry
            .as_ref()
            .and_then(|e| e.btree_address())
    }

    /// For v0/v1 superblocks, get the local heap address from the root symbol table entry's
    /// scratch-pad.
    pub fn root_local_heap_address(&self) -> Option<u64> {
        self.root_symbol_table_entry
            .as_ref()
            .and_then(|e| e.local_heap_address())
    }
}

/// Search for the HDF5 magic bytes. Per spec, the superblock can appear at
/// offsets 0, 512, 1024, 2048, etc. (powers of two times 512, plus 0).
fn find_magic(cursor: &Cursor<'_>) -> Result<u64> {
    // Check offset 0
    if cursor.len() >= 8 {
        let bytes = cursor.peek_bytes(8)?;
        if bytes == HDF5_MAGIC {
            return Ok(0);
        }
    }

    // Check 512, 1024, 2048, ...
    let mut offset: u64 = 512;
    while offset.checked_add(8).is_some_and(|end| end <= cursor.len()) {
        let c = cursor.at_offset(offset)?;
        let bytes = c.peek_bytes(8)?;
        if bytes == HDF5_MAGIC {
            return Ok(offset);
        }
        let Some(next_offset) = offset.checked_mul(2) else {
            break;
        };
        offset = next_offset;
    }

    Err(Error::InvalidMagic)
}

fn find_magic_in_storage(storage: &dyn Storage) -> Result<u64> {
    if storage.len() >= 8 {
        let bytes = storage.read_range(0, 8)?;
        if bytes.as_ref() == HDF5_MAGIC {
            return Ok(0);
        }
    }

    let mut offset: u64 = 512;
    while offset
        .checked_add(8)
        .is_some_and(|end| end <= storage.len())
    {
        let bytes = storage.read_range(offset, 8)?;
        if bytes.as_ref() == HDF5_MAGIC {
            return Ok(offset);
        }
        let Some(next_offset) = offset.checked_mul(2) else {
            break;
        };
        offset = next_offset;
    }

    Err(Error::InvalidMagic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::RangeRequestStorage;

    #[test]
    fn magic_detection() {
        // Valid magic at offset 0
        let mut data = HDF5_MAGIC.to_vec();
        data.extend_from_slice(&[0u8; 100]);
        let cursor = Cursor::new(&data);
        assert_eq!(find_magic(&cursor).unwrap(), 0);
    }

    #[test]
    fn no_magic() {
        let data = [0u8; 100];
        let cursor = Cursor::new(&data);
        assert!(find_magic(&cursor).is_err());
    }

    #[test]
    fn storage_magic_search_handles_huge_length_without_overflow() {
        let storage = RangeRequestStorage::new(u64::MAX, |_offset, len| Ok(vec![0; len]));

        let err = find_magic_in_storage(&storage).unwrap_err();

        assert!(matches!(err, Error::InvalidMagic));
    }
}
