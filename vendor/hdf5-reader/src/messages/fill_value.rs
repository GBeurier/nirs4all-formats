//! HDF5 Fill Value messages.
//!
//! Two message types carry fill value information:
//! - Old fill value (type 0x0004): raw bytes, length = message size.
//! - New fill value (type 0x0005): versioned, with allocation/write time and defined flag.

use crate::error::{Error, Result};
use crate::io::Cursor;
pub use hdf5_core::{FillTime, FillValueMessage};

/// Parse the old fill value message (type 0x0004).
///
/// The body is a four-byte size followed by that many bytes of fill value. Any bytes
/// after the value (alignment padding) are skipped.
pub fn parse_old(
    cursor: &mut Cursor<'_>,
    _offset_size: u8,
    _length_size: u8,
    msg_size: usize,
) -> Result<FillValueMessage> {
    let value = if msg_size >= 4 {
        let size = cursor.read_u32_le()? as usize;
        if size > msg_size - 4 {
            return Err(Error::InvalidData(format!(
                "old fill value message: value size {size} exceeds message size {msg_size}"
            )));
        }
        let value = (size > 0)
            .then(|| cursor.read_bytes(size).map(<[u8]>::to_vec))
            .transpose()?;
        cursor.skip(msg_size - 4 - size)?;
        value
    } else {
        cursor.skip(msg_size)?;
        None
    };

    Ok(FillValueMessage {
        defined: value.is_some(),
        fill_time: FillTime::IfSet,
        value,
    })
}

/// Parse the new fill value message (type 0x0005).
pub fn parse_new(
    cursor: &mut Cursor<'_>,
    _offset_size: u8,
    _length_size: u8,
    msg_size: usize,
) -> Result<FillValueMessage> {
    let start = cursor.position();
    let version = cursor.read_u8()?;

    match version {
        1 | 2 => parse_new_v1_v2(cursor, version),
        3 => parse_new_v3(cursor),
        v => Err(Error::UnsupportedFillValueVersion(v)),
    }
    .and_then(|msg| {
        let consumed = (cursor.position() - start) as usize;
        if consumed < msg_size {
            cursor.skip(msg_size - consumed)?;
        }
        Ok(msg)
    })
}

fn parse_new_v1_v2(cursor: &mut Cursor<'_>, _version: u8) -> Result<FillValueMessage> {
    let _alloc_time = cursor.read_u8()?;
    let fill_time_byte = cursor.read_u8()?;
    let defined_flag = cursor.read_u8()?;

    let fill_time = match fill_time_byte {
        0 => FillTime::IfSet,
        1 => FillTime::Always,
        2 => FillTime::Never,
        _ => FillTime::IfSet,
    };

    let defined = defined_flag != 0;

    let value = if defined {
        let size = cursor.read_u32_le()? as usize;
        if size > 0 {
            Some(cursor.read_bytes(size)?.to_vec())
        } else {
            None
        }
    } else {
        None
    };

    Ok(FillValueMessage {
        defined,
        fill_time,
        value,
    })
}

fn parse_new_v3(cursor: &mut Cursor<'_>) -> Result<FillValueMessage> {
    let flags = cursor.read_u8()?;

    let _alloc_time = flags & 0x03;
    let fill_time_bits = (flags >> 2) & 0x03;
    // Spec: flag bit 4 marks the fill value undefined, bit 5 marks it defined.
    let undefined = (flags & 0x10) != 0;
    let defined = (flags & 0x20) != 0;

    let fill_time = match fill_time_bits {
        0 => FillTime::IfSet,
        1 => FillTime::Always,
        2 => FillTime::Never,
        _ => FillTime::IfSet,
    };

    if undefined {
        return Ok(FillValueMessage {
            defined: false,
            fill_time,
            value: None,
        });
    }

    let value = if defined {
        let size = cursor.read_u32_le()? as usize;
        if size > 0 {
            Some(cursor.read_bytes(size)?.to_vec())
        } else {
            None
        }
    } else {
        None
    };

    Ok(FillValueMessage {
        defined,
        fill_time,
        value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_old_fill() {
        // size = 4, then the value (0x80000000, i32::MIN), then padding to 8 bytes.
        let data = [
            0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0xAA, 0xAA, 0xAA, 0xAA,
        ];
        let mut cursor = Cursor::new(&data);
        let msg = parse_old(&mut cursor, 8, 8, data.len()).unwrap();
        assert!(msg.defined);
        assert_eq!(msg.value.unwrap(), vec![0x00, 0x00, 0x00, 0x80]);
        assert_eq!(cursor.position() as usize, data.len());
    }

    #[test]
    fn parse_old_fill_rejects_oversized_value() {
        let data = [0x08, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04];
        let mut cursor = Cursor::new(&data);
        assert!(parse_old(&mut cursor, 8, 8, data.len()).is_err());
    }

    #[test]
    fn parse_old_fill_empty() {
        let data = [];
        let mut cursor = Cursor::new(&data);
        let msg = parse_old(&mut cursor, 8, 8, 0).unwrap();
        assert!(!msg.defined);
        assert!(msg.value.is_none());
    }

    #[test]
    fn parse_new_v2_defined() {
        let mut data = vec![
            0x02, // version 2
            0x01, // alloc time
            0x01, // fill time = always
            0x01, // defined = yes
        ];
        // fill value size = 8
        data.extend_from_slice(&8u32.to_le_bytes());
        // fill value bytes
        data.extend_from_slice(&[0xFF; 8]);

        let mut cursor = Cursor::new(&data);
        let msg = parse_new(&mut cursor, 8, 8, data.len()).unwrap();
        assert!(msg.defined);
        assert_eq!(msg.fill_time, FillTime::Always);
        assert_eq!(msg.value.unwrap(), vec![0xFF; 8]);
    }

    #[test]
    fn parse_new_v3_undefined() {
        let data = [
            0x03, // version 3
            0x10, // flags: undefined bit set
        ];
        let mut cursor = Cursor::new(&data);
        let msg = parse_new(&mut cursor, 8, 8, data.len()).unwrap();
        assert!(!msg.defined);
        assert!(msg.value.is_none());
    }

    #[test]
    fn parse_new_v3_defined() {
        let mut data = vec![
            0x03, // version 3
            0x2A, // flags: late alloc, write if set, defined bit set
        ];
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend_from_slice(&[0xAB; 4]);
        let mut cursor = Cursor::new(&data);
        let msg = parse_new(&mut cursor, 8, 8, data.len()).unwrap();
        assert!(msg.defined);
        assert_eq!(msg.value.unwrap(), vec![0xAB; 4]);
    }

    #[test]
    fn parse_new_v3_default_fill_without_value() {
        let data = [
            0x03, // version 3
            0x0B, // flags: incremental alloc, write if set, no defined bit
        ];
        let mut cursor = Cursor::new(&data);
        let msg = parse_new(&mut cursor, 8, 8, data.len()).unwrap();
        assert!(!msg.defined);
        assert!(msg.value.is_none());
    }
}
