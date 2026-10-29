//! HDF5 Shared Message Table object-header message (type 0x000F).
//!
//! This message appears in the superblock extension and points at the file's
//! SOHM master table.

use crate::error::{Error, Result};
use crate::io::Cursor;

/// Pointer to the file-level shared object-header message table.
#[derive(Debug, Clone)]
pub struct SharedTableMessage {
    /// Address of the SOHM master table (`SMTB`).
    pub table_address: u64,
    /// Number of shared-message indexes in the master table.
    pub num_indices: u8,
}

/// Parse a Shared Message Table message.
pub fn parse(
    cursor: &mut Cursor<'_>,
    offset_size: u8,
    _length_size: u8,
    msg_size: usize,
) -> Result<SharedTableMessage> {
    let start = cursor.position();
    let version = cursor.read_u8()?;
    if version != 0 {
        return Err(Error::InvalidData(format!(
            "unsupported shared message table message version: {version}"
        )));
    }
    cursor.skip(3)?;

    let table_address = cursor.read_offset(offset_size)?;
    let num_indices = cursor.read_u8()?;
    cursor.skip(3)?;

    let consumed = (cursor.position() - start) as usize;
    if consumed < msg_size {
        cursor.skip(msg_size - consumed)?;
    }

    Ok(SharedTableMessage {
        table_address,
        num_indices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shared_table_message() {
        let mut bytes = Vec::new();
        bytes.push(0);
        bytes.extend_from_slice(&[0, 0, 0]);
        bytes.extend_from_slice(&0x1234u64.to_le_bytes());
        bytes.push(2);
        bytes.extend_from_slice(&[0, 0, 0]);

        let mut cursor = Cursor::new(&bytes);
        let msg = parse(&mut cursor, 8, 8, bytes.len()).unwrap();
        assert_eq!(msg.table_address, 0x1234);
        assert_eq!(msg.num_indices, 2);
    }
}
