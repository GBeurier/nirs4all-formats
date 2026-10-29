//! HDF5 Dataspace message (type 0x0001).
//!
//! A dataspace describes the shape of a dataset: scalar, null, or simple
//! (one or more dimensions with current and optional maximum sizes).

use crate::error::{Error, Result};
use crate::io::Cursor;
pub use hdf5_core::{DataspaceMessage, DataspaceType, UNLIMITED};

/// Parse a dataspace message.
///
/// `length_size` is needed for version 1 where dimensions are stored using
/// the file-global length size.
pub fn parse(
    cursor: &mut Cursor<'_>,
    _offset_size: u8,
    length_size: u8,
    msg_size: usize,
) -> Result<DataspaceMessage> {
    let start = cursor.position();
    let version = cursor.read_u8()?;

    match version {
        1 => parse_v1(cursor, length_size),
        2 => parse_v2(cursor, length_size),
        v => Err(Error::UnsupportedDataspaceVersion(v)),
    }
    .and_then(|msg| {
        // Advance past any remaining bytes in the message
        let consumed = (cursor.position() - start) as usize;
        if consumed < msg_size {
            cursor.skip(msg_size - consumed)?;
        }
        Ok(msg)
    })
}

/// Version 1 dataspace message.
fn parse_v1(cursor: &mut Cursor<'_>, length_size: u8) -> Result<DataspaceMessage> {
    let rank = cursor.read_u8()?;
    let flags = cursor.read_u8()?;
    let _reserved = cursor.read_u8()?; // reserved
    let _reserved2 = cursor.read_u32_le()?; // reserved

    let has_max_dims = (flags & 0x01) != 0;
    // Bit 1 was "permutation index present" in v1 but is never actually set
    // in practice. We skip it if the flag is set.
    let has_permutation = (flags & 0x02) != 0;

    let dataspace_type = if rank == 0 {
        DataspaceType::Scalar
    } else {
        DataspaceType::Simple
    };

    let mut dims = Vec::with_capacity(rank as usize);
    for _ in 0..rank {
        dims.push(cursor.read_length(length_size)?);
    }

    let max_dims = if has_max_dims {
        let mut md = Vec::with_capacity(rank as usize);
        for _ in 0..rank {
            md.push(cursor.read_length(length_size)?);
        }
        Some(md)
    } else {
        None
    };

    if has_permutation {
        // Skip permutation indices — each is `length_size` bytes.
        for _ in 0..rank {
            cursor.read_length(length_size)?;
        }
    }

    Ok(DataspaceMessage {
        rank,
        dims,
        max_dims,
        dataspace_type,
    })
}

