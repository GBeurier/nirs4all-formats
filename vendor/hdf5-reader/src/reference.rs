//! HDF5 object reference resolution.
//!
//! Object references in HDF5 are `offset_size`-byte addresses pointing to
//! object headers. This module provides utilities for reading and resolving
//! these references.

use crate::error::{Error, Result};
use crate::io::Cursor;

/// Read a single object reference (an address) from raw bytes.
///
/// Returns the file address of the referenced object header.
pub fn resolve_object_reference(ref_bytes: &[u8], offset_size: u8) -> Result<u64> {
    if ref_bytes.len() < offset_size as usize {
        return Err(Error::InvalidData(format!(
            "object reference too short: need {} bytes, have {}",
            offset_size,
            ref_bytes.len()
        )));
    }
    let mut cursor = Cursor::new(ref_bytes);
    cursor.read_offset(offset_size)
}

/// Read an array of object references from raw bytes.
///
/// Each reference is `offset_size` bytes. Returns a vector of file addresses.
pub fn read_object_references(raw_data: &[u8], offset_size: u8) -> Result<Vec<u64>> {
    let ref_size = offset_size as usize;
    if ref_size == 0 {
        return Ok(Vec::new());
    }
    let count = raw_data.len() / ref_size;
    let mut refs = Vec::with_capacity(count);
    let mut cursor = Cursor::new(raw_data);
    for _ in 0..count {
        let addr = cursor.read_offset(offset_size)?;
        refs.push(addr);
    }
    Ok(refs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_object_reference_8byte() {
        let addr: u64 = 0x1234_5678_9ABC_DEF0;
        let bytes = addr.to_le_bytes();
        let result = resolve_object_reference(&bytes, 8).unwrap();
        assert_eq!(result, addr);
    }

    #[test]
    fn resolve_object_reference_4byte() {
        let addr: u32 = 0x1234_5678;
        let bytes = addr.to_le_bytes();
        let result = resolve_object_reference(&bytes, 4).unwrap();
        assert_eq!(result, addr as u64);
    }

    #[test]
    fn object_references_read_8_byte_addresses() {
        let mut data = Vec::new();
        data.extend_from_slice(&0x1000u64.to_le_bytes());
        data.extend_from_slice(&0x2000u64.to_le_bytes());
        data.extend_from_slice(&0x3000u64.to_le_bytes());

        let refs = read_object_references(&data, 8).unwrap();
        assert_eq!(refs, vec![0x1000, 0x2000, 0x3000]);
    }

    #[test]
    fn read_object_references_4byte() {
        let mut data = Vec::new();
        data.extend_from_slice(&0xAAAAu32.to_le_bytes());
        data.extend_from_slice(&0xBBBBu32.to_le_bytes());

        let refs = read_object_references(&data, 4).unwrap();
        assert_eq!(refs, vec![0xAAAA, 0xBBBB]);
    }

    #[test]
    fn too_short_reference() {
        let data = [0x01, 0x02];
        let err = resolve_object_reference(&data, 8).unwrap_err();
        assert!(matches!(err, Error::InvalidData(_)));
    }
}
