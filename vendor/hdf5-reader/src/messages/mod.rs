//! HDF5 object header message parsing.
//!
//! Each object header contains a sequence of messages identified by a 16-bit
//! type ID. This module dispatches to type-specific parsers and collects the
//! results into `HdfMessage` variants.

pub mod attribute;
pub mod attribute_info;
pub mod btree_k;
pub mod continuation;
pub mod dataspace;
pub mod datatype;
pub mod external_files;
pub mod fill_value;
pub mod filter_pipeline;
pub mod group_info;
pub mod layout;
pub mod link;
pub mod link_info;
pub mod modification_time;
pub mod shared;
pub mod shared_table;
pub mod symbol_table_msg;

// Re-exports for convenience.
pub use dataspace::DataspaceMessage;
pub use datatype::Datatype;

use crate::error::Result;
use crate::io::Cursor;

// ---------------------------------------------------------------------------
// Message type IDs (from the HDF5 specification)
// ---------------------------------------------------------------------------

/// NIL message — padding in the header.
pub const MSG_NIL: u16 = 0x0000;
/// Dataspace message.
pub const MSG_DATASPACE: u16 = 0x0001;
/// Link info message (v2 groups).
pub const MSG_LINK_INFO: u16 = 0x0002;
/// Datatype message.
pub const MSG_DATATYPE: u16 = 0x0003;
/// Old fill value message (deprecated).
pub const MSG_FILL_VALUE_OLD: u16 = 0x0004;
/// Fill value message.
pub const MSG_FILL_VALUE: u16 = 0x0005;
/// Link message (v2 groups).
pub const MSG_LINK: u16 = 0x0006;
/// External data files message.
pub const MSG_EXTERNAL_FILES: u16 = 0x0007;
/// Data layout message.
pub const MSG_DATA_LAYOUT: u16 = 0x0008;
/// Bogus message (testing only, should never appear).
pub const MSG_BOGUS: u16 = 0x0009;
/// Group info message (v2 groups).
pub const MSG_GROUP_INFO: u16 = 0x000A;
/// Filter pipeline message.
pub const MSG_FILTER_PIPELINE: u16 = 0x000B;
/// Attribute message.
pub const MSG_ATTRIBUTE: u16 = 0x000C;
/// Object comment message.
pub const MSG_COMMENT: u16 = 0x000D;
/// Old modification time message (deprecated).
pub const MSG_MODIFICATION_TIME_OLD: u16 = 0x000E;
/// Shared message table message.
pub const MSG_SHARED_TABLE: u16 = 0x000F;
/// Object header continuation message.
pub const MSG_CONTINUATION: u16 = 0x0010;
/// Symbol table message (v1 groups).
pub const MSG_SYMBOL_TABLE: u16 = 0x0011;
/// Modification time message.
pub const MSG_MODIFICATION_TIME: u16 = 0x0012;
/// B-tree 'K' values message.
pub const MSG_BTREE_K: u16 = 0x0013;
/// Driver info message.
pub const MSG_DRIVER_INFO: u16 = 0x0014;
/// Attribute info message.
pub const MSG_ATTRIBUTE_INFO: u16 = 0x0015;
/// Object reference count message.
pub const MSG_REFERENCE_COUNT: u16 = 0x0016;
/// File space info message (v2).
pub const MSG_FILE_SPACE_INFO: u16 = 0x0018;

// ---------------------------------------------------------------------------
// Unified message enum
// ---------------------------------------------------------------------------

/// A parsed HDF5 header message.
#[derive(Debug, Clone)]
pub enum HdfMessage {
    /// Nil (padding) — no payload.
    Nil,
    /// Dataspace (shape).
    Dataspace(dataspace::DataspaceMessage),
    /// Datatype (element type).
    Datatype(datatype::DatatypeMessage),
    /// Fill value (old or new).
    FillValue(fill_value::FillValueMessage),
    /// Data layout (compact / contiguous / chunked).
    DataLayout(layout::DataLayoutMessage),
    /// Filter pipeline (compression, shuffle, etc.).
    FilterPipeline(filter_pipeline::FilterPipelineMessage),
    /// Attribute (name + type + data).
    Attribute(attribute::AttributeMessage),
    /// Attribute info (dense attribute storage addresses).
    AttributeInfo(attribute_info::AttributeInfoMessage),
    /// Link (v2 group child).
    Link(link::LinkMessage),
    /// Link info (dense link storage addresses).
    LinkInfo(link_info::LinkInfoMessage),
    /// Group info (storage hints for v2 groups).
    GroupInfo(group_info::GroupInfoMessage),
    /// Symbol table (v1 group child navigation).
    SymbolTable(symbol_table_msg::SymbolTableMessage),
    /// Header continuation (pointer to more messages).
    Continuation(continuation::ContinuationMessage),
    /// Modification time.
    ModificationTime(modification_time::ModificationTimeMessage),
    /// B-tree K values.
    BTreeK(btree_k::BTreeKMessage),
    /// External data files.
    ExternalFiles(external_files::ExternalFilesMessage),
    /// Pointer to the file-level SOHM master table.
    SharedTable(shared_table::SharedTableMessage),
    /// Shared message wrapper.
    Shared(shared::SharedMessage),
    /// Object header continuation (marker only — the parser follows the
    /// continuation internally, but records that one was encountered).
    ObjectHeaderContinuation,
    /// Comment (plain text).
    Comment(String),
    /// Object reference count.
    ReferenceCount(u32),
    /// Unknown or unimplemented message type — raw bytes preserved.
    Unknown { type_id: u16, data: Vec<u8> },
}