/// Version 2 dataspace message.
fn parse_v2(cursor: &mut Cursor<'_>, length_size: u8) -> Result<DataspaceMessage> {
    let rank = cursor.read_u8()?;
    let flags = cursor.read_u8()?;
    let ds_type_byte = cursor.read_u8()?;

    let has_max_dims = (flags & 0x01) != 0;

    let dataspace_type = match ds_type_byte {
        0 => DataspaceType::Scalar,
        1 => DataspaceType::Simple,
        2 => DataspaceType::Null,
        _ => {
            return Err(Error::InvalidData(format!(
                "unknown dataspace type: {}",
                ds_type_byte
            )))
        }
    };

    let mut dims = Vec::with_capacity(rank as usize);
    for _ in 0..rank {
        dims.push(cursor.read_length(length_size)?);
    }

    let max_dims = if has_max_dims {
        let mut md = Vec::with_capacity(rank as usize);
        for _ in 0..rank {
            md.push(cursor.read_length(length_size)?);
        }
        Some(md)
    } else {
        None
    };

    Ok(DataspaceMessage {
        rank,
        dims,
        max_dims,
        dataspace_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_v1_scalar() {
        // Version 1, rank=0 (scalar), flags=0, reserved bytes
        let data = [
            0x01, // version
            0x00, // rank
            0x00, // flags
            0x00, // reserved
            0x00, 0x00, 0x00, 0x00, // reserved u32
        ];
        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert_eq!(msg.rank, 0);
        assert_eq!(msg.dataspace_type, DataspaceType::Scalar);
        assert!(msg.dims.is_empty());
        assert!(msg.max_dims.is_none());
        assert_eq!(msg.num_elements().unwrap(), 1);
    }

    #[test]
    fn parse_v1_simple_2d() {
        // Version 1, rank=2, flags=0x01 (has max dims), 8-byte lengths
        let mut data = vec![
            0x01, // version
            0x02, // rank
            0x01, // flags (has max dims)
            0x00, // reserved
            0x00, 0x00, 0x00, 0x00, // reserved u32
        ];
        // dim[0] = 10
        data.extend_from_slice(&10u64.to_le_bytes());
        // dim[1] = 20
        data.extend_from_slice(&20u64.to_le_bytes());
        // max_dim[0] = 100
        data.extend_from_slice(&100u64.to_le_bytes());
        // max_dim[1] = UNLIMITED
        data.extend_from_slice(&u64::MAX.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert_eq!(msg.rank, 2);
        assert_eq!(msg.dims, vec![10, 20]);
        assert_eq!(msg.max_dims.as_ref().unwrap(), &vec![100, UNLIMITED]);
        assert_eq!(msg.dataspace_type, DataspaceType::Simple);
        assert_eq!(msg.num_elements().unwrap(), 200);
    }

    #[test]
    fn parse_v2_simple_1d() {
        // Version 2, rank=1, flags=0x00, type=1 (simple), 4-byte lengths
        let mut data = vec![
            0x02, // version
            0x01, // rank
            0x00, // flags
            0x01, // type = simple
        ];
        // dim[0] = 42
        data.extend_from_slice(&42u32.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 4, 4, data.len()).unwrap();
        assert_eq!(msg.rank, 1);
        assert_eq!(msg.dims, vec![42]);
        assert!(msg.max_dims.is_none());
        assert_eq!(msg.dataspace_type, DataspaceType::Simple);
    }

    #[test]
    fn parse_v2_null() {
        let data = [
            0x02, // version
            0x00, // rank
            0x00, // flags
            0x02, // type = null
        ];
        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert_eq!(msg.dataspace_type, DataspaceType::Null);
        assert_eq!(msg.num_elements().unwrap(), 0);
    }

    #[test]
    fn parse_v2_with_max_dims() {
        let mut data = vec![
            0x02, // version
            0x03, // rank = 3
            0x01, // flags = has max dims
            0x01, // type = simple
        ];
        // current dims: 5, 10, 15
        for &d in &[5u64, 10, 15] {
            data.extend_from_slice(&d.to_le_bytes());
        }
        // max dims: 50, 100, UNLIMITED
        for &d in &[50u64, 100, u64::MAX] {
            data.extend_from_slice(&d.to_le_bytes());
        }

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        assert_eq!(msg.rank, 3);
        assert_eq!(msg.dims, vec![5, 10, 15]);
        let md = msg.max_dims.clone().unwrap();
        assert_eq!(md, vec![50, 100, UNLIMITED]);
        assert_eq!(msg.num_elements().unwrap(), 750);
    }

    #[test]
    fn unsupported_version() {
        let data = [0x03, 0x00, 0x00, 0x00];
        let mut cursor = Cursor::new(&data);
        assert!(parse(&mut cursor, 8, 8, data.len()).is_err());
    }

    #[test]
    fn num_elements_rejects_overflow() {
        let msg = DataspaceMessage {
            rank: 2,
            dims: vec![u64::MAX, 2],
            max_dims: None,
            dataspace_type: DataspaceType::Simple,
        };

        let err = msg.num_elements().unwrap_err();
        assert!(err.to_string().contains("element count"));
    }
}
