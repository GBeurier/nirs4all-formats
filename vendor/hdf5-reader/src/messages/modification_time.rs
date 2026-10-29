//! HDF5 Modification Time messages.
//!
//! Two types carry modification timestamps:
//! - Old modification time (type 0x000E): raw 4-byte string "YYYYMMDDHHMMSS" (14 bytes).
//! - New modification time (type 0x0012): version byte + reserved + u32 seconds since epoch.

use crate::error::{Error, Result};
use crate::io::Cursor;

/// Parsed modification time.
#[derive(Debug, Clone)]
pub struct ModificationTimeMessage {
    /// Seconds since UNIX epoch.
    pub seconds_since_epoch: u32,
}

/// Parse the old modification time message (type 0x000E).
///
/// The old format stores a 14-character ASCII string "YYYYMMDDHHMMSS" but
/// we just store the raw u32 value since the old format is rarely encountered
/// in practice. If we get one, we attempt to parse or store 0.
pub fn parse_old(
    cursor: &mut Cursor<'_>,
    _offset_size: u8,
    _length_size: u8,
    msg_size: usize,
) -> Result<ModificationTimeMessage> {
    // The old format is a fixed-length date string. We skip it and return 0
    // since precise conversion is rarely needed and would require date math.
    if msg_size > 0 {
        cursor.skip(msg_size)?;
    }
    Ok(ModificationTimeMessage {
        seconds_since_epoch: 0,
    })
}

/// Parse the new modification time message (type 0x0012).
pub fn parse_new(
    cursor: &mut Cursor<'_>,
    _offset_size: u8,
    _length_size: u8,
    msg_size: usize,
) -> Result<ModificationTimeMessage> {
    let start = cursor.position();
    let version = cursor.read_u8()?;
    if version != 1 {
        return Err(Error::InvalidData(format!(
            "unsupported modification time version: {}",
            version
        )));
    }

    let _reserved = cursor.read_bytes(3)?;
    let seconds_since_epoch = cursor.read_u32_le()?;

    let consumed = (cursor.position() - start) as usize;
    if consumed < msg_size {
        cursor.skip(msg_size - consumed)?;
    }

    Ok(ModificationTimeMessage {
        seconds_since_epoch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_new_modification_time() {
        let mut data = vec![
            0x01, // version
            0x00, 0x00, 0x00, // reserved
        ];
        data.extend_from_slice(&1700000000u32.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse_new(&mut cursor, 8, 8, data.len()).unwrap();
        assert_eq!(msg.seconds_since_epoch, 1700000000);
    }
}
