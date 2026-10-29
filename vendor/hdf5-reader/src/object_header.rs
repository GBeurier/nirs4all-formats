//! HDF5 Object Header parser (v1 and v2).
//!
//! Object headers contain a collection of header messages that describe an
//! HDF5 object (group, dataset, committed datatype, etc.).  Two on-disk
//! formats exist:
//!
//! * **Version 1** (HDF5 < 1.8) — 16-byte fixed prefix, messages each have an
//!   8-byte envelope (type u16 + size u16 + flags u8 + reserved 3).
//! * **Version 2** (HDF5 >= 1.8) — begins with the `OHDR` signature, variable-
//!   length prefix, messages have a 4-or-6-byte envelope, and every chunk is
//!   checksummed with Jenkins lookup3.
//!
//! Continuation messages (type `0x0010`) cause the parser to follow an offset
//! to an additional chunk of messages (an `OCHK` block in v2, or a raw message
//! run in v1).

use crate::checksum::jenkins_lookup3;
use crate::error::{Error, Result};
use crate::io::Cursor;
use crate::messages::shared::SharedMessage;
use crate::messages::{parse_message, HdfMessage};
use crate::storage::Storage;

/// Magic signature for v2 object headers.
const OHDR_SIGNATURE: [u8; 4] = *b"OHDR";

/// Magic signature for v2 continuation chunks.
const OCHK_SIGNATURE: [u8; 4] = *b"OCHK";

/// Upper bound on continuation chunks per object header; a malformed file
/// cannot make the parser loop unboundedly.
const MAX_HEADER_CONTINUATIONS: usize = 1024;

/// Guard a continuation-chunk visit: reject revisits (cycles) and enforce the
/// per-header chunk budget.
fn enter_continuation(visited: &mut std::collections::HashSet<u64>, offset: u64) -> Result<()> {
    if !visited.insert(offset) {
        return Err(Error::InvalidData(format!(
            "object header continuation cycle at offset {offset:#x}"
        )));
    }
    if visited.len() > MAX_HEADER_CONTINUATIONS {
        return Err(Error::InvalidData(format!(
            "object header exceeds {MAX_HEADER_CONTINUATIONS} continuation chunks"
        )));
    }
    Ok(())
}

/// Header continuation message type id.
const MSG_TYPE_CONTINUATION: u16 = 0x0010;

/// Nil (padding) message type id.
const MSG_TYPE_NIL: u16 = 0x0000;

fn checked_usize(value: u64, context: &str) -> Result<usize> {
    usize::try_from(value).map_err(|_| {
        Error::InvalidData(format!(
            "{context} value {value} exceeds platform usize capacity"
        ))
    })
}

fn checked_add_usize(lhs: usize, rhs: usize, context: &str) -> Result<usize> {
    lhs.checked_add(rhs)
        .ok_or_else(|| Error::InvalidData(format!("{context} exceeds platform usize capacity")))
}

/// Parsed object header with all its messages.
#[derive(Debug, Clone)]
pub struct ObjectHeader {
    /// Object header format version (1 or 2).
    pub version: u8,
    /// All parsed header messages, collected from every chunk.
    pub messages: Vec<HdfMessage>,
    /// Object reference count.
    pub reference_count: u32,
    /// Modification time in seconds since the UNIX epoch (v2 only, when the
    /// "times stored" flag is set).
    pub modification_time: Option<u32>,
}

impl ObjectHeader {
    /// Parse an object header at the given absolute file address.
    ///
    /// `data` is the entire file mapped into memory, `address` is the byte
    /// offset where the object header starts, and `offset_size` / `length_size`
    /// come from the superblock.
    pub fn parse_at(data: &[u8], address: u64, offset_size: u8, length_size: u8) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        cursor.set_position(address);

