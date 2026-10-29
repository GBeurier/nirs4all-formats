//! HDF5 shared object-header message references.
//!
//! A shared message indicates that the actual message content is stored
//! elsewhere — either in another object header or in the shared message
//! heap (SOHM).

use crate::error::{Error, Result};
use crate::io::Cursor;

/// Where the shared message is stored.
#[derive(Debug, Clone)]
pub enum SharedMessage {
    /// Shared in another object header at the given address.
    SharedInOhdr {
        /// Original object-header message type from the shared message envelope.
        message_type: u16,
        /// Object header that contains the shared message.
        address: u64,
    },
    /// Shared in the SOHM fractal heap, identified by a heap ID.
    SharedInSohm {
        /// Original object-header message type from the shared message envelope.
        message_type: u16,
        /// Fractal heap ID in the file's SOHM heap.
        heap_id: Vec<u8>,
    },
}

/// Parse a shared message wrapper.
///
/// `message_type` is the type ID from the containing object-header message
/// envelope. The shared-message payload itself only stores where the actual
/// message lives, so callers must preserve this type to decode SOHM payloads.
pub fn parse(
    cursor: &mut Cursor<'_>,
    message_type: u16,
    offset_size: u8,
    _length_size: u8,
    msg_size: usize,
) -> Result<SharedMessage> {
    let start = cursor.position();
    let version = cursor.read_u8()?;

    let msg = match version {
        1 => {
            // Version 1: type byte + reserved(6) + address
            let _type_field = cursor.read_u8()?;
            let _reserved = cursor.read_bytes(6)?;
            let address = cursor.read_offset(offset_size)?;
            SharedMessage::SharedInOhdr {
                message_type,
                address,
            }
        }
        2 => {
            let type_field = cursor.read_u8()?;
            cursor.skip(2)?;
            match type_field {
                0 => {
                    let address = cursor.read_offset(offset_size)?;
                    SharedMessage::SharedInOhdr {
                        message_type,
                        address,
                    }
                }
                t => {
                    return Err(Error::InvalidData(format!(
                        "unknown shared message v2 type: {}",
                        t
                    )));
                }
            }
        }
        3 => {
            let type_field = cursor.read_u8()?;
            cursor.skip(2)?;
            match type_field {
                1 => {
                    let heap_id = cursor.read_bytes(8)?.to_vec();
                    SharedMessage::SharedInSohm {
                        message_type,
                        heap_id,
                    }
                }
                2 => {
                    let address = cursor.read_offset(offset_size)?;
                    SharedMessage::SharedInOhdr {
                        message_type,
                        address,
                    }
                }
                t => {
                    return Err(Error::InvalidData(format!(
                        "unsupported shared message v3 type: {}",
                        t
                    )));
                }
            }
        }
        v => {
            return Err(Error::InvalidData(format!(
                "unsupported shared message version: {}",
                v
            )));
        }
    };

    let consumed = (cursor.position() - start) as usize;
    if consumed < msg_size {
        cursor.skip(msg_size - consumed)?;
    }

    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::MSG_DATATYPE;

    #[test]
    fn parses_v2_object_header_reference_with_padding() {
        let mut bytes = Vec::new();
        bytes.push(2);
        bytes.push(0);
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&0x1234u64.to_le_bytes());

        let mut cursor = Cursor::new(&bytes);
        let message = parse(&mut cursor, MSG_DATATYPE, 8, 8, bytes.len()).unwrap();
        match message {
            SharedMessage::SharedInOhdr {
                message_type,
                address,
            } => {
                assert_eq!(message_type, MSG_DATATYPE);
                assert_eq!(address, 0x1234);
            }
            other => panic!("expected OHDR reference, got {:?}", other),
        }
    }

    #[test]
    fn parses_v3_sohm_heap_id_with_padding() {
        let mut bytes = Vec::new();
        bytes.push(3);
        bytes.push(1);
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);

        let mut cursor = Cursor::new(&bytes);
        let message = parse(&mut cursor, MSG_DATATYPE, 8, 8, bytes.len()).unwrap();
        match message {
            SharedMessage::SharedInSohm {
                message_type,
                heap_id,
            } => {
                assert_eq!(message_type, MSG_DATATYPE);
                assert_eq!(heap_id, vec![1, 2, 3, 4, 5, 6, 7, 8]);
            }
            other => panic!("expected SOHM reference, got {:?}", other),
        }
    }
}
