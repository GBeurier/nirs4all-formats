//! Shared object-header message table (SOHM).
//!
//! The superblock extension can point at a file-level `SMTB` table containing
//! one or more shared-message indexes. Each index is either an `SMLI` list or
//! a v2 B-tree with record type 7.

use std::sync::Arc;

use crate::btree_v2::{self, BTreeV2Record};
use crate::checksum::jenkins_lookup3;
use crate::error::{Error, Result};
use crate::filters::FilterRegistry;
use crate::fractal_heap::FractalHeap;
use crate::io::Cursor;
use crate::messages::{parse_message, HdfMessage};
use crate::storage::Storage;

const SMTB_SIGNATURE: [u8; 4] = *b"SMTB";
const SMLI_SIGNATURE: [u8; 4] = *b"SMLI";

/// File-level SOHM master table.
#[derive(Debug, Clone)]
pub(crate) struct SharedMessageTable {
    indexes: Vec<SharedMessageIndex>,
}

#[derive(Debug, Clone)]
struct SharedMessageIndex {
    index_type: SharedMessageIndexType,
    message_type_flags: u16,
    min_message_size: u32,
    list_cutoff: u16,
    btree_cutoff: u16,
    num_messages: u16,
    index_address: u64,
    fractal_heap_address: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SharedMessageIndexType {
    List,
    BTree,
}

#[derive(Debug, Clone)]
enum SharedMessageRecord {
    Heap {
        hash: u32,
        reference_count: u32,
        heap_id: Vec<u8>,
    },
    ObjectHeader {
        hash: u32,
        message_type: u16,
        object_header_index: u16,
        object_header_address: u64,
    },
}

impl SharedMessageTable {
    /// Parse a SOHM master table from storage.
    pub(crate) fn parse_at_storage(
        storage: &dyn Storage,
        address: u64,
        num_indices: u8,
        offset_size: u8,
    ) -> Result<Self> {
        let entry_len = 1 + 1 + 2 + 4 + 2 + 2 + 2 + 2 + usize::from(offset_size) * 2;
        let table_len = 4 + usize::from(num_indices) * entry_len + 4;
        let bytes = storage.read_range(address, table_len)?;
        let mut cursor = Cursor::new(bytes.as_ref());
        let sig = cursor.read_bytes(4)?;
        if sig != SMTB_SIGNATURE {
            return Err(Error::InvalidData(format!(
                "expected SMTB signature at offset {address:#x}"
            )));
        }

        let mut indexes = Vec::with_capacity(usize::from(num_indices));
        for _ in 0..num_indices {
            let version = cursor.read_u8()?;
            if version != 0 {
                return Err(Error::InvalidData(format!(
                    "unsupported SOHM index version: {version}"
                )));
            }
            let index_type = match cursor.read_u8()? {
                0 => SharedMessageIndexType::List,
                1 => SharedMessageIndexType::BTree,
                other => {
                    return Err(Error::InvalidData(format!(
                        "unsupported SOHM index type: {other}"
                    )))
                }
            };
            let message_type_flags = cursor.read_u16_le()?;
            let min_message_size = cursor.read_u32_le()?;
            let list_cutoff = cursor.read_u16_le()?;
            let btree_cutoff = cursor.read_u16_le()?;
            let num_messages = cursor.read_u16_le()?;
            cursor.skip(2)?;
            let index_address = cursor.read_offset(offset_size)?;
            let fractal_heap_address = cursor.read_offset(offset_size)?;
            indexes.push(SharedMessageIndex {
                index_type,
                message_type_flags,
                min_message_size,
                list_cutoff,
                btree_cutoff,
                num_messages,
                index_address,
                fractal_heap_address,
            });
        }

        let checksum_pos = cursor.position() as usize;
        let stored_checksum = cursor.read_u32_le()?;
        let computed = jenkins_lookup3(&bytes.as_ref()[..checksum_pos]);
        if computed != stored_checksum {
            return Err(Error::ChecksumMismatch {
                expected: stored_checksum,
                actual: computed,
            });
        }

        Ok(Self { indexes })
    }