        // Peek at the first four bytes to decide v1 vs v2.
        let sig = cursor.peek_bytes(4)?;
        if sig == OHDR_SIGNATURE {
            Self::parse_v2(&cursor, address, offset_size, length_size)
        } else {
            Self::parse_v1(&cursor, address, offset_size, length_size)
        }
    }

    /// Parse an object header from random-access storage.
    pub fn parse_at_storage(
        storage: &dyn Storage,
        address: u64,
        offset_size: u8,
        length_size: u8,
    ) -> Result<Self> {
        let prefix = storage.read_range(address, 4)?;
        if prefix.len() < 4 {
            return Err(Error::UnexpectedEof {
                offset: address,
                needed: 4,
                available: prefix.len() as u64,
            });
        }

        if prefix.as_ref()[..4] == OHDR_SIGNATURE {
            Self::parse_v2_storage(storage, address, offset_size, length_size)
        } else {
            Self::parse_v1_storage(storage, address, offset_size, length_size)
        }
    }

    /// Resolve shared messages by following references to other object headers.
    ///
    /// For `SharedInOhdr`, the referenced object header is parsed and the
    /// matching message type is extracted. `SharedInSohm` requires the
    /// storage-backed resolver because the SOHM table lives in file metadata.
    pub fn resolve_shared_messages(
        &mut self,
        data: &[u8],
        offset_size: u8,
        length_size: u8,
    ) -> Result<()> {
        let old_messages = std::mem::take(&mut self.messages);
        let mut resolved = Vec::with_capacity(old_messages.len());
        for msg in old_messages {
            match msg {
                HdfMessage::Shared(SharedMessage::SharedInOhdr {
                    message_type,
                    address,
                }) => {
                    match Self::parse_at(data, address, offset_size, length_size) {
                        Ok(target_header) => {
                            if let Some(target_msg) =
                                select_shared_message(target_header, message_type)
                            {
                                resolved.push(target_msg);
                            }
                        }
                        Err(_) => {
                            // If we can't parse the target, keep the shared ref
                            resolved.push(HdfMessage::Shared(SharedMessage::SharedInOhdr {
                                message_type,
                                address,
                            }));
                        }
                    }
                }
                HdfMessage::Shared(SharedMessage::SharedInSohm { .. }) => {
                    self.messages = resolved;
                    return Err(Error::Other(
                        "SOHM table lookup not yet supported — file uses shared object header messages".to_string(),
                    ));
                }
                other => resolved.push(other),
            }
        }
        self.messages = resolved;
        Ok(())
    }

    /// Resolve shared messages using random-access storage and a SOHM resolver.
    pub(crate) fn resolve_shared_messages_storage_with_sohm<F>(
        &mut self,
        storage: &dyn Storage,
        offset_size: u8,
        length_size: u8,
        mut resolve_sohm: F,
    ) -> Result<()>
    where
        F: FnMut(&[u8], u16) -> Result<Option<HdfMessage>>,
    {
        let old_messages = std::mem::take(&mut self.messages);
        let mut resolved = Vec::with_capacity(old_messages.len());
        for msg in old_messages {
            match msg {
                HdfMessage::Shared(SharedMessage::SharedInOhdr {
                    message_type,
                    address,
                }) => match Self::parse_at_storage(storage, address, offset_size, length_size) {
                    Ok(target_header) => {
                        if let Some(target_msg) = select_shared_message(target_header, message_type)
                        {
                            resolved.push(target_msg);
                        }
                    }
                    Err(_) => {
                        resolved.push(HdfMessage::Shared(SharedMessage::SharedInOhdr {
                            message_type,
                            address,
                        }));
                    }
                },
                HdfMessage::Shared(SharedMessage::SharedInSohm {
                    message_type,
                    heap_id,
                }) => {
                    if let Some(message) = resolve_sohm(&heap_id, message_type)? {
                        resolved.push(message);
                    } else {
                        self.messages = resolved;
                        return Err(Error::Other(format!(
                            "SOHM entry for message type {message_type:#x} not found"
                        )));
                    }
                }
                other => resolved.push(other),
            }
        }
        self.messages = resolved;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Version 1
    // ------------------------------------------------------------------

    /// Parse a version-1 object header.
    ///
    /// Layout (16 bytes total):
    /// ```text
    ///   version          u8    (must be 1)
    ///   reserved         u8
    ///   num_messages     u16
    ///   ref_count        u32
    ///   header_data_size u32   (byte count of the message run)
    ///   reserved         u32   (alignment padding)
    /// ```
    fn parse_v1(base: &Cursor<'_>, address: u64, offset_size: u8, length_size: u8) -> Result<Self> {
        let mut cursor = base.at_offset(address)?;

        let version = cursor.read_u8()?;
        if version != 1 {
            return Err(Error::UnsupportedObjectHeaderVersion(version));
        }

        let _reserved = cursor.read_u8()?;
        let num_messages = cursor.read_u16_le()?;
        let reference_count = cursor.read_u32_le()?;
        let header_data_size = cursor.read_u32_le()? as u64;
        let _reserved2 = cursor.read_u32_le()?; // alignment padding

        // Messages start right after the 16-byte prefix.
        let messages_start = cursor.position();
        let messages_end = messages_start + header_data_size;

        let mut messages: Vec<HdfMessage> = Vec::with_capacity(num_messages as usize);
        let mut continuations: Vec<(u64, u64)> = Vec::new();

        Self::read_v1_messages(
            base,
            messages_start,
            messages_end,
            offset_size,
            length_size,
            &mut messages,
            &mut continuations,
        )?;

        // Follow continuation messages.
        let mut visited = std::collections::HashSet::new();
        while let Some((cont_offset, cont_length)) = continuations.pop() {
            enter_continuation(&mut visited, cont_offset)?;
            let cont_end = cont_offset.checked_add(cont_length).ok_or_else(|| {
                Error::InvalidData("v1 object header continuation end overflows u64".to_string())
            })?;
            Self::read_v1_messages(
                base,
                cont_offset,
                cont_end,
                offset_size,
                length_size,
                &mut messages,
                &mut continuations,
            )?;
        }

        Ok(ObjectHeader {
            version: 1,
            messages,
            reference_count,
            modification_time: None,
        })
    }

    fn parse_v1_storage(
        storage: &dyn Storage,
        address: u64,
        offset_size: u8,
        length_size: u8,
    ) -> Result<Self> {
        let header = storage.read_range(address, 16)?;
        let mut cursor = Cursor::new(header.as_ref());

        let version = cursor.read_u8()?;
        if version != 1 {
            return Err(Error::UnsupportedObjectHeaderVersion(version));
        }

        let _reserved = cursor.read_u8()?;
        let num_messages = cursor.read_u16_le()?;
        let reference_count = cursor.read_u32_le()?;
        let header_data_size = cursor.read_u32_le()? as u64;
        let _reserved2 = cursor.read_u32_le()?;

        let header_data_size = checked_usize(header_data_size, "v1 object header data size")?;
        let first_chunk_len =
            checked_add_usize(16, header_data_size, "v1 object header chunk length")?;
        let first_chunk = storage.read_range(address, first_chunk_len)?;
        let mut messages = Vec::with_capacity(num_messages as usize);
        let mut continuations = Vec::new();
        Self::read_v1_messages_from_slice(
            &first_chunk.as_ref()[16..],
            offset_size,
            length_size,
            &mut messages,
            &mut continuations,
        )?;

        let mut visited = std::collections::HashSet::new();
        while let Some((cont_offset, cont_length)) = continuations.pop() {
            enter_continuation(&mut visited, cont_offset)?;
            let cont_length = checked_usize(cont_length, "v1 object header continuation length")?;
            let chunk = storage.read_range(cont_offset, cont_length)?;
            Self::read_v1_messages_from_slice(
                chunk.as_ref(),
                offset_size,
                length_size,
                &mut messages,
                &mut continuations,
            )?;
        }

        Ok(ObjectHeader {
            version: 1,
            messages,
            reference_count,
            modification_time: None,
        })
    }

    /// Read v1 header messages from `start..end`, appending to `messages`.
    /// Any continuation messages encountered are pushed onto `continuations`
    /// for the caller to follow.
    fn read_v1_messages(
        base: &Cursor<'_>,
        start: u64,
        end: u64,
        offset_size: u8,
        length_size: u8,
        messages: &mut Vec<HdfMessage>,
        continuations: &mut Vec<(u64, u64)>,
    ) -> Result<()> {
        let mut cursor = base.at_offset(start)?;

        while cursor.position() + 8 <= end {
            let msg_type = cursor.read_u16_le()?;
            let msg_data_size = cursor.read_u16_le()? as usize;
            let msg_flags = cursor.read_u8()?;
            let _reserved = cursor.read_bytes(3)?; // 3 reserved bytes

            // Bounds-check the message data within this chunk.
            if cursor.position() + msg_data_size as u64 > end {
                return Err(Error::InvalidData(format!(
                    "v1 message data ({} bytes) extends past header chunk end",
                    msg_data_size
                )));
            }

            if msg_type == MSG_TYPE_NIL {
                // Nil / padding — skip the data bytes.
                cursor.skip(msg_data_size)?;
                messages.push(HdfMessage::Nil);
                continue;
            }

            let msg_data = cursor.read_bytes(msg_data_size)?;
            let is_shared = (msg_flags & 0x02) != 0;

            if is_shared {
                // Shared message — the stored bytes are a shared-message
                // reference, not the message payload itself.
                let shared_msg = crate::messages::shared::parse(
                    &mut Cursor::new(msg_data),
                    msg_type,
                    offset_size,
                    length_size,
                    msg_data_size,
                )?;
                messages.push(HdfMessage::Shared(shared_msg));
            } else if msg_type == MSG_TYPE_CONTINUATION {
                // Parse the continuation message to get offset + length, then
                // enqueue it for later processing.
                let cont = crate::messages::continuation::parse(
                    &mut Cursor::new(msg_data),
                    offset_size,
                    length_size,
                    msg_data_size,
                )?;
                continuations.push((cont.offset, cont.length));
                messages.push(HdfMessage::ObjectHeaderContinuation);
            } else {
                let parsed = parse_message(
                    msg_type,
                    msg_data.len(),
                    &mut Cursor::new(msg_data),
                    offset_size,
                    length_size,
                )?;
                messages.push(parsed);
            }
        }

        Ok(())
    }

    // ------------------------------------------------------------------
    // Version 2
    // ------------------------------------------------------------------

    /// Parse a version-2 object header.
    ///
    /// Layout:
    /// ```text
    ///   signature  4 bytes  ("OHDR")
    ///   version    u8       (must be 2)
    ///   flags      u8
    ///   [optional timestamps — 4 x u32 if bit 5 of flags]
    ///   [optional attr phase change — 2 x u16 if bit 4 of flags]
    ///   chunk0_size  1/2/4/8 bytes (encoded size depends on bits 0-1 of flags)
    ///   <messages for chunk 0>
    ///   checksum   u32      (Jenkins lookup3 from "OHDR" through last byte before checksum)
    /// ```
    fn parse_v2(base: &Cursor<'_>, address: u64, offset_size: u8, length_size: u8) -> Result<Self> {
        let mut cursor = base.at_offset(address)?;

        // ---- Fixed prefix ----
        let sig = cursor.read_bytes(4)?;
        if sig != OHDR_SIGNATURE {
            return Err(Error::InvalidObjectHeaderSignature);
        }
        let version = cursor.read_u8()?;
        if version != 2 {
            return Err(Error::UnsupportedObjectHeaderVersion(version));
        }
        let flags = cursor.read_u8()?;

        // Bit 5 — timestamps stored.
        let modification_time = if (flags & 0x20) != 0 {
            let _access_time = cursor.read_u32_le()?;
            let mod_time = cursor.read_u32_le()?;
            let _change_time = cursor.read_u32_le()?;
            let _birth_time = cursor.read_u32_le()?;
            Some(mod_time)
        } else {
            None
        };

        // Bit 4 — non-default attribute storage phase change values.
        if (flags & 0x10) != 0 {
            let _max_compact = cursor.read_u16_le()?;
            let _min_dense = cursor.read_u16_le()?;
        }

        // Chunk#0 size — width depends on bits 0-1 of flags.
        let size_field_width = 1usize << (flags & 0x03);
        let chunk0_data_size = cursor.read_uvar(size_field_width)?;

        // Bit 2 — attribute creation order tracked (affects per-message envelope).
        let creation_order_tracked = (flags & 0x04) != 0;

        // Messages for chunk 0 run from the current position for
        // `chunk0_data_size` bytes.  The last 4 bytes of that range are the
        // checksum.
        let messages_start = cursor.position();
        let chunk0_end = messages_start + chunk0_data_size;

        // The checksum covers everything from "OHDR" through the last byte
        // before the checksum field.
        let checksum_start = address as usize;
        let checksum_end = chunk0_end as usize; // the checksum itself sits at chunk0_end
        let stored_checksum = {
            let mut ck = base.at_offset(chunk0_end)?;
            ck.read_u32_le()?
        };
        let computed = jenkins_lookup3(&base.data()[checksum_start..checksum_end]);
        if computed != stored_checksum {
            return Err(Error::ChecksumMismatch {
                expected: stored_checksum,
                actual: computed,
            });
        }

        let mut messages: Vec<HdfMessage> = Vec::new();
        let mut continuations: Vec<(u64, u64)> = Vec::new();

        Self::read_v2_messages(
            base,
            messages_start,
            chunk0_end,
            offset_size,
            length_size,
            creation_order_tracked,
            &mut messages,
            &mut continuations,
        )?;

        // Follow continuation chunks.
        let mut visited = std::collections::HashSet::new();
        while let Some((cont_offset, cont_length)) = continuations.pop() {
            enter_continuation(&mut visited, cont_offset)?;
            Self::read_v2_continuation_chunk(
                base,
                cont_offset,
                cont_length,
                offset_size,
                length_size,
                creation_order_tracked,
                &mut messages,
                &mut continuations,
            )?;
        }

        Ok(ObjectHeader {
            version: 2,
            messages,
            reference_count: 0, // v2 does not store a reference count in the header
            modification_time,
        })
    }

    fn parse_v2_storage(
        storage: &dyn Storage,
        address: u64,
        offset_size: u8,
        length_size: u8,
    ) -> Result<Self> {
        let fixed_prefix = storage.read_range(address, 6)?;
        let mut cursor = Cursor::new(fixed_prefix.as_ref());

        let sig = cursor.read_bytes(4)?;
        if sig != OHDR_SIGNATURE {
            return Err(Error::InvalidObjectHeaderSignature);
        }
        let version = cursor.read_u8()?;
        if version != 2 {
            return Err(Error::UnsupportedObjectHeaderVersion(version));
        }
        let flags = cursor.read_u8()?;

        let size_field_width = 1usize << (flags & 0x03);
        let mut prefix_len = 6usize;
        if (flags & 0x20) != 0 {
            prefix_len = checked_add_usize(prefix_len, 16, "v2 object header prefix length")?;
        }
        if (flags & 0x10) != 0 {
            prefix_len = checked_add_usize(prefix_len, 4, "v2 object header prefix length")?;
        }
        prefix_len = checked_add_usize(
            prefix_len,
            size_field_width,
            "v2 object header prefix length",
        )?;

        let prefix = storage.read_range(address, prefix_len)?;
        let mut cursor = Cursor::new(prefix.as_ref());
        let sig = cursor.read_bytes(4)?;
        if sig != OHDR_SIGNATURE {
            return Err(Error::InvalidObjectHeaderSignature);
        }
        let version = cursor.read_u8()?;
        if version != 2 {
            return Err(Error::UnsupportedObjectHeaderVersion(version));
        }
        let flags = cursor.read_u8()?;

        let modification_time = if (flags & 0x20) != 0 {
            let _access_time = cursor.read_u32_le()?;
            let mod_time = cursor.read_u32_le()?;
            let _change_time = cursor.read_u32_le()?;
            let _birth_time = cursor.read_u32_le()?;
            Some(mod_time)
        } else {
            None
        };

        if (flags & 0x10) != 0 {
            let _max_compact = cursor.read_u16_le()?;
            let _min_dense = cursor.read_u16_le()?;
        }

        let chunk0_data_size = cursor.read_uvar(size_field_width)?;
        let creation_order_tracked = (flags & 0x04) != 0;
        let messages_start = checked_usize(cursor.position(), "v2 object header message start")?;
        let chunk0_data_size = checked_usize(chunk0_data_size, "v2 object header chunk0 size")?;
        let chunk0_end = checked_add_usize(
            messages_start,
            chunk0_data_size,
            "v2 object header chunk0 end",
        )?;

        let chunk_with_checksum_len =
            checked_add_usize(chunk0_end, 4, "v2 object header chunk0 checksum end")?;
        let chunk = storage.read_range(address, chunk_with_checksum_len)?;
        let stored_checksum = u32::from_le_bytes(
            chunk.as_ref()[chunk0_end..chunk0_end + 4]
                .try_into()
                .unwrap(),
        );
        let computed = jenkins_lookup3(&chunk.as_ref()[..chunk0_end]);
        if computed != stored_checksum {
            return Err(Error::ChecksumMismatch {
                expected: stored_checksum,
                actual: computed,
            });
        }

        let mut messages = Vec::new();
        let mut continuations = Vec::new();
        Self::read_v2_messages_from_slice(
            &chunk.as_ref()[messages_start..chunk0_end],
            offset_size,
            length_size,
            creation_order_tracked,
            &mut messages,
            &mut continuations,
        )?;

        let mut visited = std::collections::HashSet::new();
        while let Some((cont_offset, cont_length)) = continuations.pop() {
            enter_continuation(&mut visited, cont_offset)?;
            Self::read_v2_continuation_chunk_storage(
                storage,
                cont_offset,
                cont_length,
                offset_size,
                length_size,
                creation_order_tracked,
                &mut messages,
                &mut continuations,
            )?;
        }

        Ok(ObjectHeader {
            version: 2,
            messages,
            reference_count: 0,
            modification_time,
        })
    }

    /// Read v2 messages from `start..end`.
    #[allow(clippy::too_many_arguments)]
    fn read_v2_messages(
        base: &Cursor<'_>,
        start: u64,
        end: u64,
        offset_size: u8,
        length_size: u8,
        creation_order_tracked: bool,
        messages: &mut Vec<HdfMessage>,
        continuations: &mut Vec<(u64, u64)>,
    ) -> Result<()> {
        let mut cursor = base.at_offset(start)?;

        // Minimum envelope: type(1) + size(2) + flags(1) = 4 bytes, optionally
        // +2 for creation order.
        let min_envelope = if creation_order_tracked { 6 } else { 4 };

        while cursor.position() + min_envelope as u64 <= end {
            let msg_type = cursor.read_u8()? as u16;
            let msg_data_size = cursor.read_u16_le()? as usize;
            let msg_flags = cursor.read_u8()?;

            if creation_order_tracked {
                let _creation_order = cursor.read_u16_le()?;
            }

            if msg_type == MSG_TYPE_NIL {
                if msg_data_size == 0
                    && base.data()[cursor.position() as usize..end as usize]
                        .iter()
                        .all(|byte| *byte == 0)
                {
                    break;
                }
                cursor.skip(msg_data_size)?;
                messages.push(HdfMessage::Nil);
                continue;
            }

            if cursor.position() + msg_data_size as u64 > end {
                return Err(Error::InvalidData(format!(
                    "v2 message data ({} bytes) extends past chunk end",
                    msg_data_size
                )));
            }

            let msg_data = cursor.read_bytes(msg_data_size)?;
            let is_shared = (msg_flags & 0x02) != 0;

            if is_shared {
                let shared_msg = crate::messages::shared::parse(
                    &mut Cursor::new(msg_data),
                    msg_type,
                    offset_size,
                    length_size,
                    msg_data_size,
                )?;
                messages.push(HdfMessage::Shared(shared_msg));
            } else if msg_type == MSG_TYPE_CONTINUATION {
                let cont = crate::messages::continuation::parse(
                    &mut Cursor::new(msg_data),
                    offset_size,
                    length_size,
                    msg_data_size,
                )?;
                continuations.push((cont.offset, cont.length));
                messages.push(HdfMessage::ObjectHeaderContinuation);
            } else {
                let parsed = parse_message(
                    msg_type,
                    msg_data.len(),
                    &mut Cursor::new(msg_data),
                    offset_size,
                    length_size,
                )?;
                messages.push(parsed);
            }
        }

        Ok(())
    }

    fn read_v1_messages_from_slice(
        data: &[u8],
        offset_size: u8,
        length_size: u8,
        messages: &mut Vec<HdfMessage>,
        continuations: &mut Vec<(u64, u64)>,
    ) -> Result<()> {
        let mut cursor = Cursor::new(data);
        while cursor.remaining() >= 8 {
            let msg_type = cursor.read_u16_le()?;
            let msg_data_size = cursor.read_u16_le()? as usize;
            let msg_flags = cursor.read_u8()?;
            let _reserved = cursor.read_bytes(3)?;

            if cursor.remaining() < msg_data_size as u64 {
                return Err(Error::InvalidData(format!(
                    "v1 message data ({} bytes) extends past header chunk end",
                    msg_data_size
                )));
            }

            if msg_type == MSG_TYPE_NIL {
                cursor.skip(msg_data_size)?;
                messages.push(HdfMessage::Nil);
                continue;
            }

            let msg_data = cursor.read_bytes(msg_data_size)?;
            let is_shared = (msg_flags & 0x02) != 0;
            if is_shared {
                let shared_msg = crate::messages::shared::parse(
                    &mut Cursor::new(msg_data),
                    msg_type,
                    offset_size,
                    length_size,
                    msg_data_size,
                )?;
                messages.push(HdfMessage::Shared(shared_msg));
            } else if msg_type == MSG_TYPE_CONTINUATION {
                let cont = crate::messages::continuation::parse(
                    &mut Cursor::new(msg_data),
                    offset_size,
                    length_size,
                    msg_data_size,
                )?;
                continuations.push((cont.offset, cont.length));
                messages.push(HdfMessage::ObjectHeaderContinuation);
            } else {
                let parsed = parse_message(
                    msg_type,
                    msg_data.len(),
                    &mut Cursor::new(msg_data),
                    offset_size,
                    length_size,
                )?;
                messages.push(parsed);
            }
        }
        Ok(())
    }

    fn read_v2_messages_from_slice(
        data: &[u8],
        offset_size: u8,
        length_size: u8,
        creation_order_tracked: bool,
        messages: &mut Vec<HdfMessage>,
        continuations: &mut Vec<(u64, u64)>,
    ) -> Result<()> {
        let mut cursor = Cursor::new(data);
        let min_envelope = if creation_order_tracked { 6 } else { 4 };

        while cursor.remaining() >= min_envelope as u64 {
            let msg_type = cursor.read_u8()? as u16;
            let msg_data_size = cursor.read_u16_le()? as usize;
            let msg_flags = cursor.read_u8()?;

            if creation_order_tracked {
                let _creation_order = cursor.read_u16_le()?;
            }

            if msg_type == MSG_TYPE_NIL {
                if msg_data_size == 0
                    && data[cursor.position() as usize..]
                        .iter()
                        .all(|byte| *byte == 0)
                {
                    break;
                }
                cursor.skip(msg_data_size)?;
                messages.push(HdfMessage::Nil);
                continue;
            }

            if cursor.remaining() < msg_data_size as u64 {
                return Err(Error::InvalidData(format!(
                    "v2 message data ({} bytes) extends past chunk end",
                    msg_data_size
                )));
            }

            let msg_data = cursor.read_bytes(msg_data_size)?;
            let is_shared = (msg_flags & 0x02) != 0;
            if is_shared {
                let shared_msg = crate::messages::shared::parse(
                    &mut Cursor::new(msg_data),
                    msg_type,
                    offset_size,
                    length_size,
                    msg_data_size,
                )?;
                messages.push(HdfMessage::Shared(shared_msg));
            } else if msg_type == MSG_TYPE_CONTINUATION {
                let cont = crate::messages::continuation::parse(
                    &mut Cursor::new(msg_data),
                    offset_size,
                    length_size,
                    msg_data_size,
                )?;
                continuations.push((cont.offset, cont.length));
                messages.push(HdfMessage::ObjectHeaderContinuation);
            } else {
                let parsed = parse_message(
                    msg_type,
                    msg_data.len(),
                    &mut Cursor::new(msg_data),
                    offset_size,
                    length_size,
                )?;
                messages.push(parsed);
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn read_v2_continuation_chunk_storage(
        storage: &dyn Storage,
        cont_offset: u64,
        cont_length: u64,
        offset_size: u8,
        length_size: u8,
        creation_order_tracked: bool,
        messages: &mut Vec<HdfMessage>,
        continuations: &mut Vec<(u64, u64)>,
    ) -> Result<()> {
        let cont_length = checked_usize(cont_length, "v2 object header continuation length")?;
        let chunk = storage.read_range(cont_offset, cont_length)?;
        if chunk.len() < 8 || chunk.as_ref()[..4] != OCHK_SIGNATURE {
            return Err(Error::InvalidObjectHeaderSignature);
        }
        let messages_end = chunk.len() - 4;
        let stored_checksum = u32::from_le_bytes(
            chunk.as_ref()[messages_end..messages_end + 4]
                .try_into()
                .unwrap(),
        );
        let computed = jenkins_lookup3(&chunk.as_ref()[..messages_end]);
        if computed != stored_checksum {
            return Err(Error::ChecksumMismatch {
                expected: stored_checksum,
                actual: computed,
            });
        }

        Self::read_v2_messages_from_slice(
            &chunk.as_ref()[4..messages_end],
            offset_size,
            length_size,
            creation_order_tracked,
            messages,
            continuations,
        )
    }

    /// Read and verify a v2 continuation chunk (`OCHK`).
    #[allow(clippy::too_many_arguments)]
    ///
    /// Layout:
    /// ```text
    ///   "OCHK"    4 bytes
    ///   messages  (cont_length - 4 - 4) bytes
    ///   checksum  u32
    /// ```
    fn read_v2_continuation_chunk(
        base: &Cursor<'_>,
        cont_offset: u64,
        cont_length: u64,
        offset_size: u8,
        length_size: u8,
        creation_order_tracked: bool,
        messages: &mut Vec<HdfMessage>,
        continuations: &mut Vec<(u64, u64)>,
    ) -> Result<()> {
        let mut cursor = base.at_offset(cont_offset)?;

        let sig = cursor.read_bytes(4)?;
        if sig != OCHK_SIGNATURE {
            return Err(Error::InvalidObjectHeaderSignature);
        }

        let chunk_end = cont_offset + cont_length;
        // The last 4 bytes of the chunk are the checksum.
        let messages_end = chunk_end - 4;
        let messages_start = cursor.position(); // right after "OCHK"

        // Verify checksum: covers "OCHK" through the byte before the checksum.
        let checksum_start = cont_offset as usize;
        let checksum_end = messages_end as usize;
        let stored_checksum = {
            let mut ck = base.at_offset(messages_end)?;
            ck.read_u32_le()?
        };
        let computed = jenkins_lookup3(&base.data()[checksum_start..checksum_end]);
        if computed != stored_checksum {
            return Err(Error::ChecksumMismatch {
                expected: stored_checksum,
                actual: computed,
            });
        }

        Self::read_v2_messages(
            base,
            messages_start,
            messages_end,
            offset_size,
            length_size,
            creation_order_tracked,
            messages,
            continuations,
        )
    }
}

fn select_shared_message(header: ObjectHeader, message_type: u16) -> Option<HdfMessage> {
    let mut first_real_message = None;
    for message in header.messages {
        match message {
            HdfMessage::Nil | HdfMessage::ObjectHeaderContinuation | HdfMessage::Shared(_) => {
                continue;
            }
            other if message_matches_type(&other, message_type) => return Some(other),
            other => {
                if first_real_message.is_none() {
                    first_real_message = Some(other);
                }
            }
        }
    }
    first_real_message
}

fn message_matches_type(message: &HdfMessage, message_type: u16) -> bool {
    use crate::messages::*;

    matches!(
        (message_type, message),
        (MSG_DATASPACE, HdfMessage::Dataspace(_))
            | (MSG_DATATYPE, HdfMessage::Datatype(_))
            | (MSG_FILL_VALUE, HdfMessage::FillValue(_))
            | (MSG_FILL_VALUE_OLD, HdfMessage::FillValue(_))
            | (MSG_DATA_LAYOUT, HdfMessage::DataLayout(_))
            | (MSG_FILTER_PIPELINE, HdfMessage::FilterPipeline(_))
            | (MSG_ATTRIBUTE, HdfMessage::Attribute(_))
            | (MSG_ATTRIBUTE_INFO, HdfMessage::AttributeInfo(_))
            | (MSG_LINK, HdfMessage::Link(_))
            | (MSG_LINK_INFO, HdfMessage::LinkInfo(_))
            | (MSG_GROUP_INFO, HdfMessage::GroupInfo(_))
            | (MSG_SYMBOL_TABLE, HdfMessage::SymbolTable(_))
            | (MSG_CONTINUATION, HdfMessage::Continuation(_))
            | (MSG_MODIFICATION_TIME, HdfMessage::ModificationTime(_))
            | (MSG_MODIFICATION_TIME_OLD, HdfMessage::ModificationTime(_))
            | (MSG_BTREE_K, HdfMessage::BTreeK(_))
            | (MSG_EXTERNAL_FILES, HdfMessage::ExternalFiles(_))
            | (MSG_SHARED_TABLE, HdfMessage::SharedTable(_))
            | (MSG_COMMENT, HdfMessage::Comment(_))
            | (MSG_REFERENCE_COUNT, HdfMessage::ReferenceCount(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checksum::jenkins_lookup3;
    use crate::storage::BytesStorage;

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    /// Build a v1 object header containing the given pre-encoded messages.
    /// Each entry in `raw_messages` is `(type_id, flags, payload)`.
    fn build_v1_header(raw_messages: &[(u16, u8, &[u8])], ref_count: u32) -> Vec<u8> {
        // Compute total message data size.
        let data_size: usize = raw_messages
            .iter()
            .map(|(_, _, payload)| 8 + payload.len()) // 8-byte envelope per message
            .sum();

        let mut buf = Vec::new();
        // Version
        buf.push(1);
        // Reserved
        buf.push(0);
        // Number of messages
        buf.extend_from_slice(&(raw_messages.len() as u16).to_le_bytes());
        // Reference count
        buf.extend_from_slice(&ref_count.to_le_bytes());
        // Header data size
        buf.extend_from_slice(&(data_size as u32).to_le_bytes());
        // Reserved padding (4 bytes)
        buf.extend_from_slice(&[0u8; 4]);

        // Messages
        for (type_id, flags, payload) in raw_messages {
            buf.extend_from_slice(&type_id.to_le_bytes());
            buf.extend_from_slice(&(payload.len() as u16).to_le_bytes());
            buf.push(*flags);
            buf.extend_from_slice(&[0u8; 3]); // reserved
            buf.extend_from_slice(payload);
        }

        buf
    }

    /// Build a v2 OHDR chunk#0 with the given raw messages.
    /// `flags` controls the header flags byte.  Timestamps and phase-change
    /// values are added automatically when the corresponding flag bits are set.
    /// Each entry in `raw_messages` is `(type_id, flags, payload)`.
    /// Returns the complete OHDR block including the trailing checksum.
    fn build_v2_header(
        header_flags: u8,
        raw_messages: &[(u8, u8, &[u8])],
        timestamps: Option<[u32; 4]>,
        phase_change: Option<(u16, u16)>,
    ) -> Vec<u8> {
        let creation_order = (header_flags & 0x04) != 0;

        // Compute message data size.
        let envelope_size: usize = if creation_order { 6 } else { 4 };
        let msg_data_size: usize = raw_messages
            .iter()
            .map(|(_, _, payload)| envelope_size + payload.len())
            .sum();

        let mut buf = Vec::new();
        // Signature
        buf.extend_from_slice(&OHDR_SIGNATURE);
        // Version
        buf.push(2);
        // Flags
        buf.push(header_flags);

        // Timestamps (bit 5)
        if let Some(ts) = timestamps {
            for &t in &ts {
                buf.extend_from_slice(&t.to_le_bytes());
            }
        }

        // Phase change (bit 4)
        if let Some((max_compact, min_dense)) = phase_change {
            buf.extend_from_slice(&max_compact.to_le_bytes());
            buf.extend_from_slice(&min_dense.to_le_bytes());
        }

        // Chunk#0 size field — encode using the width dictated by bits 0-1.
        let size_width = 1usize << (header_flags & 0x03);
        match size_width {
            1 => buf.push(msg_data_size as u8),
            2 => buf.extend_from_slice(&(msg_data_size as u16).to_le_bytes()),
            4 => buf.extend_from_slice(&(msg_data_size as u32).to_le_bytes()),
            8 => buf.extend_from_slice(&(msg_data_size as u64).to_le_bytes()),
            _ => unreachable!(),
        }

        // Messages
        for (type_id, mflags, payload) in raw_messages {
            buf.push(*type_id);
            buf.extend_from_slice(&(payload.len() as u16).to_le_bytes());
            buf.push(*mflags);
            if creation_order {
                buf.extend_from_slice(&0u16.to_le_bytes());
            }
            buf.extend_from_slice(payload);
        }

        // Checksum — covers everything so far.
        let ck = jenkins_lookup3(&buf);
        buf.extend_from_slice(&ck.to_le_bytes());

        buf
    }

    /// Build a v2 OCHK continuation chunk containing the given raw messages.
    fn build_v2_ochk(raw_messages: &[(u8, u8, &[u8])], creation_order: bool) -> Vec<u8> {
        let mut buf = Vec::new();
        // Signature
        buf.extend_from_slice(&OCHK_SIGNATURE);

        // Messages
        for (type_id, mflags, payload) in raw_messages {
            buf.push(*type_id);
            buf.extend_from_slice(&(payload.len() as u16).to_le_bytes());
            buf.push(*mflags);
            if creation_order {
                buf.extend_from_slice(&0u16.to_le_bytes());
            }
            buf.extend_from_slice(payload);
        }

        // Checksum over everything before the checksum itself.
        let ck = jenkins_lookup3(&buf);
        buf.extend_from_slice(&ck.to_le_bytes());

        buf
    }

    #[test]
    fn parse_v1_storage_accepts_header_near_eof() {
        let header = build_v1_header(&[], 7);
        let mut file_data = vec![0xAA; 3];
        let address = file_data.len() as u64;
        file_data.extend_from_slice(&header);
        let storage = BytesStorage::new(file_data);

        let hdr = ObjectHeader::parse_at_storage(&storage, address, 8, 8).unwrap();

        assert_eq!(hdr.version, 1);
        assert_eq!(hdr.reference_count, 7);
        assert!(hdr.messages.is_empty());
    }

    #[test]
    fn parse_v2_storage_accepts_header_near_eof() {
        let header = build_v2_header(0x00, &[], None, None);
        let mut file_data = vec![0xAA; 5];
        let address = file_data.len() as u64;
        file_data.extend_from_slice(&header);
        let storage = BytesStorage::new(file_data);

        let hdr = ObjectHeader::parse_at_storage(&storage, address, 8, 8).unwrap();

        assert_eq!(hdr.version, 2);
        assert!(hdr.messages.is_empty());
    }

    #[test]
    fn parse_v2_storage_rejects_oversized_chunk0_size() {
        let mut header = Vec::new();
        header.extend_from_slice(&OHDR_SIGNATURE);
        header.push(2);
        header.push(0x03);
        header.extend_from_slice(&u64::MAX.to_le_bytes());
        let storage = BytesStorage::new(header);

        let err = ObjectHeader::parse_at_storage(&storage, 0, 8, 8).unwrap_err();

        assert!(matches!(err, Error::InvalidData(_)));
    }

    // ------------------------------------------------------------------
    // Tests — Version 1
    // ------------------------------------------------------------------

    #[test]
    fn v1_empty_header() {
        let data = build_v1_header(&[], 1);
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert_eq!(hdr.version, 1);
        assert_eq!(hdr.reference_count, 1);
        assert!(hdr.messages.is_empty());
        assert!(hdr.modification_time.is_none());
    }

    #[test]
    fn v1_nil_message() {
        // A single nil message with 4 bytes of padding payload.
        let data = build_v1_header(&[(0x0000, 0, &[0u8; 4])], 1);
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert_eq!(hdr.messages.len(), 1);
        assert!(matches!(hdr.messages[0], HdfMessage::Nil));
    }

    #[test]
    fn v1_unknown_message() {
        // An unknown message type should be stored as HdfMessage::Unknown.
        let payload = [0xAA, 0xBB, 0xCC];
        let data = build_v1_header(&[(0x00FF, 0, &payload)], 2);
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert_eq!(hdr.reference_count, 2);
        assert_eq!(hdr.messages.len(), 1);
        match &hdr.messages[0] {
            HdfMessage::Unknown { type_id, data } => {
                assert_eq!(*type_id, 0x00FF);
                assert_eq!(data.as_slice(), &payload);
            }
            other => panic!("expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn v1_symbol_table_message() {
        // Type 0x0011 — symbol table message.
        // Payload: btree address (8 bytes) + heap address (8 bytes).
        let mut payload = Vec::new();
        payload.extend_from_slice(&0x1000u64.to_le_bytes());
        payload.extend_from_slice(&0x2000u64.to_le_bytes());

        let data = build_v1_header(&[(0x0011, 0, &payload)], 1);
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert_eq!(hdr.messages.len(), 1);
        match &hdr.messages[0] {
            HdfMessage::SymbolTable(st) => {
                assert_eq!(st.btree_address, 0x1000);
                assert_eq!(st.heap_address, 0x2000);
            }
            other => panic!("expected SymbolTable, got {:?}", other),
        }
    }

    #[test]
    fn v1_continuation_message() {
        // Build a continuation payload that points to a second chunk.
        // The second chunk contains one unknown message.
        let unknown_payload = [0xDD; 2];

        // Build the continuation target (a raw v1 message run, no header prefix).
        let mut cont_chunk = Vec::new();
        // message type 0x00FE
        cont_chunk.extend_from_slice(&0x00FEu16.to_le_bytes());
        // message data size
        cont_chunk.extend_from_slice(&(unknown_payload.len() as u16).to_le_bytes());
        // flags
        cont_chunk.push(0);
        // reserved
        cont_chunk.extend_from_slice(&[0u8; 3]);
        // payload
        cont_chunk.extend_from_slice(&unknown_payload);

        // We will place the continuation chunk after the main header.
        // First build the main header with a continuation message.
        let main_header_base_size = 16; // v1 prefix
                                        // The continuation message envelope = 8, payload = offset_size + length_size.
                                        // With offset_size=8, length_size=8, the continuation payload is 16 bytes.
        let cont_msg_envelope_size = 8 + 16; // 24
        let cont_chunk_offset = (main_header_base_size + cont_msg_envelope_size) as u64;

        let mut cont_payload = Vec::new();
        cont_payload.extend_from_slice(&cont_chunk_offset.to_le_bytes()); // offset
        cont_payload.extend_from_slice(&(cont_chunk.len() as u64).to_le_bytes()); // length

        let main_header = build_v1_header(&[(MSG_TYPE_CONTINUATION, 0, &cont_payload)], 1);

        // Concatenate main header + continuation chunk.
        let mut file_data = main_header;
        assert_eq!(file_data.len() as u64, cont_chunk_offset);
        file_data.extend_from_slice(&cont_chunk);

        let hdr = ObjectHeader::parse_at(&file_data, 0, 8, 8).unwrap();
        // Should have the continuation marker + the unknown message from the continuation chunk.
        assert_eq!(hdr.messages.len(), 2);
        assert!(matches!(
            hdr.messages[0],
            HdfMessage::ObjectHeaderContinuation
        ));
        match &hdr.messages[1] {
            HdfMessage::Unknown { type_id, data } => {
                assert_eq!(*type_id, 0x00FE);
                assert_eq!(data.as_slice(), &unknown_payload);
            }
            other => panic!("expected Unknown from continuation, got {:?}", other),
        }
    }

    #[test]
    fn v1_nonzero_address_offset() {
        // Place the header at a non-zero offset in the file.
        let prefix_pad = vec![0xFFu8; 64];
        let header = build_v1_header(&[(0x00AA, 0, &[0x01])], 3);

        let mut file_data = prefix_pad;
        file_data.extend_from_slice(&header);

        let hdr = ObjectHeader::parse_at(&file_data, 64, 8, 8).unwrap();
        assert_eq!(hdr.version, 1);
        assert_eq!(hdr.reference_count, 3);
        assert_eq!(hdr.messages.len(), 1);
    }

    #[test]
    fn v1_bad_version() {
        let mut data = build_v1_header(&[], 1);
        data[0] = 3; // corrupt version to 3
        let err = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap_err();
        assert!(matches!(err, Error::UnsupportedObjectHeaderVersion(3)));
    }

    // ------------------------------------------------------------------
    // Tests — Version 2
    // ------------------------------------------------------------------

    #[test]
    fn v2_empty_header() {
        // Flags=0 → 1-byte size field, no timestamps, no phase change, no creation order.
        let data = build_v2_header(0x00, &[], None, None);
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert_eq!(hdr.version, 2);
        assert!(hdr.messages.is_empty());
        assert!(hdr.modification_time.is_none());
    }

    #[test]
    fn v2_nil_message() {
        let data = build_v2_header(0x00, &[(0x00, 0, &[0u8; 3])], None, None);
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert_eq!(hdr.messages.len(), 1);
        assert!(matches!(hdr.messages[0], HdfMessage::Nil));
    }

    #[test]
    fn v2_unknown_message() {
        let payload = [0x11, 0x22];
        let data = build_v2_header(0x00, &[(0xFE, 0, &payload)], None, None);
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert_eq!(hdr.messages.len(), 1);
        match &hdr.messages[0] {
            HdfMessage::Unknown { type_id, data } => {
                assert_eq!(*type_id, 0x00FE);
                assert_eq!(data.as_slice(), &payload);
            }
            other => panic!("expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn v2_with_timestamps() {
        // Flags: bit 5 (timestamps) + bits 0-1 = 0 (1-byte size field).
        let flags = 0x20;
        let ts = [1000u32, 2000, 3000, 4000]; // access, modification, change, birth
        let data = build_v2_header(flags, &[], Some(ts), None);
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert_eq!(hdr.modification_time, Some(2000));
    }

    #[test]
    fn v2_with_phase_change() {
        // Flags: bit 4 (phase change) + bits 0-1 = 0.
        let flags = 0x10;
        let data = build_v2_header(flags, &[], None, Some((8, 6)));
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert!(hdr.messages.is_empty());
    }

    #[test]
    fn v2_with_creation_order() {
        // Flags: bit 2 (creation order tracked) + bits 0-1 = 0.
        let flags = 0x04;
        let payload = [0xAA];
        let data = build_v2_header(flags, &[(0xFE, 0, &payload)], None, None);
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert_eq!(hdr.messages.len(), 1);
        match &hdr.messages[0] {
            HdfMessage::Unknown { type_id, .. } => assert_eq!(*type_id, 0x00FE),
            other => panic!("expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn v2_2byte_size_field() {
        // bits 0-1 = 1 → 2-byte size field.
        let flags = 0x01;
        let payload = [0x42; 5];
        let data = build_v2_header(flags, &[(0xFE, 0, &payload)], None, None);
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert_eq!(hdr.messages.len(), 1);
    }

    #[test]
    fn v2_4byte_size_field() {
        // bits 0-1 = 2 → 4-byte size field.
        let flags = 0x02;
        let data = build_v2_header(flags, &[(0xFE, 0, &[0x01])], None, None);
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert_eq!(hdr.messages.len(), 1);
    }

    #[test]
    fn v2_8byte_size_field() {
        // bits 0-1 = 3 → 8-byte size field.
        let flags = 0x03;
        let data = build_v2_header(flags, &[(0xFE, 0, &[0x01])], None, None);
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert_eq!(hdr.messages.len(), 1);
    }

    #[test]
    fn v2_checksum_mismatch() {
        let mut data = build_v2_header(0x00, &[(0xFE, 0, &[0x01])], None, None);
        // Corrupt the last byte (part of checksum).
        let last = data.len() - 1;
        data[last] ^= 0xFF;
        let err = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap_err();
        assert!(matches!(err, Error::ChecksumMismatch { .. }));
    }

    #[test]
    fn v2_continuation_chunk() {
        // Build a continuation chunk (OCHK) that holds one unknown message.
        let unknown_payload = [0xCC; 3];
        let ochk = build_v2_ochk(&[(0xFD, 0, &unknown_payload)], false);

        // The continuation message payload is offset(8) + length(8) = 16 bytes.
        // We will compute the offset of the OCHK after building the main OHDR.
        // Strategy: build OHDR first with a placeholder, measure its size,
        // set the actual offset, then rebuild.

        // Placeholder continuation payload (will rewrite).
        let mut cont_payload = vec![0u8; 16];

        // Build OHDR with the continuation message.  The OHDR occupies:
        //   4 (sig) + 1 (ver) + 1 (flags) + 1 (size field, flags=0) + messages + 4 (checksum)
        // Message envelope: type(1) + size(2) + flags(1) = 4; payload = 16.
        // Total OHDR = 4 + 1 + 1 + 1 + 4 + 16 + 4 = 31 bytes.
        // The OCHK starts at byte 31.

        // We need the offset to be the byte where OCHK starts.
        // OHDR: sig(4) + ver(1) + flags(1) + size(1) + [envelope(4)+payload(16)] + checksum(4) = 31
        let ohdr_size = 4 + 1 + 1 + 1 + (4 + cont_payload.len()) + 4;
        let ochk_offset = ohdr_size as u64;

        // Rebuild continuation payload with correct offset.
        cont_payload.clear();
        cont_payload.extend_from_slice(&ochk_offset.to_le_bytes());
        cont_payload.extend_from_slice(&(ochk.len() as u64).to_le_bytes());

        let ohdr = build_v2_header(0x00, &[(0x10, 0, &cont_payload)], None, None);
        assert_eq!(ohdr.len(), ohdr_size);

        let mut file_data = ohdr;
        file_data.extend_from_slice(&ochk);

        let hdr = ObjectHeader::parse_at(&file_data, 0, 8, 8).unwrap();
        // Should have: continuation marker + unknown message from OCHK.
        assert_eq!(hdr.messages.len(), 2);
        assert!(matches!(
            hdr.messages[0],
            HdfMessage::ObjectHeaderContinuation
        ));
        match &hdr.messages[1] {
            HdfMessage::Unknown { type_id, data } => {
                assert_eq!(*type_id, 0x00FD);
                assert_eq!(data.as_slice(), &unknown_payload);
            }
            other => panic!("expected Unknown from OCHK, got {:?}", other),
        }
    }

    #[test]
    fn v1_continuation_cycle_is_rejected() {
        // A continuation chunk whose only message is a continuation pointing
        // back at itself must error instead of looping forever.
        let main_header_base_size = 16;
        let cont_msg_envelope_size = 8 + 16;
        let cont_chunk_offset = (main_header_base_size + cont_msg_envelope_size) as u64;

        // The self-referential chunk: one continuation message pointing at
        // this chunk's own offset.
        let mut cont_chunk = Vec::new();
        cont_chunk.extend_from_slice(&MSG_TYPE_CONTINUATION.to_le_bytes());
        cont_chunk.extend_from_slice(&16u16.to_le_bytes());
        cont_chunk.push(0);
        cont_chunk.extend_from_slice(&[0u8; 3]);
        let cont_chunk_len = (cont_chunk.len() + 16) as u64;
        cont_chunk.extend_from_slice(&cont_chunk_offset.to_le_bytes());
        cont_chunk.extend_from_slice(&cont_chunk_len.to_le_bytes());

        let mut cont_payload = Vec::new();
        cont_payload.extend_from_slice(&cont_chunk_offset.to_le_bytes());
        cont_payload.extend_from_slice(&cont_chunk_len.to_le_bytes());

        let main_header = build_v1_header(&[(MSG_TYPE_CONTINUATION, 0, &cont_payload)], 1);
        let mut file_data = main_header;
        assert_eq!(file_data.len() as u64, cont_chunk_offset);
        file_data.extend_from_slice(&cont_chunk);

        let err = ObjectHeader::parse_at(&file_data, 0, 8, 8).unwrap_err();
        assert!(
            err.to_string().contains("continuation cycle"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn v2_continuation_cycle_is_rejected() {
        // An OCHK whose only message is a continuation pointing back at the
        // same OCHK must error instead of looping forever.
        let mut cont_payload = vec![0u8; 16];
        let ohdr_size = 4 + 1 + 1 + 1 + (4 + cont_payload.len()) + 4;
        let ochk_offset = ohdr_size as u64;

        // Build the self-referential OCHK with a placeholder payload first to
        // learn its length, then rebuild with the real values.
        let placeholder = build_v2_ochk(&[(0x10, 0, &[0u8; 16])], false);
        let ochk_len = placeholder.len() as u64;
        let mut self_payload = Vec::new();
        self_payload.extend_from_slice(&ochk_offset.to_le_bytes());
        self_payload.extend_from_slice(&ochk_len.to_le_bytes());
        let ochk = build_v2_ochk(&[(0x10, 0, &self_payload)], false);
        assert_eq!(ochk.len() as u64, ochk_len);

        cont_payload.clear();
        cont_payload.extend_from_slice(&ochk_offset.to_le_bytes());
        cont_payload.extend_from_slice(&ochk_len.to_le_bytes());
        let ohdr = build_v2_header(0x00, &[(0x10, 0, &cont_payload)], None, None);
        assert_eq!(ohdr.len(), ohdr_size);

        let mut file_data = ohdr;
        file_data.extend_from_slice(&ochk);

        let err = ObjectHeader::parse_at(&file_data, 0, 8, 8).unwrap_err();
        assert!(
            err.to_string().contains("continuation cycle"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn v1_storage_continuation_cycle_is_rejected() {
        let main_header_base_size = 16;
        let cont_msg_envelope_size = 8 + 16;
        let cont_chunk_offset = (main_header_base_size + cont_msg_envelope_size) as u64;

        let mut cont_chunk = Vec::new();
        cont_chunk.extend_from_slice(&MSG_TYPE_CONTINUATION.to_le_bytes());
        cont_chunk.extend_from_slice(&16u16.to_le_bytes());
        cont_chunk.push(0);
        cont_chunk.extend_from_slice(&[0u8; 3]);
        let cont_chunk_len = (cont_chunk.len() + 16) as u64;
        cont_chunk.extend_from_slice(&cont_chunk_offset.to_le_bytes());
        cont_chunk.extend_from_slice(&cont_chunk_len.to_le_bytes());

        let mut cont_payload = Vec::new();
        cont_payload.extend_from_slice(&cont_chunk_offset.to_le_bytes());
        cont_payload.extend_from_slice(&cont_chunk_len.to_le_bytes());

        let mut file_data = build_v1_header(&[(MSG_TYPE_CONTINUATION, 0, &cont_payload)], 1);
        assert_eq!(file_data.len() as u64, cont_chunk_offset);
        file_data.extend_from_slice(&cont_chunk);
        let storage = BytesStorage::new(file_data);

        let err = ObjectHeader::parse_at_storage(&storage, 0, 8, 8).unwrap_err();
        assert!(
            err.to_string().contains("continuation cycle"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn v2_storage_continuation_cycle_is_rejected() {
        let mut cont_payload = vec![0u8; 16];
        let ohdr_size = 4 + 1 + 1 + 1 + (4 + cont_payload.len()) + 4;
        let ochk_offset = ohdr_size as u64;

        let placeholder = build_v2_ochk(&[(0x10, 0, &[0u8; 16])], false);
        let ochk_len = placeholder.len() as u64;
        let mut self_payload = Vec::new();
        self_payload.extend_from_slice(&ochk_offset.to_le_bytes());
        self_payload.extend_from_slice(&ochk_len.to_le_bytes());
        let ochk = build_v2_ochk(&[(0x10, 0, &self_payload)], false);

        cont_payload.clear();
        cont_payload.extend_from_slice(&ochk_offset.to_le_bytes());
        cont_payload.extend_from_slice(&ochk_len.to_le_bytes());
        let mut file_data = build_v2_header(0x00, &[(0x10, 0, &cont_payload)], None, None);
        file_data.extend_from_slice(&ochk);
        let storage = BytesStorage::new(file_data);

        let err = ObjectHeader::parse_at_storage(&storage, 0, 8, 8).unwrap_err();
        assert!(
            err.to_string().contains("continuation cycle"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn v2_ochk_checksum_mismatch() {
        let unknown_payload = [0xCC; 3];
        let mut ochk = build_v2_ochk(&[(0xFD, 0, &unknown_payload)], false);
        // Corrupt OCHK checksum.
        let last = ochk.len() - 1;
        ochk[last] ^= 0xFF;

        let ohdr_size = 4 + 1 + 1 + 1 + (4 + 16) + 4; // 31
        let ochk_offset = ohdr_size as u64;

        let mut cont_payload = Vec::new();
        cont_payload.extend_from_slice(&ochk_offset.to_le_bytes());
        cont_payload.extend_from_slice(&(ochk.len() as u64).to_le_bytes());

        let ohdr = build_v2_header(0x00, &[(0x10, 0, &cont_payload)], None, None);
        let mut file_data = ohdr;
        file_data.extend_from_slice(&ochk);

        let err = ObjectHeader::parse_at(&file_data, 0, 8, 8).unwrap_err();
        assert!(matches!(err, Error::ChecksumMismatch { .. }));
    }

    #[test]
    fn v2_multiple_messages() {
        // Two unknown messages in the same chunk.
        let p1 = [0x01, 0x02];
        let p2 = [0x03, 0x04, 0x05];
        let data = build_v2_header(0x00, &[(0xA0, 0, &p1), (0xA1, 0, &p2)], None, None);
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert_eq!(hdr.messages.len(), 2);
        match &hdr.messages[0] {
            HdfMessage::Unknown { type_id, .. } => assert_eq!(*type_id, 0x00A0),
            other => panic!("expected Unknown 0xA0, got {:?}", other),
        }
        match &hdr.messages[1] {
            HdfMessage::Unknown { type_id, .. } => assert_eq!(*type_id, 0x00A1),
            other => panic!("expected Unknown 0xA1, got {:?}", other),
        }
    }

    #[test]
    fn v2_zero_length_nil_before_more_messages() {
        let p1 = [0xAA];
        let p2 = [0xBB];
        let data = build_v2_header(
            0x04,
            &[(0xFE, 0, &p1), (0x00, 0, &[]), (0xFD, 0, &p2)],
            None,
            None,
        );
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert_eq!(hdr.messages.len(), 3);
        assert!(matches!(hdr.messages[0], HdfMessage::Unknown { .. }));
        assert!(matches!(hdr.messages[1], HdfMessage::Nil));
        assert!(matches!(hdr.messages[2], HdfMessage::Unknown { .. }));
    }

    #[test]
    fn v2_nonzero_address() {
        // Place the OHDR at offset 128 in a larger buffer.
        let prefix_pad = vec![0u8; 128];
        let ohdr = build_v2_header(0x00, &[(0xFE, 0, &[0x42])], None, None);

        let mut file_data = prefix_pad;
        file_data.extend_from_slice(&ohdr);

        let hdr = ObjectHeader::parse_at(&file_data, 128, 8, 8).unwrap();
        assert_eq!(hdr.version, 2);
        assert_eq!(hdr.messages.len(), 1);
    }

    #[test]
    fn v2_all_flags_combined() {
        // Combine timestamps (0x20) + phase change (0x10) + creation order (0x04) + 2-byte size (0x01).
        let flags = 0x20 | 0x10 | 0x04 | 0x01;
        let ts = [100u32, 200, 300, 400];
        let payload = [0xBB];
        let data = build_v2_header(flags, &[(0xFE, 0, &payload)], Some(ts), Some((12, 8)));
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert_eq!(hdr.version, 2);
        assert_eq!(hdr.modification_time, Some(200));
        assert_eq!(hdr.messages.len(), 1);
    }

    #[test]
    fn v1_multiple_messages() {
        // Two messages in a single v1 header.
        let p1 = [0xAA; 4];
        let p2 = [0xBB; 8];
        let data = build_v1_header(&[(0x00FF, 0, &p1), (0x00FE, 0, &p2)], 5);
        let hdr = ObjectHeader::parse_at(&data, 0, 8, 8).unwrap();
        assert_eq!(hdr.version, 1);
        assert_eq!(hdr.reference_count, 5);
        assert_eq!(hdr.messages.len(), 2);
    }

    #[test]
    fn v1_4byte_offsets() {
        // Verify correct operation with 4-byte offset/length sizes.
        // Symbol table message with 4-byte addresses.
        let mut payload = Vec::new();
        payload.extend_from_slice(&0x1000u32.to_le_bytes());
        payload.extend_from_slice(&0x2000u32.to_le_bytes());

        let data = build_v1_header(&[(0x0011, 0, &payload)], 1);
        let hdr = ObjectHeader::parse_at(&data, 0, 4, 4).unwrap();
        assert_eq!(hdr.messages.len(), 1);
        match &hdr.messages[0] {
            HdfMessage::SymbolTable(st) => {
                assert_eq!(st.btree_address, 0x1000);
                assert_eq!(st.heap_address, 0x2000);
            }
            other => panic!("expected SymbolTable, got {:?}", other),
        }
    }
}
