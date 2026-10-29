//! HDF5 Local Heap (HEAP).
//!
//! A local heap stores small, variable-length data — most commonly the link
//! names referenced by symbol table entries in old-style (v1) groups. Each
//! group has its own local heap pointed to by the Symbol Table message or the
//! root symbol table entry's scratch-pad.
//!
//! The heap header stores metadata while the actual string data lives in a
//! contiguous data segment elsewhere in the file.

use crate::error::{Error, Result};
use crate::io::Cursor;
use crate::storage::Storage;

/// Signature bytes for a Local Heap: ASCII `HEAP`.
const HEAP_SIGNATURE: [u8; 4] = *b"HEAP";

/// A parsed HDF5 local heap header.
///
/// The data segment (containing the actual strings) is stored separately in
/// the file at `data_segment_address`.
#[derive(Debug, Clone)]
pub struct LocalHeap {
    /// Size in bytes of the data segment.
    pub data_segment_size: u64,
    /// Offset to the head of the free-list within the data segment
    /// (relative to the start of the data segment).
    pub free_list_offset: u64,
    /// Absolute file address of the data segment.
    pub data_segment_address: u64,
}

impl LocalHeap {
    /// Parse a local heap header at the current cursor position.
    ///
    /// Format:
    /// - Signature: `HEAP` (4 bytes)
    /// - Version: 0 (1 byte)
    /// - Reserved: 3 bytes
    /// - Data segment size (`length_size` bytes)
    /// - Offset to head of free list (`length_size` bytes)
    /// - Data segment address (`offset_size` bytes)
    pub fn parse(cursor: &mut Cursor, offset_size: u8, length_size: u8) -> Result<Self> {
        let sig = cursor.read_bytes(4)?;
        if sig != HEAP_SIGNATURE {
            return Err(Error::InvalidLocalHeapSignature);
        }

        let version = cursor.read_u8()?;
        if version != 0 {
            return Err(Error::UnsupportedLocalHeapVersion(version));
        }

        // Reserved 3 bytes
        cursor.skip(3)?;

        let data_segment_size = cursor.read_length(length_size)?;
        let free_list_offset = cursor.read_length(length_size)?;
        let data_segment_address = cursor.read_offset(offset_size)?;

        Ok(LocalHeap {
            data_segment_size,
            free_list_offset,
            data_segment_address,
        })
    }

    /// Parse a local heap header from random-access storage.
    pub fn parse_at_storage(
        storage: &dyn Storage,
        address: u64,
        offset_size: u8,
        length_size: u8,
    ) -> Result<Self> {
        let header_len = 4
            + 1
            + 3
            + usize::from(length_size)
            + usize::from(length_size)
            + usize::from(offset_size);
        let bytes = storage.read_range(address, header_len)?;
        let mut cursor = Cursor::new(bytes.as_ref());
        Self::parse(&mut cursor, offset_size, length_size)
    }

    /// Read a null-terminated string at the given offset within the heap's
    /// data segment.
    ///
    /// `offset` is relative to the start of the data segment (as stored in
    /// `SymbolTableEntry::link_name_offset`). `file_data` must be the entire
    /// file (or at least the portion containing the data segment).
    pub fn get_string(&self, offset: u64, file_data: &[u8]) -> Result<String> {
        let abs = self
            .data_segment_address
            .checked_add(offset)
            .ok_or(Error::OffsetOutOfBounds(offset))?;
        let start = abs as usize;

        if start >= file_data.len() {
            return Err(Error::OffsetOutOfBounds(abs));
        }

        // Find the null terminator within the data segment bounds.
        let segment_end = (self.data_segment_address as usize)
            .saturating_add(self.data_segment_size as usize)
            .min(file_data.len());

        let search_region = &file_data[start..segment_end];
        let null_pos = search_region.iter().position(|&b| b == 0).ok_or_else(|| {
            Error::InvalidData("local heap string missing null terminator".into())
        })?;

        let s = std::str::from_utf8(&search_region[..null_pos])
            .map_err(|e| Error::InvalidData(format!("invalid UTF-8 in local heap string: {e}")))?;

        Ok(s.to_string())
    }

