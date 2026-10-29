use crate::error::{ByteOrder, Error, Result};

/// A cursor over a byte slice for sequential reading with endian-aware helpers.
///
/// All HDF5 file parsing goes through this type. It wraps a `&[u8]` and tracks
/// the current position. Methods advance the position on success.
#[derive(Clone)]
pub struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    /// Create a new cursor at position 0.
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Current byte position.
    #[inline]
    pub fn position(&self) -> u64 {
        self.pos as u64
    }

    /// Set the position.
    pub fn set_position(&mut self, pos: u64) {
        self.pos = pos as usize;
    }

    /// Total length of the underlying data.
    #[inline]
    pub fn len(&self) -> u64 {
        self.data.len() as u64
    }

    /// Returns `true` if the underlying data is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Remaining bytes from current position.
    #[inline]
    pub fn remaining(&self) -> u64 {
        self.data.len().saturating_sub(self.pos) as u64
    }

    /// Get the underlying data slice.
    #[inline]
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    /// Get a slice starting from an absolute offset.
    pub fn slice_from(&self, offset: u64) -> Result<&'a [u8]> {
        let offset = offset as usize;
        if offset > self.data.len() {
            return Err(Error::OffsetOutOfBounds(offset as u64));
        }
        Ok(&self.data[offset..])
    }

    /// Create a new cursor at a given absolute offset.
    pub fn at_offset(&self, offset: u64) -> Result<Cursor<'a>> {
        if offset as usize > self.data.len() {
            return Err(Error::OffsetOutOfBounds(offset));
        }
        Ok(Cursor {
            data: self.data,
            pos: offset as usize,
        })
    }

    /// Read exactly `n` bytes and advance.
    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or(Error::UnexpectedEof {
            offset: self.pos as u64,
            needed: n as u64,
            available: self.remaining(),
        })?;
        if end > self.data.len() {
            return Err(Error::UnexpectedEof {
                offset: self.pos as u64,
                needed: n as u64,
                available: self.remaining(),
            });
        }
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    /// Peek at the next `n` bytes without advancing.
    pub fn peek_bytes(&self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or(Error::UnexpectedEof {
            offset: self.pos as u64,
            needed: n as u64,
            available: self.remaining(),
        })?;
        if end > self.data.len() {
            return Err(Error::UnexpectedEof {
                offset: self.pos as u64,
                needed: n as u64,
                available: self.remaining(),
            });
        }
        Ok(&self.data[self.pos..end])
    }

    /// Skip `n` bytes.
    pub fn skip(&mut self, n: usize) -> Result<()> {
        let end = self.pos.checked_add(n).ok_or(Error::UnexpectedEof {
            offset: self.pos as u64,
            needed: n as u64,
            available: self.remaining(),
        })?;
        if end > self.data.len() {
            return Err(Error::UnexpectedEof {
                offset: self.pos as u64,
                needed: n as u64,
                available: self.remaining(),
            });
        }
        self.pos = end;
        Ok(())
    }

    /// Align position to `alignment` boundary.
    pub fn align(&mut self, alignment: usize) -> Result<()> {
        if alignment == 0 || alignment == 1 {
            return Ok(());
        }
        let remainder = self.pos % alignment;
        if remainder != 0 {
            self.skip(alignment - remainder)?;
        }
        Ok(())
    }

    // ---- Single-byte reads ----

    pub fn read_u8(&mut self) -> Result<u8> {
        let b = self.read_bytes(1)?;
        Ok(b[0])
    }

    pub fn read_i8(&mut self) -> Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    // ---- Little-endian reads (HDF5 default) ----

    pub fn read_u16_le(&mut self) -> Result<u16> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn read_u32_le(&mut self) -> Result<u32> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_u64_le(&mut self) -> Result<u64> {
        let b = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    pub fn read_i16_le(&mut self) -> Result<i16> {
        let b = self.read_bytes(2)?;
        Ok(i16::from_le_bytes([b[0], b[1]]))
    }

    pub fn read_i32_le(&mut self) -> Result<i32> {
        let b = self.read_bytes(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_i64_le(&mut self) -> Result<i64> {
        let b = self.read_bytes(8)?;
        Ok(i64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    pub fn read_f32_le(&mut self) -> Result<f32> {
        let b = self.read_bytes(4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_f64_le(&mut self) -> Result<f64> {
        let b = self.read_bytes(8)?;
        Ok(f64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    // ---- Big-endian reads ----

    pub fn read_u16_be(&mut self) -> Result<u16> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    pub fn read_u32_be(&mut self) -> Result<u32> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_u64_be(&mut self) -> Result<u64> {
        let b = self.read_bytes(8)?;
        Ok(u64::from_be_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    // ---- Endian-dispatched reads ----

    pub fn read_u16(&mut self, order: ByteOrder) -> Result<u16> {
        match order {
            ByteOrder::LittleEndian => self.read_u16_le(),
            ByteOrder::BigEndian => self.read_u16_be(),
        }
    }

    pub fn read_u32(&mut self, order: ByteOrder) -> Result<u32> {
        match order {
            ByteOrder::LittleEndian => self.read_u32_le(),
            ByteOrder::BigEndian => self.read_u32_be(),
        }
    }

    pub fn read_u64(&mut self, order: ByteOrder) -> Result<u64> {
        match order {
            ByteOrder::LittleEndian => self.read_u64_le(),
            ByteOrder::BigEndian => self.read_u64_be(),
        }
    }

    // ---- Variable-size offset/length reads ----

    /// Read an offset (address) of `size` bytes (little-endian).
    /// HDF5 uses 2/4/8-byte offsets depending on superblock configuration.
    pub fn read_offset(&mut self, size: u8) -> Result<u64> {
        match size {
            2 => self.read_u16_le().map(u64::from),
            4 => self.read_u32_le().map(u64::from),
            8 => self.read_u64_le(),
            _ => Err(Error::UnsupportedOffsetSize(size)),
        }
    }

    /// Read a length of `size` bytes (little-endian).
    pub fn read_length(&mut self, size: u8) -> Result<u64> {
        match size {
            2 => self.read_u16_le().map(u64::from),
            4 => self.read_u32_le().map(u64::from),
            8 => self.read_u64_le(),
            _ => Err(Error::UnsupportedLengthSize(size)),
        }
    }

    /// Read a variable-size unsigned integer of 1..=8 bytes (little-endian).
    pub fn read_uvar(&mut self, size: usize) -> Result<u64> {
        match size {
            1 => self.read_u8().map(u64::from),
            2 => self.read_u16_le().map(u64::from),
            4 => self.read_u32_le().map(u64::from),
            8 => self.read_u64_le(),
            3 | 5..=7 => {
                let bytes = self.read_bytes(size)?;
                let mut value = 0u64;
                for (shift, byte) in bytes.iter().enumerate() {
                    value |= (*byte as u64) << (shift * 8);
                }
                Ok(value)
            }
            _ => Err(Error::InvalidData(format!(
                "unsupported variable integer size: {}",
                size
            ))),
        }
    }

    /// Check if an offset value represents the "undefined" address.
    pub fn is_undefined_offset(val: u64, offset_size: u8) -> bool {
        match offset_size {
            2 => val == 0xFFFF,
            4 => val == 0xFFFF_FFFF,
            8 => val == 0xFFFF_FFFF_FFFF_FFFF,
            _ => false,
        }
    }

    /// Read a null-terminated UTF-8 string.
    pub fn read_null_terminated_string(&mut self) -> Result<String> {
        let start = self.pos;
        while self.pos < self.data.len() {
            if self.data[self.pos] == 0 {
                let s = std::str::from_utf8(&self.data[start..self.pos])
                    .map_err(|e| Error::InvalidData(format!("invalid UTF-8 string: {e}")))?;
                self.pos += 1; // skip null terminator
                return Ok(s.to_string());
            }
            self.pos += 1;
        }
        Err(Error::UnexpectedEof {
            offset: start as u64,
            needed: 1,
            available: 0,
        })
    }

    /// Read a fixed-length string, trimming null padding.
    pub fn read_fixed_string(&mut self, len: usize) -> Result<String> {
        let bytes = self.read_bytes(len)?;
        // Trim trailing nulls
        let end = bytes.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        let s = std::str::from_utf8(&bytes[..end])
            .map_err(|e| Error::InvalidData(format!("invalid UTF-8 string: {e}")))?;
        Ok(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_reads_u8_and_advances() {
        let data = [0x42];
        let mut c = Cursor::new(&data);
        assert_eq!(c.read_u8().unwrap(), 0x42);
        assert_eq!(c.position(), 1);
    }

    #[test]
    fn cursor_reads_little_endian_u16() {
        let data = [0x01, 0x02];
        let mut c = Cursor::new(&data);
        assert_eq!(c.read_u16_le().unwrap(), 0x0201);
    }

    #[test]
    fn cursor_reads_little_endian_u32() {
        let data = [0x01, 0x02, 0x03, 0x04];
        let mut c = Cursor::new(&data);
        assert_eq!(c.read_u32_le().unwrap(), 0x04030201);
    }

    #[test]
    fn cursor_reads_little_endian_u64() {
        let data = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let mut c = Cursor::new(&data);
        assert_eq!(c.read_u64_le().unwrap(), 0x0807060504030201);
    }

    #[test]
    fn cursor_reads_offsets_by_width() {
        // 4-byte offset
        let data = [0x00, 0x01, 0x00, 0x00];
        let mut c = Cursor::new(&data);
        assert_eq!(c.read_offset(4).unwrap(), 256);

        // 8-byte offset
        let data = [0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        let mut c = Cursor::new(&data);
        assert_eq!(c.read_offset(8).unwrap(), 0x100000000);
    }

    #[test]
    fn null_terminated_string() {
        let data = b"hello\0world";
        let mut c = Cursor::new(data);
        assert_eq!(c.read_null_terminated_string().unwrap(), "hello");
        assert_eq!(c.position(), 6);
    }

    #[test]
    fn fixed_string() {
        let data = b"hi\0\0\0";
        let mut c = Cursor::new(data);
        assert_eq!(c.read_fixed_string(5).unwrap(), "hi");
    }

    #[test]
    fn cursor_aligns_to_requested_boundary() {
        let data = [0u8; 16];
        let mut c = Cursor::new(&data);
        c.skip(3).unwrap();
        c.align(8).unwrap();
        assert_eq!(c.position(), 8);

        // Already aligned
        c.align(8).unwrap();
        assert_eq!(c.position(), 8);
    }

    #[test]
    fn eof_error() {
        let data = [0u8; 2];
        let mut c = Cursor::new(&data);
        assert!(c.read_u32_le().is_err());
    }

    #[test]
    fn undefined_offsets_match_offset_width() {
        assert!(Cursor::is_undefined_offset(0xFFFFFFFF, 4));
        assert!(Cursor::is_undefined_offset(0xFFFFFFFFFFFFFFFF, 8));
        assert!(!Cursor::is_undefined_offset(0, 4));
    }
}