    /// Resolve a SOHM heap ID into the concrete object-header message.
    pub(crate) fn resolve_heap_message(
        &self,
        heap_id: &[u8],
        message_type: u16,
        storage: &dyn Storage,
        offset_size: u8,
        length_size: u8,
        filter_registry: Option<&FilterRegistry>,
    ) -> Result<Option<HdfMessage>> {
        let preferred_indexes: Vec<&SharedMessageIndex> = self
            .indexes
            .iter()
            .filter(|index| index.tracks_message_type(message_type))
            .collect();

        let indexes: Vec<&SharedMessageIndex> = if preferred_indexes.is_empty() {
            self.indexes.iter().collect()
        } else {
            preferred_indexes
        };

        for index in indexes {
            for record in index.records(storage, offset_size, length_size)? {
                match record {
                    SharedMessageRecord::Heap {
                        hash,
                        reference_count,
                        heap_id: record_heap_id,
                    } => {
                        let _ = (hash, reference_count);
                        if record_heap_id != heap_id {
                            continue;
                        }
                    }
                    SharedMessageRecord::ObjectHeader {
                        hash,
                        message_type,
                        object_header_index,
                        object_header_address,
                    } => {
                        let _ = (
                            hash,
                            message_type,
                            object_header_index,
                            object_header_address,
                        );
                        continue;
                    }
                }

                if Cursor::is_undefined_offset(index.fractal_heap_address, offset_size) {
                    return Err(Error::UndefinedAddress);
                }
                let heap = FractalHeap::parse_at_storage(
                    storage,
                    index.fractal_heap_address,
                    offset_size,
                    length_size,
                )?;
                let payload = heap.get_object_storage_with_registry(
                    heap_id,
                    storage,
                    offset_size,
                    length_size,
                    filter_registry,
                )?;
                let mut cursor = Cursor::new(&payload);
                let message = parse_message(
                    message_type,
                    payload.len(),
                    &mut cursor,
                    offset_size,
                    length_size,
                )?;
                return Ok(Some(message));
            }
        }

        Ok(None)
    }
}

impl SharedMessageIndex {
    fn tracks_message_type(&self, message_type: u16) -> bool {
        let Some(bit) = shared_message_type_bit(message_type) else {
            return false;
        };
        (self.message_type_flags & (1u16 << bit)) != 0
    }

    fn records(
        &self,
        storage: &dyn Storage,
        offset_size: u8,
        length_size: u8,
    ) -> Result<Vec<SharedMessageRecord>> {
        let _ = (self.min_message_size, self.list_cutoff, self.btree_cutoff);
        match self.index_type {
            SharedMessageIndexType::List => {
                parse_record_list(storage, self.index_address, self.num_messages, offset_size)
            }
            SharedMessageIndexType::BTree => {
                let header = btree_v2::BTreeV2Header::parse_at_storage(
                    storage,
                    self.index_address,
                    offset_size,
                    length_size,
                )?;
                let records = btree_v2::collect_btree_v2_records_storage(
                    storage,
                    &header,
                    offset_size,
                    length_size,
                    None,
                    &[],
                    None,
                )?;
                records
                    .into_iter()
                    .filter_map(record_from_btree)
                    .collect::<Result<Vec<_>>>()
            }
        }
    }
}

fn parse_record_list(
    storage: &dyn Storage,
    address: u64,
    num_records: u16,
    offset_size: u8,
) -> Result<Vec<SharedMessageRecord>> {
    if num_records == 0 {
        return Ok(Vec::new());
    }
    let max_record_len = 20usize;
    let max_len = 4 + usize::from(num_records) * max_record_len + 4;
    let bytes = storage.read_range(address, max_len)?;
    let mut cursor = Cursor::new(bytes.as_ref());
    let sig = cursor.read_bytes(4)?;
    if sig != SMLI_SIGNATURE {
        return Err(Error::InvalidData(format!(
            "expected SMLI signature at offset {address:#x}"
        )));
    }

    let mut records = Vec::with_capacity(usize::from(num_records));
    for _ in 0..num_records {
        records.push(parse_record(&mut cursor, offset_size)?);
    }

    let checksum_pos = cursor.position() as usize;
    let stored_checksum = cursor.read_u32_le()?;
    let computed = jenkins_lookup3(&bytes.as_ref()[..checksum_pos]);
    if computed != stored_checksum {
        return Err(Error::ChecksumMismatch {
            expected: stored_checksum,
            actual: computed,
        });
    }

    Ok(records)
}

fn parse_record(cursor: &mut Cursor<'_>, offset_size: u8) -> Result<SharedMessageRecord> {
    let location = cursor.read_u8()?;
    cursor.skip(3)?;
    let hash = cursor.read_u32_le()?;
    match location {
        0 => {
            let reference_count = cursor.read_u32_le()?;
            let heap_id = cursor.read_bytes(8)?.to_vec();
            Ok(SharedMessageRecord::Heap {
                hash,
                reference_count,
                heap_id,
            })
        }
        1 => {
            let _reserved = cursor.read_u8()?;
            let message_type = u16::from(cursor.read_u8()?);
            let object_header_index = cursor.read_u16_le()?;
            let object_header_address = cursor.read_offset(offset_size)?;
            Ok(SharedMessageRecord::ObjectHeader {
                hash,
                message_type,
                object_header_index,
                object_header_address,
            })
        }
        other => Err(Error::InvalidData(format!(
            "unknown SOHM record location: {other}"
        ))),
    }
}

fn record_from_btree(record: BTreeV2Record) -> Option<Result<SharedMessageRecord>> {
    match record {
        BTreeV2Record::SharedMessageHeap {
            hash,
            reference_count,
            heap_id,
        } => Some(Ok(SharedMessageRecord::Heap {
            hash,
            reference_count,
            heap_id,
        })),
        BTreeV2Record::SharedMessageObjectHeader {
            hash,
            message_type,
            object_header_index,
            object_header_address,
        } => Some(Ok(SharedMessageRecord::ObjectHeader {
            hash,
            message_type,
            object_header_index,
            object_header_address,
        })),
        _ => None,
    }
}

fn shared_message_type_bit(message_type: u16) -> Option<u8> {
    match message_type {
        crate::messages::MSG_DATASPACE => Some(0),
        crate::messages::MSG_DATATYPE => Some(1),
        crate::messages::MSG_FILL_VALUE | crate::messages::MSG_FILL_VALUE_OLD => Some(2),
        crate::messages::MSG_FILTER_PIPELINE => Some(3),
        crate::messages::MSG_ATTRIBUTE => Some(4),
        _ => None,
    }
}

pub(crate) type SharedMessageTableRef = Arc<SharedMessageTable>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::BytesStorage;