    /// Read a null-terminated string from random-access storage.
    pub fn get_string_storage(&self, offset: u64, storage: &dyn Storage) -> Result<String> {
        if offset >= self.data_segment_size {
            return Err(Error::OffsetOutOfBounds(offset));
        }

        let available = self
            .data_segment_size
            .checked_sub(offset)
            .ok_or(Error::OffsetOutOfBounds(offset))?;
        let len = usize::try_from(available).map_err(|_| {
            Error::InvalidData("local heap string region exceeds platform usize capacity".into())
        })?;
        let abs = self
            .data_segment_address
            .checked_add(offset)
            .ok_or(Error::OffsetOutOfBounds(offset))?;
        let bytes = storage.read_range(abs, len)?;
        let null_pos = bytes.iter().position(|&b| b == 0).ok_or_else(|| {
            Error::InvalidData("local heap string missing null terminator".into())
        })?;
        let s = std::str::from_utf8(&bytes[..null_pos])
            .map_err(|e| Error::InvalidData(format!("invalid UTF-8 in local heap string: {e}")))?;
        Ok(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a local heap header with the given parameters (8-byte offset/length).
    fn build_heap_header(
        data_segment_size: u64,
        free_list_offset: u64,
        data_segment_address: u64,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"HEAP");
        buf.push(0); // version
        buf.extend_from_slice(&[0, 0, 0]); // reserved
        buf.extend_from_slice(&data_segment_size.to_le_bytes());
        buf.extend_from_slice(&free_list_offset.to_le_bytes());
        buf.extend_from_slice(&data_segment_address.to_le_bytes());
        buf
    }

    #[test]
    fn parse_local_heap() {
        let data = build_heap_header(256, 128, 0x2000);

        let mut cursor = Cursor::new(&data);
        let heap = LocalHeap::parse(&mut cursor, 8, 8).unwrap();

        assert_eq!(heap.data_segment_size, 256);
        assert_eq!(heap.free_list_offset, 128);
        assert_eq!(heap.data_segment_address, 0x2000);
    }

    #[test]
    fn parse_local_heap_4byte() {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"HEAP");
        buf.push(0); // version
        buf.extend_from_slice(&[0, 0, 0]); // reserved
        buf.extend_from_slice(&64u32.to_le_bytes()); // data segment size
        buf.extend_from_slice(&32u32.to_le_bytes()); // free list offset
        buf.extend_from_slice(&0x400u32.to_le_bytes()); // data segment address

        let mut cursor = Cursor::new(&buf);
        let heap = LocalHeap::parse(&mut cursor, 4, 4).unwrap();

        assert_eq!(heap.data_segment_size, 64);
        assert_eq!(heap.free_list_offset, 32);
        assert_eq!(heap.data_segment_address, 0x400);
    }

    #[test]
    fn bad_signature() {
        let mut data = build_heap_header(256, 128, 0x2000);
        data[0] = b'X'; // corrupt signature
        let mut cursor = Cursor::new(&data);
        assert!(matches!(
            LocalHeap::parse(&mut cursor, 8, 8),
            Err(Error::InvalidLocalHeapSignature)
        ));
    }

    #[test]
    fn bad_version() {
        let mut data = build_heap_header(256, 128, 0x2000);
        data[4] = 1; // version 1 (unsupported)
        let mut cursor = Cursor::new(&data);
        assert!(matches!(
            LocalHeap::parse(&mut cursor, 8, 8),
            Err(Error::UnsupportedLocalHeapVersion(1))
        ));
    }

    #[test]
    fn local_heap_reads_null_terminated_strings() {
        // Simulate a file where the data segment starts at offset 100.
        let mut file_data = vec![0u8; 200];
        // Place "hello\0world\0" at the data segment.
        let seg_start = 100usize;
        file_data[seg_start..seg_start + 6].copy_from_slice(b"hello\0");
        file_data[seg_start + 6..seg_start + 12].copy_from_slice(b"world\0");

        let heap = LocalHeap {
            data_segment_size: 100,
            free_list_offset: 50,
            data_segment_address: seg_start as u64,
        };

        assert_eq!(heap.get_string(0, &file_data).unwrap(), "hello");
        assert_eq!(heap.get_string(6, &file_data).unwrap(), "world");
    }

    #[test]
    fn get_string_out_of_bounds() {
        let file_data = vec![0u8; 50];
        let heap = LocalHeap {
            data_segment_size: 100,
            free_list_offset: 0,
            data_segment_address: 100, // beyond file_data
        };
        assert!(heap.get_string(0, &file_data).is_err());
    }

    #[test]
    fn get_string_missing_null() {
        // Data segment with no null terminator.
        let mut file_data = vec![0xFFu8; 200];
        file_data[100..105].copy_from_slice(b"abcde");

        let heap = LocalHeap {
            data_segment_size: 5,
            free_list_offset: 0,
            data_segment_address: 100,
        };
        assert!(heap.get_string(0, &file_data).is_err());
    }
}
