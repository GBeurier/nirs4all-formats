//! HDF5 B-tree Version 2.
//!
//! V2 B-trees are used by newer-style groups and datasets for indexed link
//! storage, attribute storage, and chunked dataset indexing. The header
//! (`BTHD`) describes the tree parameters. Internal nodes (`BTIN`) and leaf
//! nodes (`BTLF`) contain the actual records.
//!
//! This module provides the header parse, record types, and a traversal
//! function that collects all records from a tree.

use std::collections::HashSet;

use crate::checksum::jenkins_lookup3;
use crate::error::{Error, Result};
use crate::io::Cursor;
use crate::storage::Storage;

// ---------------------------------------------------------------------------
// Signatures
// ---------------------------------------------------------------------------

const BTHD_SIGNATURE: [u8; 4] = *b"BTHD";
const BTIN_SIGNATURE: [u8; 4] = *b"BTIN";
const BTLF_SIGNATURE: [u8; 4] = *b"BTLF";
const MAX_BTREE_V2_DEPTH: u16 = 64;

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

/// Parsed B-tree v2 header.
#[derive(Debug, Clone)]
pub struct BTreeV2Header {
    /// B-tree type (determines the record format).
    pub btree_type: u8,
    /// Size in bytes of each B-tree node (both internal and leaf).
    pub node_size: u32,
    /// Size in bytes of each record.
    pub record_size: u16,
    /// Depth of the tree (0 = root is a leaf).
    pub depth: u16,
    /// Percent full at which to split a node.
    pub split_percent: u8,
    /// Percent full at which to merge a node.
    pub merge_percent: u8,
    /// Address of the root node.
    pub root_node_address: u64,
    /// Number of records in the root node.
    pub num_records_in_root: u16,
    /// Total number of records in the entire tree.
    pub total_records: u64,
}

impl BTreeV2Header {
    /// Parse a B-tree v2 header at the current cursor position.
    ///
    /// Format:
    /// - Signature: `BTHD` (4 bytes)
    /// - Version: 0 (1 byte)
    /// - B-tree type (u8)
    /// - Node size (u32 LE)
    /// - Record size (u16 LE)
    /// - Depth (u16 LE)
    /// - Split percent (u8)
    /// - Merge percent (u8)
    /// - Root node address (`offset_size` bytes)
    /// - Number of records in root node (u16 LE)
    /// - Total number of records in tree (`length_size` bytes)
    /// - Checksum (u32 LE)
    pub fn parse(cursor: &mut Cursor, offset_size: u8, length_size: u8) -> Result<Self> {
        let start = cursor.position();

        let sig = cursor.read_bytes(4)?;
        if sig != BTHD_SIGNATURE {
            return Err(Error::InvalidBTreeV2Signature { context: "header" });
        }

        let version = cursor.read_u8()?;
        if version != 0 {
            return Err(Error::UnsupportedBTreeVersion(version));
        }

        let btree_type = cursor.read_u8()?;
        let node_size = cursor.read_u32_le()?;
        let record_size = cursor.read_u16_le()?;
        let depth = cursor.read_u16_le()?;
        let split_percent = cursor.read_u8()?;
        let merge_percent = cursor.read_u8()?;
        let root_node_address = cursor.read_offset(offset_size)?;
        let num_records_in_root = cursor.read_u16_le()?;
        let total_records = cursor.read_length(length_size)?;

        // Checksum covers everything from signature through total_records.
        let checksum_end = cursor.position();
        let stored_checksum = cursor.read_u32_le()?;

        let computed = jenkins_lookup3(&cursor.data()[start as usize..checksum_end as usize]);
        if computed != stored_checksum {
            return Err(Error::ChecksumMismatch {
                expected: stored_checksum,
                actual: computed,
            });
        }

        Ok(BTreeV2Header {
            btree_type,
            node_size,
            record_size,
            depth,
            split_percent,
            merge_percent,
            root_node_address,
            num_records_in_root,
            total_records,
        })
    }

    /// Parse a B-tree v2 header from random-access storage.
    pub fn parse_at_storage(
        storage: &dyn Storage,
        address: u64,
        offset_size: u8,
        length_size: u8,
    ) -> Result<Self> {
        let header_len = 4
            + 1
            + 1
            + 4
            + 2
            + 2
            + 1
            + 1
            + usize::from(offset_size)
            + 2
            + usize::from(length_size)
            + 4;
        let bytes = storage.read_range(address, header_len)?;
        let mut cursor = Cursor::new(bytes.as_ref());
        Self::parse(&mut cursor, offset_size, length_size)
    }
}

// ---------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------

/// A record from a B-tree v2.
///
/// The record format depends on the B-tree type field in the header.
#[derive(Debug, Clone)]
pub enum BTreeV2Record {
    /// Type 1: indirectly accessed, non-filtered huge fractal heap object.
    HugeIndirectNonFiltered {
        address: u64,
        length: u64,
        object_id: u64,
    },
    /// Type 2: indirectly accessed, filtered huge fractal heap object.
    HugeIndirectFiltered {
        address: u64,
        filtered_length: u64,
        filter_mask: u32,
        memory_length: u64,
        object_id: u64,
    },
    /// Type 3: directly accessed, non-filtered huge fractal heap object.
    HugeDirectNonFiltered { address: u64, length: u64 },
    /// Type 4: directly accessed, filtered huge fractal heap object.
    HugeDirectFiltered {
        address: u64,
        filtered_length: u64,
        filter_mask: u32,
        memory_length: u64,
    },
    /// Type 5: Link name for indexed group (hashed).
    LinkNameHash { hash: u32, heap_id: Vec<u8> },
    /// Type 6: Creation order for indexed group.
    CreationOrder { order: u64, heap_id: Vec<u8> },
    /// Type 8: Attribute name for indexed group (hashed).
    AttributeNameHash {
        hash: u32,
        flags: u8,
        creation_order: u32,
        heap_id: Vec<u8>,
    },
    /// Type 9: Attribute creation order.
    AttributeCreationOrder { order: u32, heap_id: Vec<u8> },
    /// Type 10: Non-filtered chunked dataset record (v2 chunk index).
    ChunkedNonFiltered { address: u64, offsets: Vec<u64> },
    /// Type 11: Filtered chunked dataset record (v2 chunk index).
    ChunkedFiltered {
        address: u64,
        chunk_size: u64,
        filter_mask: u32,
        offsets: Vec<u64>,
    },
    /// Type 7: shared object-header message stored in the SOHM heap.
    SharedMessageHeap {
        hash: u32,
        reference_count: u32,
        heap_id: Vec<u8>,
    },
    /// Type 7: shared object-header message stored in an object header.
    SharedMessageObjectHeader {
        hash: u32,
        message_type: u16,
        object_header_index: u16,
        object_header_address: u64,
    },
    /// Unknown/unsupported record type — raw bytes preserved.
    Unknown { record_type: u8, data: Vec<u8> },
}

// ---------------------------------------------------------------------------
// Record parsing
// ---------------------------------------------------------------------------