/// Parse a single header message given its type ID, size, and a cursor
/// positioned at the start of the message payload.
///
/// `offset_size` and `length_size` come from the superblock.
pub fn parse_message(
    type_id: u16,
    msg_size: usize,
    cursor: &mut Cursor<'_>,
    offset_size: u8,
    length_size: u8,
) -> Result<HdfMessage> {
    // Short-circuit for NIL (padding) messages.
    if type_id == MSG_NIL {
        if msg_size > 0 {
            cursor.skip(msg_size)?;
        }
        return Ok(HdfMessage::Nil);
    }

    match type_id {
        MSG_DATASPACE => {
            let msg = dataspace::parse(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::Dataspace(msg))
        }
        MSG_DATATYPE => {
            let msg = datatype::parse(cursor, msg_size)?;
            Ok(HdfMessage::Datatype(msg))
        }
        MSG_FILL_VALUE_OLD => {
            let msg = fill_value::parse_old(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::FillValue(msg))
        }
        MSG_FILL_VALUE => {
            let msg = fill_value::parse_new(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::FillValue(msg))
        }
        MSG_DATA_LAYOUT => {
            let msg = layout::parse(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::DataLayout(msg))
        }
        MSG_FILTER_PIPELINE => {
            let msg = filter_pipeline::parse(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::FilterPipeline(msg))
        }
        MSG_ATTRIBUTE => {
            let msg = attribute::parse(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::Attribute(msg))
        }
        MSG_ATTRIBUTE_INFO => {
            let msg = attribute_info::parse(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::AttributeInfo(msg))
        }
        MSG_LINK => {
            let msg = link::parse(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::Link(msg))
        }
        MSG_LINK_INFO => {
            let msg = link_info::parse(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::LinkInfo(msg))
        }
        MSG_GROUP_INFO => {
            let msg = group_info::parse(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::GroupInfo(msg))
        }
        MSG_SYMBOL_TABLE => {
            let msg = symbol_table_msg::parse(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::SymbolTable(msg))
        }
        MSG_CONTINUATION => {
            let msg = continuation::parse(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::Continuation(msg))
        }
        MSG_MODIFICATION_TIME_OLD => {
            let msg = modification_time::parse_old(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::ModificationTime(msg))
        }
        MSG_MODIFICATION_TIME => {
            let msg = modification_time::parse_new(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::ModificationTime(msg))
        }
        MSG_BTREE_K => {
            let msg = btree_k::parse(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::BTreeK(msg))
        }
        MSG_EXTERNAL_FILES => {
            let msg = external_files::parse(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::ExternalFiles(msg))
        }
        MSG_SHARED_TABLE => {
            let msg = shared_table::parse(cursor, offset_size, length_size, msg_size)?;
            Ok(HdfMessage::SharedTable(msg))
        }
        MSG_COMMENT => {
            let comment = cursor.read_fixed_string(msg_size)?;
            Ok(HdfMessage::Comment(comment))
        }
        MSG_REFERENCE_COUNT => {
            let count = cursor.read_u32_le()?;
            if msg_size > 4 {
                cursor.skip(msg_size - 4)?;
            }
            Ok(HdfMessage::ReferenceCount(count))
        }
        _ => {
            // Unknown or unimplemented message — preserve raw bytes.
            let data = if msg_size > 0 {
                cursor.read_bytes(msg_size)?.to_vec()
            } else {
                vec![]
            };
            Ok(HdfMessage::Unknown { type_id, data })
        }
    }
}