    #[test]
    fn parses_master_table() {
        let mut table = Vec::new();
        table.extend_from_slice(b"SMTB");
        table.push(0);
        table.push(0);
        table.extend_from_slice(&0x0002u16.to_le_bytes());
        table.extend_from_slice(&16u32.to_le_bytes());
        table.extend_from_slice(&8u16.to_le_bytes());
        table.extend_from_slice(&6u16.to_le_bytes());
        table.extend_from_slice(&1u16.to_le_bytes());
        table.extend_from_slice(&[0, 0]);
        table.extend_from_slice(&64u64.to_le_bytes());
        table.extend_from_slice(&128u64.to_le_bytes());
        let checksum = jenkins_lookup3(&table);
        table.extend_from_slice(&checksum.to_le_bytes());

        let storage = BytesStorage::new(table);
        let parsed = SharedMessageTable::parse_at_storage(&storage, 0, 1, 8).unwrap();
        assert_eq!(parsed.indexes.len(), 1);
        assert!(parsed.indexes[0].tracks_message_type(crate::messages::MSG_DATATYPE));
        assert_eq!(parsed.indexes[0].num_messages, 1);
        assert_eq!(parsed.indexes[0].index_address, 64);
        assert_eq!(parsed.indexes[0].fractal_heap_address, 128);
    }

    #[test]
    fn parses_record_list_heap_record() {
        let mut data = vec![0u8; 32];
        let mut list = Vec::new();
        list.extend_from_slice(b"SMLI");
        list.push(0);
        list.extend_from_slice(&[0, 0, 0]);
        list.extend_from_slice(&0x1122_3344u32.to_le_bytes());
        list.extend_from_slice(&2u32.to_le_bytes());
        list.extend_from_slice(&[8, 7, 6, 5, 4, 3, 2, 1]);
        let checksum = jenkins_lookup3(&list);
        list.extend_from_slice(&checksum.to_le_bytes());
        data.extend_from_slice(&list);

        let storage = BytesStorage::new(data);
        let records = parse_record_list(&storage, 32, 1, 8).unwrap();
        match &records[0] {
            SharedMessageRecord::Heap {
                hash,
                reference_count,
                heap_id,
            } => {
                assert_eq!(*hash, 0x1122_3344);
                assert_eq!(*reference_count, 2);
                assert_eq!(heap_id, &[8, 7, 6, 5, 4, 3, 2, 1]);
            }
            other => panic!("expected heap record, got {:?}", other),
        }
    }
}