/// Parse a single record of the given B-tree type.
#[allow(clippy::too_many_arguments)]
fn parse_record(
    cursor: &mut Cursor,
    btree_type: u8,
    record_size: u16,
    offset_size: u8,
    length_size: u8,
    ndims: Option<u32>,
    chunk_dims: &[u32],
    heap_id_len: usize,
) -> Result<BTreeV2Record> {
    let record_start = cursor.position();

    let record = match btree_type {
        // Type 1: indirectly accessed, non-filtered huge fractal heap object.
        1 => BTreeV2Record::HugeIndirectNonFiltered {
            address: cursor.read_offset(offset_size)?,
            length: cursor.read_length(length_size)?,
            object_id: cursor.read_length(length_size)?,
        },

        // Type 2: indirectly accessed, filtered huge fractal heap object.
        2 => BTreeV2Record::HugeIndirectFiltered {
            address: cursor.read_offset(offset_size)?,
            filtered_length: cursor.read_length(length_size)?,
            filter_mask: cursor.read_u32_le()?,
            memory_length: cursor.read_length(length_size)?,
            object_id: cursor.read_length(length_size)?,
        },

        // Type 3: directly accessed, non-filtered huge fractal heap object.
        3 => BTreeV2Record::HugeDirectNonFiltered {
            address: cursor.read_offset(offset_size)?,
            length: cursor.read_length(length_size)?,
        },

        // Type 4: directly accessed, filtered huge fractal heap object.
        4 => BTreeV2Record::HugeDirectFiltered {
            address: cursor.read_offset(offset_size)?,
            filtered_length: cursor.read_length(length_size)?,
            filter_mask: cursor.read_u32_le()?,
            memory_length: cursor.read_length(length_size)?,
        },

        // Type 5: link name hash
        5 => {
            let hash = cursor.read_u32_le()?;
            let heap_id = cursor.read_bytes(heap_id_len)?.to_vec();
            BTreeV2Record::LinkNameHash { hash, heap_id }
        }

        // Type 6: creation order
        6 => {
            let order = cursor.read_u64_le()?;
            let heap_id = cursor.read_bytes(heap_id_len)?.to_vec();
            BTreeV2Record::CreationOrder { order, heap_id }
        }

        // Type 7: shared object-header messages.
        7 => {
            let location = cursor.read_u8()?;
            cursor.skip(3)?;
            let hash = cursor.read_u32_le()?;
            match location {
                0 => {
                    let reference_count = cursor.read_u32_le()?;
                    let heap_id = cursor.read_bytes(8)?.to_vec();
                    BTreeV2Record::SharedMessageHeap {
                        hash,
                        reference_count,
                        heap_id,
                    }
                }
                1 => {
                    let _reserved = cursor.read_u8()?;
                    let message_type = u16::from(cursor.read_u8()?);
                    let object_header_index = cursor.read_u16_le()?;
                    let object_header_address = cursor.read_offset(offset_size)?;
                    BTreeV2Record::SharedMessageObjectHeader {
                        hash,
                        message_type,
                        object_header_index,
                        object_header_address,
                    }
                }
                other => {
                    return Err(Error::InvalidData(format!(
                        "unknown SOHM B-tree record location: {other}"
                    )));
                }
            }
        }

        // Type 8: attribute name hash
        8 => {
            let heap_id = cursor.read_bytes(heap_id_len)?.to_vec();
            let flags = cursor.read_u8()?;
            let creation_order = cursor.read_u32_le()?;
            let hash = cursor.read_u32_le()?;
            BTreeV2Record::AttributeNameHash {
                hash,
                flags,
                creation_order,
                heap_id,
            }
        }

        // Type 9: attribute creation order
        9 => {
            let heap_id = cursor.read_bytes(heap_id_len)?.to_vec();
            let _flags = cursor.read_u8()?;
            let order = cursor.read_u32_le()?;
            BTreeV2Record::AttributeCreationOrder { order, heap_id }
        }

        // Type 10: non-filtered chunk
        10 => {
            // Chunk offsets are encoded as scaled 64-bit values.
            // The number of offset dimensions is calculated from the record size.
            // Each offset is 8 bytes in a type-10 record.
            let offset_bytes_available = record_payload_len(
                record_size,
                offset_size as usize,
                "B-tree v2 type-10 chunk record is shorter than its address",
            )?;
            let address = cursor.read_offset(offset_size)?;
            let num_offsets = offset_bytes_available / 8;
            let offsets = read_scaled_chunk_offsets(cursor, num_offsets, ndims, chunk_dims)?;
            BTreeV2Record::ChunkedNonFiltered { address, offsets }
        }

        // Type 11: filtered chunk. The on-disk chunk-size field width is not
        // the file's global length size — libhdf5 sizes it to the nominal
        // chunk byte count. We recover that width from the record size:
        //   record_size = address + chunk_size_field + filter_mask(4) + offsets
        // where the number of scaled 8-byte offsets equals the dimensionality.
        11 => {
            let ndims_val = ndims
                .map(|n| n as usize)
                .or_else(|| (!chunk_dims.is_empty()).then_some(chunk_dims.len()))
                .ok_or_else(|| {
                    Error::InvalidData(
                        "B-tree v2 type-11 chunk record needs a known dimensionality".to_string(),
                    )
                })?;
            let offsets_bytes = ndims_val
                .checked_mul(8)
                .ok_or_else(|| Error::InvalidData("chunk offset bytes overflow".to_string()))?;
            let fixed_size = offset_size as usize + 4 + offsets_bytes;
            let chunk_size_len =
                (record_size as usize)
                    .checked_sub(fixed_size)
                    .ok_or_else(|| {
                        Error::InvalidData(
                            "B-tree v2 type-11 chunk record is shorter than its fixed fields"
                                .to_string(),
                        )
                    })?;
            if chunk_size_len == 0 || chunk_size_len > 8 {
                return Err(Error::InvalidData(format!(
                    "B-tree v2 type-11 chunk-size field width {chunk_size_len} is out of range"
                )));
            }
            let address = cursor.read_offset(offset_size)?;
            // The chunk-size field is 1–8 bytes wide (sized to the chunk), not a "length".
            let chunk_size = cursor.read_uvar(chunk_size_len)?;
            let filter_mask = cursor.read_u32_le()?;
            let offsets = read_scaled_chunk_offsets(cursor, ndims_val, ndims, chunk_dims)?;
            BTreeV2Record::ChunkedFiltered {
                address,
                chunk_size,
                filter_mask,
                offsets,
            }
        }

        // Unknown type — read raw bytes.
        _ => {
            let data = cursor.read_bytes(record_size as usize)?.to_vec();
            return Ok(BTreeV2Record::Unknown {
                record_type: btree_type,
                data,
            });
        }
    };

    // Ensure we consumed no more than record_size bytes, then skip padding.
    let consumed = cursor.position() - record_start;
    let record_size = u64::from(record_size);
    if consumed > record_size {
        return Err(Error::InvalidData(format!(
            "B-tree v2 type-{btree_type} record consumed {consumed} bytes but record size is {record_size}"
        )));
    }
    if consumed < record_size {
        cursor.skip((record_size - consumed) as usize)?;
    }

    Ok(record)
}

fn record_payload_len(record_size: u16, fixed_size: usize, error_message: &str) -> Result<usize> {
    (record_size as usize)
        .checked_sub(fixed_size)
        .ok_or_else(|| Error::InvalidData(error_message.into()))
}

fn read_scaled_chunk_offsets(
    cursor: &mut Cursor,
    num_offsets: usize,
    ndims: Option<u32>,
    chunk_dims: &[u32],
) -> Result<Vec<u64>> {
    if let Some(ndims) = ndims {
        let ndims = usize::try_from(ndims)
            .map_err(|_| Error::InvalidData("B-tree v2 chunk rank exceeds usize".into()))?;
        if num_offsets != ndims {
            return Err(Error::InvalidData(format!(
                "B-tree v2 chunk record has {num_offsets} offsets for {ndims} dimensions"
            )));
        }
    }

    if !chunk_dims.is_empty() && num_offsets != chunk_dims.len() {
        return Err(Error::InvalidData(format!(
            "B-tree v2 chunk record has {num_offsets} offsets but {} chunk dimensions",
            chunk_dims.len()
        )));
    }

    let mut offsets = Vec::with_capacity(num_offsets);
    for dim in 0..num_offsets {
        let scaled = cursor.read_u64_le()?;
        let chunk_extent = chunk_dims.get(dim).copied().unwrap_or(1);
        let offset = scaled
            .checked_mul(u64::from(chunk_extent))
            .ok_or_else(|| Error::InvalidData("B-tree v2 chunk offset overflows u64".into()))?;
        offsets.push(offset);
    }
    Ok(offsets)
}

fn record_matches_chunk_bounds(
    record: &BTreeV2Record,
    chunk_dims: &[u32],
    chunk_bounds: Option<(&[u64], &[u64])>,
) -> bool {
    let Some((first_chunk, last_chunk)) = chunk_bounds else {
        return true;
    };

    let offsets = match record {
        BTreeV2Record::ChunkedNonFiltered { offsets, .. }
        | BTreeV2Record::ChunkedFiltered { offsets, .. } => offsets,
        _ => return true,
    };

    offsets.iter().enumerate().all(|(dim, offset)| {
        let chunk_index = *offset / u64::from(chunk_dims[dim]);
        chunk_index >= first_chunk[dim] && chunk_index <= last_chunk[dim]
    })
}