/// Returns a human-readable name for a message type ID.
pub fn message_type_name(type_id: u16) -> &'static str {
    match type_id {
        MSG_NIL => "NIL",
        MSG_DATASPACE => "Dataspace",
        MSG_LINK_INFO => "LinkInfo",
        MSG_DATATYPE => "Datatype",
        MSG_FILL_VALUE_OLD => "FillValue (old)",
        MSG_FILL_VALUE => "FillValue",
        MSG_LINK => "Link",
        MSG_EXTERNAL_FILES => "ExternalFiles",
        MSG_DATA_LAYOUT => "DataLayout",
        MSG_BOGUS => "Bogus",
        MSG_GROUP_INFO => "GroupInfo",
        MSG_FILTER_PIPELINE => "FilterPipeline",
        MSG_ATTRIBUTE => "Attribute",
        MSG_COMMENT => "Comment",
        MSG_MODIFICATION_TIME_OLD => "ModificationTime (old)",
        MSG_SHARED_TABLE => "SharedTable",
        MSG_CONTINUATION => "Continuation",
        MSG_SYMBOL_TABLE => "SymbolTable",
        MSG_MODIFICATION_TIME => "ModificationTime",
        MSG_BTREE_K => "BTreeK",
        MSG_DRIVER_INFO => "DriverInfo",
        MSG_ATTRIBUTE_INFO => "AttributeInfo",
        MSG_REFERENCE_COUNT => "ReferenceCount",
        MSG_FILE_SPACE_INFO => "FileSpaceInfo",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_nil_message() {
        let data = [0u8; 16];
        let mut cursor = Cursor::new(&data);
        let msg = parse_message(MSG_NIL, 16, &mut cursor, 8, 8).unwrap();
        assert!(matches!(msg, HdfMessage::Nil));
        assert_eq!(cursor.position(), 16);
    }

    #[test]
    fn parse_unknown_message() {
        let data = [0xAA, 0xBB, 0xCC, 0xDD];
        let mut cursor = Cursor::new(&data);
        let msg = parse_message(0xFFFF, 4, &mut cursor, 8, 8).unwrap();
        match msg {
            HdfMessage::Unknown { type_id, data } => {
                assert_eq!(type_id, 0xFFFF);
                assert_eq!(data, vec![0xAA, 0xBB, 0xCC, 0xDD]);
            }
            other => panic!("expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn parse_comment_message() {
        let data = b"hello world\0\0\0\0\0";
        let mut cursor = Cursor::new(data.as_ref());
        let msg = parse_message(MSG_COMMENT, 16, &mut cursor, 8, 8).unwrap();
        match msg {
            HdfMessage::Comment(s) => assert_eq!(s, "hello world"),
            other => panic!("expected Comment, got {:?}", other),
        }
    }

    #[test]
    fn parse_reference_count() {
        let data = 42u32.to_le_bytes();
        let mut cursor = Cursor::new(&data);
        let msg = parse_message(MSG_REFERENCE_COUNT, 4, &mut cursor, 8, 8).unwrap();
        match msg {
            HdfMessage::ReferenceCount(n) => assert_eq!(n, 42),
            other => panic!("expected ReferenceCount, got {:?}", other),
        }
    }

    #[test]
    fn parse_symbol_table_via_dispatch() {
        let mut data = Vec::new();
        data.extend_from_slice(&0x1234u64.to_le_bytes());
        data.extend_from_slice(&0x5678u64.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse_message(MSG_SYMBOL_TABLE, data.len(), &mut cursor, 8, 8).unwrap();
        match msg {
            HdfMessage::SymbolTable(st) => {
                assert_eq!(st.btree_address, 0x1234);
                assert_eq!(st.heap_address, 0x5678);
            }
            other => panic!("expected SymbolTable, got {:?}", other),
        }
    }

    #[test]
    fn parse_continuation_via_dispatch() {
        let mut data = Vec::new();
        data.extend_from_slice(&0xAAAAu64.to_le_bytes());
        data.extend_from_slice(&512u64.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse_message(MSG_CONTINUATION, data.len(), &mut cursor, 8, 8).unwrap();
        match msg {
            HdfMessage::Continuation(c) => {
                assert_eq!(c.offset, 0xAAAA);
                assert_eq!(c.length, 512);
            }
            other => panic!("expected Continuation, got {:?}", other),
        }
    }

    #[test]
    fn message_type_names_cover_known_and_unknown_ids() {
        assert_eq!(message_type_name(MSG_DATASPACE), "Dataspace");
        assert_eq!(message_type_name(MSG_DATATYPE), "Datatype");
        assert_eq!(message_type_name(0x9999), "Unknown");
    }
}