fn record_matches_link_name_hash(record: &BTreeV2Record, target_hash: Option<u32>) -> bool {
    match target_hash {
        Some(target_hash) => {
            matches!(record, BTreeV2Record::LinkNameHash { hash, .. } if *hash == target_hash)
        }
        None => true,
    }
}

fn record_matches_query(
    record: &BTreeV2Record,
    chunk_dims: &[u32],
    chunk_bounds: Option<(&[u64], &[u64])>,
    link_name_hash: Option<u32>,
) -> bool {
    record_matches_link_name_hash(record, link_name_hash)
        && record_matches_chunk_bounds(record, chunk_dims, chunk_bounds)
}

fn link_name_hash(record: &BTreeV2Record) -> Option<u32> {
    match record {
        BTreeV2Record::LinkNameHash { hash, .. } => Some(*hash),
        _ => None,
    }
}

fn child_may_match_link_name_hash(
    records: &[BTreeV2Record],
    child_index: usize,
    target_hash: Option<u32>,
) -> bool {
    let Some(target_hash) = target_hash else {
        return true;
    };

    let lower_matches = child_index == 0
        || link_name_hash(&records[child_index - 1]).map_or(true, |hash| target_hash >= hash);
    let upper_matches = child_index == records.len()
        || link_name_hash(&records[child_index]).map_or(true, |hash| target_hash <= hash);
    lower_matches && upper_matches
}

fn validate_btree_v2_depth(depth: u16) -> Result<()> {
    if depth > MAX_BTREE_V2_DEPTH {
        return Err(Error::InvalidData(format!(
            "B-tree v2 depth {depth} exceeds traversal limit {MAX_BTREE_V2_DEPTH}"
        )));
    }
    Ok(())
}

fn enter_btree_v2_node(visited: &mut HashSet<u64>, address: u64) -> Result<()> {
    if !visited.insert(address) {
        return Err(Error::InvalidData(format!(
            "B-tree v2 traversal revisits node at offset {address:#x}"
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Node parsing
// ---------------------------------------------------------------------------

/// Compute the number of bytes needed to represent `max_records` as an
/// unsigned integer (used for child-node record counts in internal nodes).
fn num_records_size(max_records: u64) -> usize {
    if max_records <= 0xFF {
        1
    } else if max_records <= 0xFFFF {
        2
    } else if max_records <= 0xFFFF_FFFF {
        4
    } else {
        8
    }
}

/// Compute the maximum number of records that fit in a leaf node.
fn max_leaf_records(node_size: u32, record_size: u16) -> u64 {
    // Leaf node overhead: signature(4) + version(1) + type(1) + checksum(4) = 10
    let overhead = 10u32;
    if node_size <= overhead || record_size == 0 {
        return 0;
    }
    ((node_size - overhead) / record_size as u32) as u64
}

/// Compute the maximum number of records that fit in an internal node.
/// This depends on the pointer size (offset_size) and the number-of-records
/// encoding for child nodes, which makes it recursive in principle. We use
/// an iterative approach.
fn max_internal_records(
    node_size: u32,
    record_size: u16,
    offset_size: u8,
    max_child_records: u64,
) -> u64 {
    // Internal node overhead: signature(4) + version(1) + type(1) + checksum(4) = 10
    let overhead = 10u32;
    if node_size <= overhead || record_size == 0 {
        return 0;
    }
    let available = (node_size - overhead) as u64;
    // Each entry in an internal node is: record(record_size) + child_pointer(offset_size) + num_records(var)
    let nrec_size = num_records_size(max_child_records) as u64;
    let entry_size = record_size as u64 + offset_size as u64 + nrec_size;
    // There is one more child pointer + num_records than records.
    // So: n * record_size + (n+1) * (offset_size + nrec_size) <= available
    // => n * (record_size + offset_size + nrec_size) + offset_size + nrec_size <= available
    let extra = offset_size as u64 + nrec_size;
    if available <= extra {
        return 0;
    }
    (available - extra) / entry_size
}

fn node_checksum_bounds(
    cursor: &Cursor<'_>,
    start: u64,
    node_size: u32,
    compact_end: u64,
    context: &'static str,
) -> Result<usize> {
    let start = usize::try_from(start)
        .map_err(|_| Error::InvalidData(format!("B-tree v2 {context} offset is too large")))?;
    let node_size = usize::try_from(node_size)
        .map_err(|_| Error::InvalidData(format!("B-tree v2 {context} size is too large")))?;
    if node_size < 10 {
        return Err(Error::InvalidData(format!(
            "B-tree v2 {context} node is too small: {node_size} bytes"
        )));
    }

    let checksum_pos = usize::try_from(compact_end)
        .map_err(|_| Error::InvalidData(format!("B-tree v2 {context} checksum is too large")))?;
    if checksum_pos < start {
        return Err(Error::InvalidData(format!(
            "B-tree v2 {context} payload starts before node"
        )));
    }
    let node_end = start
        .checked_add(node_size)
        .ok_or_else(|| Error::InvalidData(format!("B-tree v2 {context} size overflow")))?;

    if node_end > cursor.data().len() {
        return Err(Error::UnexpectedEof {
            offset: start as u64,
            needed: node_size as u64,
            available: cursor.data().len().saturating_sub(start) as u64,
        });
    }
    let checksum_end = checksum_pos
        .checked_add(4)
        .ok_or_else(|| Error::InvalidData(format!("B-tree v2 {context} checksum overflow")))?;
    if checksum_end > node_end {
        return Err(Error::InvalidData(format!(
            "B-tree v2 {context} contents exceed node size"
        )));
    }

    Ok(checksum_pos)
}

fn verify_node_checksum(
    cursor: &mut Cursor<'_>,
    start: u64,
    node_size: u32,
    compact_end: u64,
    context: &'static str,
) -> Result<()> {
    let checksum_pos = node_checksum_bounds(cursor, start, node_size, compact_end, context)?;
    cursor.set_position(checksum_pos as u64);
    let stored_checksum = cursor.read_u32_le()?;
    let computed = jenkins_lookup3(&cursor.data()[start as usize..checksum_pos]);
    if computed != stored_checksum {
        return Err(Error::ChecksumMismatch {
            expected: stored_checksum,
            actual: computed,
        });
    }
    Ok(())
}

/// Parse a leaf node and collect its records.
#[allow(clippy::too_many_arguments)]
fn parse_leaf_node(
    cursor: &mut Cursor,
    header: &BTreeV2Header,
    offset_size: u8,
    length_size: u8,
    ndims: Option<u32>,
    chunk_dims: &[u32],
    chunk_bounds: Option<(&[u64], &[u64])>,
    link_name_hash: Option<u32>,
    num_records: u16,
    heap_id_len: usize,
    records: &mut Vec<BTreeV2Record>,
) -> Result<()> {
    let start = cursor.position();

    let sig = cursor.read_bytes(4)?;
    if sig != BTLF_SIGNATURE {
        return Err(Error::InvalidBTreeV2Signature {
            context: "leaf node",
        });
    }

    let version = cursor.read_u8()?;
    if version != 0 {
        return Err(Error::UnsupportedBTreeVersion(version));
    }

    let node_type = cursor.read_u8()?;
    if node_type != header.btree_type {
        return Err(Error::InvalidData(format!(
            "B-tree v2 leaf node type mismatch: header says {}, node says {}",
            header.btree_type, node_type
        )));
    }

    for _ in 0..num_records {
        let record = parse_record(
            cursor,
            header.btree_type,
            header.record_size,
            offset_size,
            length_size,
            ndims,
            chunk_dims,
            heap_id_len,
        )?;
        if record_matches_query(&record, chunk_dims, chunk_bounds, link_name_hash) {
            records.push(record);
        }
    }

    verify_node_checksum(
        cursor,
        start,
        header.node_size,
        cursor.position(),
        "leaf node",
    )?;

    Ok(())
}

/// Parse an internal node, collecting child addresses and recursing.
#[allow(clippy::too_many_arguments)]
fn parse_internal_node(
    data: &[u8],
    address: u64,
    header: &BTreeV2Header,
    offset_size: u8,
    length_size: u8,
    ndims: Option<u32>,
    chunk_dims: &[u32],
    chunk_bounds: Option<(&[u64], &[u64])>,
    link_name_hash: Option<u32>,
    num_records: u16,
    depth: u16,
    heap_id_len: usize,
    visited: &mut HashSet<u64>,
    records: &mut Vec<BTreeV2Record>,
) -> Result<()> {
    if depth == 0 {
        return Err(Error::InvalidData(
            "B-tree v2 internal node traversal reached depth zero".into(),
        ));
    }
    enter_btree_v2_node(visited, address)?;

    let mut cursor = Cursor::new(data);
    cursor.set_position(address);

    let start = cursor.position();

    let sig = cursor.read_bytes(4)?;
    if sig != BTIN_SIGNATURE {
        return Err(Error::InvalidBTreeV2Signature {
            context: "internal node",
        });
    }

    let version = cursor.read_u8()?;
    if version != 0 {
        return Err(Error::UnsupportedBTreeVersion(version));
    }

    let node_type = cursor.read_u8()?;
    if node_type != header.btree_type {
        return Err(Error::InvalidData(format!(
            "B-tree v2 internal node type mismatch: header says {}, node says {}",
            header.btree_type, node_type
        )));
    }

    // Compute max records for children to know the encoding size for
    // child record counts.
    let max_child_records = if depth == 1 {
        max_leaf_records(header.node_size, header.record_size)
    } else {
        // For deeper trees, compute iteratively.
        let leaf_max = max_leaf_records(header.node_size, header.record_size);
        let mut prev_max = leaf_max;
        for _ in 1..depth {
            prev_max =
                max_internal_records(header.node_size, header.record_size, offset_size, prev_max);
        }
        prev_max
    };
    let nrec_bytes = num_records_size(max_child_records);

    // Read records and child pointers interleaved:
    // child[0], record[0], child[1], record[1], ..., record[n-1], child[n]
    // Plus total_records counts for each child.
    //
    // Actually per the HDF5 spec the layout is:
    // record[0], record[1], ..., record[n-1],
    // child_ptr[0], nrec[0], total[0], child_ptr[1], nrec[1], total[1], ..., child_ptr[n], nrec[n], total[n]
    //
    // Records first, then child pointers with their metadata.

    // Read all records.
    let mut node_records = Vec::with_capacity(num_records as usize);
    for _ in 0..num_records {
        let record = parse_record(
            &mut cursor,
            header.btree_type,
            header.record_size,
            offset_size,
            length_size,
            ndims,
            chunk_dims,
            heap_id_len,
        )?;
        node_records.push(record);
    }

    // Read child node pointers (num_records + 1 of them).
    let num_children = num_records as usize + 1;

    // Whether to include a "total records" field for each child.
    // This is present when depth > 1.
    let has_total_records = depth > 1;
    // total_records encoding size for deeper nodes
    let total_nrec_bytes = if has_total_records {
        // Total records in a sub-tree — need enough bytes to hold the
        // maximum total records. We use length_size as an upper bound.
        length_size as usize
    } else {
        0
    };

    let mut child_addresses = Vec::with_capacity(num_children);
    let mut child_nrecords = Vec::with_capacity(num_children);

    for _ in 0..num_children {
        let child_addr = cursor.read_offset(offset_size)?;
        child_addresses.push(child_addr);
        let nrec = cursor.read_uvar(nrec_bytes)?;
        child_nrecords.push(nrec as u16);
        if has_total_records {
            // Skip total records count
            cursor.read_uvar(total_nrec_bytes)?;
        }
    }

    let compact_end = cursor.position();
    verify_node_checksum(
        &mut cursor,
        start,
        header.node_size,
        compact_end,
        "internal node",
    )?;

    // HDF5's v2 B-tree iterator visits child[i], then record[i], then the
    // final child. Internal-node records are real records, not only separators.
    let child_depth = depth - 1;
    for (i, record) in node_records.iter().enumerate() {
        let child_addr = child_addresses[i];
        if !Cursor::is_undefined_offset(child_addr, offset_size)
            && child_may_match_link_name_hash(&node_records, i, link_name_hash)
        {
            let child_nrec = child_nrecords[i];
            if child_depth == 0 {
                enter_btree_v2_node(visited, child_addr)?;
                let mut child_cursor = Cursor::new(data);
                child_cursor.set_position(child_addr);
                parse_leaf_node(
                    &mut child_cursor,
                    header,
                    offset_size,
                    length_size,
                    ndims,
                    chunk_dims,
                    chunk_bounds,
                    link_name_hash,
                    child_nrec,
                    heap_id_len,
                    records,
                )?;
            } else {
                parse_internal_node(
                    data,
                    child_addr,
                    header,
                    offset_size,
                    length_size,
                    ndims,
                    chunk_dims,
                    chunk_bounds,
                    link_name_hash,
                    child_nrec,
                    child_depth,
                    heap_id_len,
                    visited,
                    records,
                )?;
            }
        }

        if record_matches_query(record, chunk_dims, chunk_bounds, link_name_hash) {
            records.push(record.clone());
        }
    }

    let final_child_index = node_records.len();
    let final_child_addr = child_addresses[final_child_index];
    if !Cursor::is_undefined_offset(final_child_addr, offset_size)
        && child_may_match_link_name_hash(&node_records, final_child_index, link_name_hash)
    {
        let child_nrec = child_nrecords[final_child_index];
        if child_depth == 0 {
            enter_btree_v2_node(visited, final_child_addr)?;
            let mut child_cursor = Cursor::new(data);
            child_cursor.set_position(final_child_addr);
            parse_leaf_node(
                &mut child_cursor,
                header,
                offset_size,
                length_size,
                ndims,
                chunk_dims,
                chunk_bounds,
                link_name_hash,
                child_nrec,
                heap_id_len,
                records,
            )?;
        } else {
            parse_internal_node(
                data,
                final_child_addr,
                header,
                offset_size,
                length_size,
                ndims,
                chunk_dims,
                chunk_bounds,
                link_name_hash,
                child_nrec,
                child_depth,
                heap_id_len,
                visited,
                records,
            )?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Public traversal
// ---------------------------------------------------------------------------

/// Collect all records from a B-tree v2 by traversing from the root.
///
/// `header` must be a previously parsed `BTreeV2Header`. `data` is the full
/// file buffer. `ndims` is needed for chunk index record types (10 and 11).
pub fn collect_btree_v2_records(
    data: &[u8],
    header: &BTreeV2Header,
    offset_size: u8,
    length_size: u8,
    ndims: Option<u32>,
    chunk_dims: &[u32],
    chunk_bounds: Option<(&[u64], &[u64])>,
) -> Result<Vec<BTreeV2Record>> {
    collect_btree_v2_records_with_link_hash(
        data,
        header,
        offset_size,
        length_size,
        ndims,
        chunk_dims,
        chunk_bounds,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_btree_v2_records_with_link_hash(
    data: &[u8],
    header: &BTreeV2Header,
    offset_size: u8,
    length_size: u8,
    ndims: Option<u32>,
    chunk_dims: &[u32],
    chunk_bounds: Option<(&[u64], &[u64])>,
    link_name_hash: Option<u32>,
) -> Result<Vec<BTreeV2Record>> {
    if Cursor::is_undefined_offset(header.root_node_address, offset_size) {
        return Ok(Vec::new());
    }

    if header.total_records == 0 || header.num_records_in_root == 0 {
        return Ok(Vec::new());
    }
    validate_btree_v2_depth(header.depth)?;

    // Determine heap_id_len from the record_size and btree_type.
    let heap_id_len = compute_heap_id_len(header);

    let mut records = Vec::new();
    let mut visited = HashSet::new();

    if header.depth == 0 {
        // Root is a leaf node.
        enter_btree_v2_node(&mut visited, header.root_node_address)?;
        let mut cursor = Cursor::new(data);
        cursor.set_position(header.root_node_address);
        parse_leaf_node(
            &mut cursor,
            header,
            offset_size,
            length_size,
            ndims,
            chunk_dims,
            chunk_bounds,
            link_name_hash,
            header.num_records_in_root,
            heap_id_len,
            &mut records,
        )?;
    } else {
        // Root is an internal node.
        parse_internal_node(
            data,
            header.root_node_address,
            header,
            offset_size,
            length_size,
            ndims,
            chunk_dims,
            chunk_bounds,
            link_name_hash,
            header.num_records_in_root,
            header.depth,
            heap_id_len,
            &mut visited,
            &mut records,
        )?;
    }

    Ok(records)
}

/// Collect link-name records whose stored hash matches `target_hash`.
#[cfg(test)]
pub(crate) fn collect_btree_v2_link_name_hash_records(
    data: &[u8],
    header: &BTreeV2Header,
    offset_size: u8,
    length_size: u8,
    target_hash: u32,
) -> Result<Vec<BTreeV2Record>> {
    collect_btree_v2_records_with_link_hash(
        data,
        header,
        offset_size,
        length_size,
        None,
        &[],
        None,
        Some(target_hash),
    )
}

/// Collect all records from a B-tree v2 using random-access storage.
pub fn collect_btree_v2_records_storage(
    storage: &dyn Storage,
    header: &BTreeV2Header,
    offset_size: u8,
    length_size: u8,
    ndims: Option<u32>,
    chunk_dims: &[u32],
    chunk_bounds: Option<(&[u64], &[u64])>,
) -> Result<Vec<BTreeV2Record>> {
    collect_btree_v2_records_storage_with_link_hash(
        storage,
        header,
        offset_size,
        length_size,
        ndims,
        chunk_dims,
        chunk_bounds,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_btree_v2_records_storage_with_link_hash(
    storage: &dyn Storage,
    header: &BTreeV2Header,
    offset_size: u8,
    length_size: u8,
    ndims: Option<u32>,
    chunk_dims: &[u32],
    chunk_bounds: Option<(&[u64], &[u64])>,
    link_name_hash: Option<u32>,
) -> Result<Vec<BTreeV2Record>> {
    if Cursor::is_undefined_offset(header.root_node_address, offset_size) {
        return Ok(Vec::new());
    }

    if header.total_records == 0 || header.num_records_in_root == 0 {
        return Ok(Vec::new());
    }
    validate_btree_v2_depth(header.depth)?;

    let heap_id_len = compute_heap_id_len(header);
    let mut records = Vec::new();
    let mut visited = HashSet::new();

    if header.depth == 0 {
        enter_btree_v2_node(&mut visited, header.root_node_address)?;
        parse_leaf_node_storage(
            storage,
            header.root_node_address,
            header,
            offset_size,
            length_size,
            ndims,
            chunk_dims,
            chunk_bounds,
            link_name_hash,
            header.num_records_in_root,
            heap_id_len,
            &mut records,
        )?;
    } else {
        parse_internal_node_storage(
            storage,
            header.root_node_address,
            header,
            offset_size,
            length_size,
            ndims,
            chunk_dims,
            chunk_bounds,
            link_name_hash,
            header.num_records_in_root,
            header.depth,
            heap_id_len,
            &mut visited,
            &mut records,
        )?;
    }

    Ok(records)
}

/// Collect link-name records whose stored hash matches `target_hash`.
pub(crate) fn collect_btree_v2_link_name_hash_records_storage(
    storage: &dyn Storage,
    header: &BTreeV2Header,
    offset_size: u8,
    length_size: u8,
    target_hash: u32,
) -> Result<Vec<BTreeV2Record>> {
    collect_btree_v2_records_storage_with_link_hash(
        storage,
        header,
        offset_size,
        length_size,
        None,
        &[],
        None,
        Some(target_hash),
    )
}

#[allow(clippy::too_many_arguments)]
fn parse_leaf_node_storage(
    storage: &dyn Storage,
    address: u64,
    header: &BTreeV2Header,
    offset_size: u8,
    length_size: u8,
    ndims: Option<u32>,
    chunk_dims: &[u32],
    chunk_bounds: Option<(&[u64], &[u64])>,
    link_name_hash: Option<u32>,
    num_records: u16,
    heap_id_len: usize,
    records: &mut Vec<BTreeV2Record>,
) -> Result<()> {
    let node_len = usize::try_from(header.node_size).map_err(|_| {
        Error::InvalidData("B-tree v2 node size exceeds platform usize capacity".into())
    })?;
    let node_bytes = storage.read_range(address, node_len)?;
    let mut cursor = Cursor::new(node_bytes.as_ref());
    parse_leaf_node(
        &mut cursor,
        header,
        offset_size,
        length_size,
        ndims,
        chunk_dims,
        chunk_bounds,
        link_name_hash,
        num_records,
        heap_id_len,
        records,
    )
}

#[allow(clippy::too_many_arguments)]
fn parse_internal_node_storage(
    storage: &dyn Storage,
    address: u64,
    header: &BTreeV2Header,
    offset_size: u8,
    length_size: u8,
    ndims: Option<u32>,
    chunk_dims: &[u32],
    chunk_bounds: Option<(&[u64], &[u64])>,
    link_name_hash: Option<u32>,
    num_records: u16,
    depth: u16,
    heap_id_len: usize,
    visited: &mut HashSet<u64>,
    records: &mut Vec<BTreeV2Record>,
) -> Result<()> {
    if depth == 0 {
        return Err(Error::InvalidData(
            "B-tree v2 internal node traversal reached depth zero".into(),
        ));
    }
    enter_btree_v2_node(visited, address)?;

    let node_len = usize::try_from(header.node_size).map_err(|_| {
        Error::InvalidData("B-tree v2 node size exceeds platform usize capacity".into())
    })?;
    let node_bytes = storage.read_range(address, node_len)?;
    let mut cursor = Cursor::new(node_bytes.as_ref());
    let start = cursor.position();

    let sig = cursor.read_bytes(4)?;
    if sig != BTIN_SIGNATURE {
        return Err(Error::InvalidBTreeV2Signature {
            context: "internal node",
        });
    }

    let version = cursor.read_u8()?;
    if version != 0 {
        return Err(Error::UnsupportedBTreeVersion(version));
    }

    let node_type = cursor.read_u8()?;
    if node_type != header.btree_type {
        return Err(Error::InvalidData(format!(
            "B-tree v2 internal node type mismatch: header says {}, node says {}",
            header.btree_type, node_type
        )));
    }

    let max_child_records = if depth == 1 {
        max_leaf_records(header.node_size, header.record_size)
    } else {
        let leaf_max = max_leaf_records(header.node_size, header.record_size);
        let mut prev_max = leaf_max;
        for _ in 1..depth {
            prev_max =
                max_internal_records(header.node_size, header.record_size, offset_size, prev_max);
        }
        prev_max
    };
    let nrec_bytes = num_records_size(max_child_records);

    let mut node_records = Vec::with_capacity(num_records as usize);
    for _ in 0..num_records {
        let record = parse_record(
            &mut cursor,
            header.btree_type,
            header.record_size,
            offset_size,
            length_size,
            ndims,
            chunk_dims,
            heap_id_len,
        )?;
        node_records.push(record);
    }

    let num_children = usize::from(num_records) + 1;
    let has_total_records = depth > 1;
    let total_nrec_bytes = if has_total_records {
        usize::from(length_size)
    } else {
        0
    };

    let mut child_addresses = Vec::with_capacity(num_children);
    let mut child_nrecords = Vec::with_capacity(num_children);
    for _ in 0..num_children {
        child_addresses.push(cursor.read_offset(offset_size)?);
        child_nrecords.push(cursor.read_uvar(nrec_bytes)? as u16);
        if has_total_records {
            cursor.read_uvar(total_nrec_bytes)?;
        }
    }

    let compact_end = cursor.position();
    verify_node_checksum(
        &mut cursor,
        start,
        header.node_size,
        compact_end,
        "internal node",
    )?;

    let child_depth = depth - 1;
    for (i, record) in node_records.iter().enumerate() {
        let child_addr = child_addresses[i];
        if !Cursor::is_undefined_offset(child_addr, offset_size)
            && child_may_match_link_name_hash(&node_records, i, link_name_hash)
        {
            let child_nrec = child_nrecords[i];
            if child_depth == 0 {
                enter_btree_v2_node(visited, child_addr)?;
                parse_leaf_node_storage(
                    storage,
                    child_addr,
                    header,
                    offset_size,
                    length_size,
                    ndims,
                    chunk_dims,
                    chunk_bounds,
                    link_name_hash,
                    child_nrec,
                    heap_id_len,
                    records,
                )?;
            } else {
                parse_internal_node_storage(
                    storage,
                    child_addr,
                    header,
                    offset_size,
                    length_size,
                    ndims,
                    chunk_dims,
                    chunk_bounds,
                    link_name_hash,
                    child_nrec,
                    child_depth,
                    heap_id_len,
                    visited,
                    records,
                )?;
            }
        }

        if record_matches_query(record, chunk_dims, chunk_bounds, link_name_hash) {
            records.push(record.clone());
        }
    }

    let final_child_index = node_records.len();
    let final_child_addr = child_addresses[final_child_index];
    if !Cursor::is_undefined_offset(final_child_addr, offset_size)
        && child_may_match_link_name_hash(&node_records, final_child_index, link_name_hash)
    {
        let child_nrec = child_nrecords[final_child_index];
        if child_depth == 0 {
            enter_btree_v2_node(visited, final_child_addr)?;
            parse_leaf_node_storage(
                storage,
                final_child_addr,
                header,
                offset_size,
                length_size,
                ndims,
                chunk_dims,
                chunk_bounds,
                link_name_hash,
                child_nrec,
                heap_id_len,
                records,
            )?;
        } else {
            parse_internal_node_storage(
                storage,
                final_child_addr,
                header,
                offset_size,
                length_size,
                ndims,
                chunk_dims,
                chunk_bounds,
                link_name_hash,
                child_nrec,
                child_depth,
                heap_id_len,
                visited,
                records,
            )?;
        }
    }

    Ok(())
}

/// Compute the heap ID length from the record size and tree type.
///
/// For link/attribute B-trees (types 5, 6, 8, 9), the heap ID occupies
/// the bytes not used by the fixed fields. For chunk types (10, 11) or
/// unknown types, return 0 (heap_id is not used).
fn compute_heap_id_len(header: &BTreeV2Header) -> usize {
    let rs = header.record_size as usize;
    match header.btree_type {
        5 => rs.saturating_sub(4),         // hash(4)
        6 => rs.saturating_sub(8),         // order(8)
        8 => rs.saturating_sub(4 + 1 + 4), // hash(4) + flags(1) + creation_order(4)
        9 => rs.saturating_sub(1 + 4),     // flags(1) + order(4)
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal BTHD with the given parameters.
    #[allow(clippy::too_many_arguments)]
    fn build_header(
        btree_type: u8,
        node_size: u32,
        record_size: u16,
        depth: u16,
        root_node_address: u64,
        num_records_in_root: u16,
        total_records: u64,
        offset_size: u8,
        length_size: u8,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"BTHD");
        buf.push(0); // version
        buf.push(btree_type);
        buf.extend_from_slice(&node_size.to_le_bytes());
        buf.extend_from_slice(&record_size.to_le_bytes());
        buf.extend_from_slice(&depth.to_le_bytes());
        buf.push(75); // split percent
        buf.push(40); // merge percent
        match offset_size {
            4 => buf.extend_from_slice(&(root_node_address as u32).to_le_bytes()),
            8 => buf.extend_from_slice(&root_node_address.to_le_bytes()),
            _ => panic!("unsupported"),
        }
        buf.extend_from_slice(&num_records_in_root.to_le_bytes());
        match length_size {
            4 => buf.extend_from_slice(&(total_records as u32).to_le_bytes()),
            8 => buf.extend_from_slice(&total_records.to_le_bytes()),
            _ => panic!("unsupported"),
        }
        // Compute and append checksum.
        let checksum = jenkins_lookup3(&buf);
        buf.extend_from_slice(&checksum.to_le_bytes());
        buf
    }

    #[test]
    fn parse_header() {
        let data = build_header(5, 4096, 12, 0, 0x1000, 3, 3, 8, 8);
        let mut cursor = Cursor::new(&data);
        let hdr = BTreeV2Header::parse(&mut cursor, 8, 8).unwrap();

        assert_eq!(hdr.btree_type, 5);
        assert_eq!(hdr.node_size, 4096);
        assert_eq!(hdr.record_size, 12);
        assert_eq!(hdr.depth, 0);
        assert_eq!(hdr.split_percent, 75);
        assert_eq!(hdr.merge_percent, 40);
        assert_eq!(hdr.root_node_address, 0x1000);
        assert_eq!(hdr.num_records_in_root, 3);
        assert_eq!(hdr.total_records, 3);
    }

    #[test]
    fn bad_signature() {
        let mut data = build_header(5, 4096, 12, 0, 0x1000, 0, 0, 8, 8);
        data[0] = b'X';
        let mut cursor = Cursor::new(&data);
        assert!(matches!(
            BTreeV2Header::parse(&mut cursor, 8, 8),
            Err(Error::InvalidBTreeV2Signature { .. })
        ));
    }

    #[test]
    fn bad_checksum() {
        let mut data = build_header(5, 4096, 12, 0, 0x1000, 0, 0, 8, 8);
        // Corrupt a byte in the middle.
        data[6] = 0xFF;
        let mut cursor = Cursor::new(&data);
        assert!(matches!(
            BTreeV2Header::parse(&mut cursor, 8, 8),
            Err(Error::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn collect_empty_tree() {
        let header = BTreeV2Header {
            btree_type: 5,
            node_size: 4096,
            record_size: 12,
            depth: 0,
            split_percent: 75,
            merge_percent: 40,
            root_node_address: u64::MAX,
            num_records_in_root: 0,
            total_records: 0,
        };
        let data = vec![0u8; 100];
        let records = collect_btree_v2_records(&data, &header, 8, 8, None, &[], None).unwrap();
        assert!(records.is_empty());
    }

    #[test]
    fn heap_id_len_uses_record_type_sizes() {
        // Type 5: record_size - 4
        let h5 = BTreeV2Header {
            btree_type: 5,
            record_size: 12,
            node_size: 0,
            depth: 0,
            split_percent: 0,
            merge_percent: 0,
            root_node_address: 0,
            num_records_in_root: 0,
            total_records: 0,
        };
        assert_eq!(compute_heap_id_len(&h5), 8);

        // Type 8: record_size - 9
        let h8 = BTreeV2Header {
            btree_type: 8,
            record_size: 17,
            ..h5
        };
        assert_eq!(compute_heap_id_len(&h8), 8);

        // Type 9: record_size - flags(1) - order(4)
        let h9 = BTreeV2Header {
            btree_type: 9,
            record_size: 13,
            ..h5
        };
        assert_eq!(compute_heap_id_len(&h9), 8);
    }

    #[test]
    fn parse_attribute_name_record_uses_on_disk_field_order() {
        let heap_id = [0x00, 0xbe, 0x00, 0x00, 0x00, 0x00, 0x1c, 0x00];
        let mut data = heap_id.to_vec();
        data.push(0x02);
        data.extend_from_slice(&6u32.to_le_bytes());
        data.extend_from_slice(&0x0396_dda5u32.to_le_bytes());
        let mut cursor = Cursor::new(&data);

        let record = parse_record(&mut cursor, 8, data.len() as u16, 8, 8, None, &[], 8).unwrap();
        match record {
            BTreeV2Record::AttributeNameHash {
                hash,
                flags,
                creation_order,
                heap_id: parsed_heap_id,
            } => {
                assert_eq!(parsed_heap_id, heap_id);
                assert_eq!(flags, 0x02);
                assert_eq!(creation_order, 6);
                assert_eq!(hash, 0x0396_dda5);
            }
            other => panic!("expected attribute-name record, got {other:?}"),
        }
    }

    #[test]
    fn parse_attribute_creation_order_record_uses_on_disk_field_order() {
        let heap_id = [0x00, 0x16, 0x00, 0x00, 0x00, 0x00, 0x1c, 0x00];
        let mut data = heap_id.to_vec();
        data.push(0x02);
        data.extend_from_slice(&7u32.to_le_bytes());
        let mut cursor = Cursor::new(&data);

        let record = parse_record(&mut cursor, 9, data.len() as u16, 8, 8, None, &[], 8).unwrap();
        match record {
            BTreeV2Record::AttributeCreationOrder {
                order,
                heap_id: parsed_heap_id,
            } => {
                assert_eq!(parsed_heap_id, heap_id);
                assert_eq!(order, 7);
            }
            other => panic!("expected attribute-creation-order record, got {other:?}"),
        }
    }

    #[test]
    fn max_leaf_records_accounts_for_node_overhead() {
        // node_size=4096, record_size=12, overhead=10
        // => (4096 - 10) / 12 = 340
        assert_eq!(max_leaf_records(4096, 12), 340);
    }

    #[test]
    fn record_count_size_scales_at_integer_boundaries() {
        assert_eq!(num_records_size(0), 1);
        assert_eq!(num_records_size(255), 1);
        assert_eq!(num_records_size(256), 2);
        assert_eq!(num_records_size(65535), 2);
        assert_eq!(num_records_size(65536), 4);
    }

    #[test]
    fn parse_huge_indirect_record() {
        let mut data = Vec::new();
        data.extend_from_slice(&0x1234u64.to_le_bytes());
        data.extend_from_slice(&99u64.to_le_bytes());
        data.extend_from_slice(&7u64.to_le_bytes());
        let mut cursor = Cursor::new(&data);

        let record = parse_record(&mut cursor, 1, data.len() as u16, 8, 8, None, &[], 0).unwrap();
        match record {
            BTreeV2Record::HugeIndirectNonFiltered {
                address,
                length,
                object_id,
            } => {
                assert_eq!(address, 0x1234);
                assert_eq!(length, 99);
                assert_eq!(object_id, 7);
            }
            other => panic!("expected huge record, got {:?}", other),
        }
    }

    #[test]
    fn parse_shared_heap_record() {
        let mut data = Vec::new();
        data.push(0);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&0xAABB_CCDDu32.to_le_bytes());
        data.extend_from_slice(&3u32.to_le_bytes());
        data.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        let mut cursor = Cursor::new(&data);

        let record = parse_record(&mut cursor, 7, data.len() as u16, 8, 8, None, &[], 0).unwrap();
        match record {
            BTreeV2Record::SharedMessageHeap {
                hash,
                reference_count,
                heap_id,
            } => {
                assert_eq!(hash, 0xAABB_CCDD);
                assert_eq!(reference_count, 3);
                assert_eq!(heap_id, vec![1, 2, 3, 4, 5, 6, 7, 8]);
            }
            other => panic!("expected shared heap record, got {:?}", other),
        }
    }

    #[test]
    fn parse_record_rejects_known_record_that_exceeds_record_size() {
        let mut data = Vec::new();
        data.extend_from_slice(&0x1234u64.to_le_bytes());
        data.extend_from_slice(&99u64.to_le_bytes());
        data.extend_from_slice(&7u64.to_le_bytes());
        let mut cursor = Cursor::new(&data);

        let err = parse_record(&mut cursor, 1, 16, 8, 8, None, &[], 0).unwrap_err();
        assert!(
            matches!(err, Error::InvalidData(message) if message.contains("consumed 24 bytes but record size is 16"))
        );
    }

    #[test]
    fn parse_chunk_record_rejects_size_shorter_than_address() {
        let mut data = Vec::new();
        data.extend_from_slice(&0x1234u64.to_le_bytes());
        let mut cursor = Cursor::new(&data);

        let err = parse_record(&mut cursor, 10, 4, 8, 8, None, &[], 0).unwrap_err();
        assert!(
            matches!(err, Error::InvalidData(message) if message.contains("shorter than its address"))
        );
        assert_eq!(cursor.position(), 0);
    }

    #[test]
    fn parse_filtered_chunk_record_rejects_size_shorter_than_fixed_fields() {
        let mut data = Vec::new();
        data.extend_from_slice(&0x1234u64.to_le_bytes());
        data.extend_from_slice(&99u64.to_le_bytes());
        data.extend_from_slice(&0xAu32.to_le_bytes());
        let mut cursor = Cursor::new(&data);

        // One dimension: fixed fields are address(8) + chunk_size(>=1) +
        // filter_mask(4) + offset(8) = 21+, so a 16-byte record is too short.
        let err = parse_record(&mut cursor, 11, 16, 8, 8, Some(1), &[], 0).unwrap_err();
        assert!(
            matches!(err, Error::InvalidData(message) if message.contains("shorter than its fixed fields"))
        );
        assert_eq!(cursor.position(), 0);
    }

    #[test]
    fn parse_chunk_record_scales_offsets_by_chunk_dimensions() {
        let mut data = Vec::new();
        data.extend_from_slice(&0x1234u64.to_le_bytes());
        data.extend_from_slice(&2u64.to_le_bytes());
        data.extend_from_slice(&1u64.to_le_bytes());
        let mut cursor = Cursor::new(&data);

        let record = parse_record(
            &mut cursor,
            10,
            data.len() as u16,
            8,
            8,
            Some(2),
            &[5, 7],
            0,
        )
        .unwrap();
        match record {
            BTreeV2Record::ChunkedNonFiltered { address, offsets } => {
                assert_eq!(address, 0x1234);
                assert_eq!(offsets, vec![10, 7]);
            }
            other => panic!("expected non-filtered chunk record, got {:?}", other),
        }
    }

    #[test]
    fn parse_filtered_chunk_record_scales_offsets_by_chunk_dimensions() {
        let mut data = Vec::new();
        data.extend_from_slice(&0x1234u64.to_le_bytes());
        data.extend_from_slice(&99u64.to_le_bytes());
        data.extend_from_slice(&0xAu32.to_le_bytes());
        data.extend_from_slice(&3u64.to_le_bytes());
        data.extend_from_slice(&4u64.to_le_bytes());
        let mut cursor = Cursor::new(&data);

        let record = parse_record(
            &mut cursor,
            11,
            data.len() as u16,
            8,
            8,
            Some(2),
            &[5, 7],
            0,
        )
        .unwrap();
        match record {
            BTreeV2Record::ChunkedFiltered {
                address,
                chunk_size,
                filter_mask,
                offsets,
            } => {
                assert_eq!(address, 0x1234);
                assert_eq!(chunk_size, 99);
                assert_eq!(filter_mask, 0xA);
                assert_eq!(offsets, vec![15, 28]);
            }
            other => panic!("expected filtered chunk record, got {:?}", other),
        }
    }

    #[test]
    fn parse_leaf_with_type5_records() {
        // Build a leaf node with 2 type-5 records (link name hash).
        // record_size = 12 (hash=4 + heap_id=8)
        let record_size: u16 = 12;
        let node_size: u32 = 4096;

        let header = BTreeV2Header {
            btree_type: 5,
            node_size,
            record_size,
            depth: 0,
            split_percent: 75,
            merge_percent: 40,
            root_node_address: 0, // will point to our leaf
            num_records_in_root: 2,
            total_records: 2,
        };

        // Build the leaf node.
        let mut leaf = Vec::new();
        leaf.extend_from_slice(b"BTLF"); // signature
        leaf.push(0); // version
        leaf.push(5); // type

        // Record 1: hash=0xAABBCCDD, heap_id=[1,2,3,4,5,6,7,8]
        leaf.extend_from_slice(&0xAABBCCDDu32.to_le_bytes());
        leaf.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);

        // Record 2: hash=0x11223344, heap_id=[9,10,11,12,13,14,15,16]
        leaf.extend_from_slice(&0x11223344u32.to_le_bytes());
        leaf.extend_from_slice(&[9, 10, 11, 12, 13, 14, 15, 16]);

        // The checksum covers the populated node payload. The node allocation
        // may be larger and is padded after the checksum.
        let checksum = jenkins_lookup3(&leaf);
        leaf.extend_from_slice(&checksum.to_le_bytes());
        leaf.resize(node_size as usize, 0);

        let mut records = Vec::new();
        let mut cursor = Cursor::new(&leaf);
        parse_leaf_node(
            &mut cursor,
            &header,
            8,
            8,
            None,
            &[],
            None,
            None,
            2,
            8, // heap_id_len
            &mut records,
        )
        .unwrap();

        assert_eq!(records.len(), 2);
        match &records[0] {
            BTreeV2Record::LinkNameHash { hash, heap_id } => {
                assert_eq!(*hash, 0xAABBCCDD);
                assert_eq!(heap_id, &[1, 2, 3, 4, 5, 6, 7, 8]);
            }
            _ => panic!("expected LinkNameHash"),
        }
        match &records[1] {
            BTreeV2Record::LinkNameHash { hash, heap_id } => {
                assert_eq!(*hash, 0x11223344);
                assert_eq!(heap_id, &[9, 10, 11, 12, 13, 14, 15, 16]);
            }
            _ => panic!("expected LinkNameHash"),
        }
    }

    fn build_type5_record(hash: u32, heap_id: [u8; 8]) -> Vec<u8> {
        let mut record = Vec::new();
        record.extend_from_slice(&hash.to_le_bytes());
        record.extend_from_slice(&heap_id);
        record
    }

    fn build_type5_leaf(records: &[(u32, [u8; 8])], node_size: usize) -> Vec<u8> {
        let mut leaf = Vec::new();
        leaf.extend_from_slice(b"BTLF");
        leaf.push(0);
        leaf.push(5);
        for &(hash, heap_id) in records {
            leaf.extend_from_slice(&build_type5_record(hash, heap_id));
        }
        let checksum = jenkins_lookup3(&leaf);
        leaf.extend_from_slice(&checksum.to_le_bytes());
        leaf.resize(node_size, 0);
        leaf
    }

    fn build_type5_internal(
        records: &[(u32, [u8; 8])],
        children: &[(u64, u8)],
        node_size: usize,
    ) -> Vec<u8> {
        assert_eq!(children.len(), records.len() + 1);

        let mut node = Vec::new();
        node.extend_from_slice(b"BTIN");
        node.push(0);
        node.push(5);
        for &(hash, heap_id) in records {
            node.extend_from_slice(&build_type5_record(hash, heap_id));
        }
        for &(address, child_records) in children {
            node.extend_from_slice(&address.to_le_bytes());
            node.push(child_records);
        }
        let checksum = jenkins_lookup3(&node);
        node.extend_from_slice(&checksum.to_le_bytes());
        node.resize(node_size, 0);
        node
    }

    fn link_hashes(records: &[BTreeV2Record]) -> Vec<u32> {
        records
            .iter()
            .map(|record| match record {
                BTreeV2Record::LinkNameHash { hash, .. } => *hash,
                other => panic!("expected LinkNameHash, got {other:?}"),
            })
            .collect()
    }

    #[test]
    fn collect_depth_one_tree_includes_internal_records_in_order() {
        // Mirrors HDF5 H5B2__iterate_node: recurse into child[i], visit
        // record[i], then recurse into the final child. Internal-node records
        // are callback records, not just separators.
        let node_size = 128usize;
        let left_addr = node_size as u64;
        let right_addr = (node_size * 2) as u64;
        let header = BTreeV2Header {
            btree_type: 5,
            node_size: node_size as u32,
            record_size: 12,
            depth: 1,
            split_percent: 75,
            merge_percent: 40,
            root_node_address: 0,
            num_records_in_root: 1,
            total_records: 3,
        };

        let root = build_type5_internal(
            &[(20, [20; 8])],
            &[(left_addr, 1), (right_addr, 1)],
            node_size,
        );
        let left_leaf = build_type5_leaf(&[(10, [10; 8])], node_size);
        let right_leaf = build_type5_leaf(&[(30, [30; 8])], node_size);

        let mut data = vec![0; node_size * 3];
        data[0..node_size].copy_from_slice(&root);
        data[node_size..node_size * 2].copy_from_slice(&left_leaf);
        data[node_size * 2..node_size * 3].copy_from_slice(&right_leaf);

        let records = collect_btree_v2_records(&data, &header, 8, 8, None, &[], None).unwrap();
        assert_eq!(link_hashes(&records), vec![10, 20, 30]);

        let storage = crate::storage::BytesStorage::new(data);
        let records =
            collect_btree_v2_records_storage(&storage, &header, 8, 8, None, &[], None).unwrap();
        assert_eq!(link_hashes(&records), vec![10, 20, 30]);
    }

    #[test]
    fn collect_rejects_depth_above_traversal_limit() {
        let header = BTreeV2Header {
            btree_type: 5,
            node_size: 128,
            record_size: 12,
            depth: MAX_BTREE_V2_DEPTH + 1,
            split_percent: 75,
            merge_percent: 40,
            root_node_address: 0,
            num_records_in_root: 1,
            total_records: 1,
        };
        let data = vec![0; 128];

        let err = collect_btree_v2_records(&data, &header, 8, 8, None, &[], None).unwrap_err();
        assert!(
            matches!(err, Error::InvalidData(message) if message.contains("exceeds traversal limit"))
        );
    }

    #[test]
    fn collect_rejects_revisited_leaf_node() {
        let node_size = 128usize;
        let leaf_addr = node_size as u64;
        let header = BTreeV2Header {
            btree_type: 5,
            node_size: node_size as u32,
            record_size: 12,
            depth: 1,
            split_percent: 75,
            merge_percent: 40,
            root_node_address: 0,
            num_records_in_root: 1,
            total_records: 3,
        };

        let root = build_type5_internal(
            &[(20, [20; 8])],
            &[(leaf_addr, 1), (leaf_addr, 1)],
            node_size,
        );
        let leaf = build_type5_leaf(&[(10, [10; 8])], node_size);

        let mut data = vec![0; node_size * 2];
        data[0..node_size].copy_from_slice(&root);
        data[node_size..node_size * 2].copy_from_slice(&leaf);

        let err = collect_btree_v2_records(&data, &header, 8, 8, None, &[], None).unwrap_err();
        assert!(matches!(err, Error::InvalidData(message) if message.contains("revisits node")));

        let storage = crate::storage::BytesStorage::new(data);
        let err =
            collect_btree_v2_records_storage(&storage, &header, 8, 8, None, &[], None).unwrap_err();
        assert!(matches!(err, Error::InvalidData(message) if message.contains("revisits node")));
    }

    #[test]
    fn collect_link_name_hash_records_filters_leaf_records() {
        let record_size: u16 = 12;
        let node_size: u32 = 4096;
        let header = BTreeV2Header {
            btree_type: 5,
            node_size,
            record_size,
            depth: 0,
            split_percent: 75,
            merge_percent: 40,
            root_node_address: 0,
            num_records_in_root: 2,
            total_records: 2,
        };

        let mut leaf = Vec::new();
        leaf.extend_from_slice(b"BTLF");
        leaf.push(0);
        leaf.push(5);
        leaf.extend_from_slice(&0x1111_1111u32.to_le_bytes());
        leaf.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        leaf.extend_from_slice(&0x2222_2222u32.to_le_bytes());
        leaf.extend_from_slice(&[9, 10, 11, 12, 13, 14, 15, 16]);
        let checksum = jenkins_lookup3(&leaf);
        leaf.extend_from_slice(&checksum.to_le_bytes());
        leaf.resize(node_size as usize, 0);

        let records =
            collect_btree_v2_link_name_hash_records(&leaf, &header, 8, 8, 0x2222_2222).unwrap();
        assert_eq!(records.len(), 1);
        match &records[0] {
            BTreeV2Record::LinkNameHash { hash, heap_id } => {
                assert_eq!(*hash, 0x2222_2222);
                assert_eq!(heap_id, &[9, 10, 11, 12, 13, 14, 15, 16]);
            }
            other => panic!("expected LinkNameHash, got {other:?}"),
        }
    }

    #[test]
    fn child_link_hash_filter_prunes_disjoint_hash_ranges() {
        let records = vec![
            BTreeV2Record::LinkNameHash {
                hash: 10,
                heap_id: vec![],
            },
            BTreeV2Record::LinkNameHash {
                hash: 20,
                heap_id: vec![],
            },
        ];

        assert!(child_may_match_link_name_hash(&records, 0, Some(10)));
        assert!(!child_may_match_link_name_hash(&records, 0, Some(15)));
        assert!(child_may_match_link_name_hash(&records, 1, Some(15)));
        assert!(!child_may_match_link_name_hash(&records, 2, Some(15)));
        assert!(child_may_match_link_name_hash(&records, 2, Some(20)));
    }
}
