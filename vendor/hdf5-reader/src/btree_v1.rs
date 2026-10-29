//! HDF5 B-link Tree Version 1.
//!
//! V1 B-trees are used by old-style (v0/v1) groups for link name lookup
//! (type 0) and by chunked datasets for raw data chunk indexing (type 1).
//!
//! The tree is a B-link tree: each node carries left and right sibling
//! pointers. Keys bracket children — a node with N children has N+1 keys.
//! Leaf nodes (level 0) point to either symbol table nodes (SNODs) for
//! type 0 or raw data chunk addresses for type 1. Internal nodes point to
//! child B-tree nodes.

use std::collections::HashSet;

use crate::error::{Error, Result};
use crate::io::Cursor;
use crate::storage::Storage;

/// Signature bytes for a v1 B-tree node: ASCII `TREE`.
const BTREE_V1_SIGNATURE: [u8; 4] = *b"TREE";
const MAX_BTREE_V1_RECURSION_DEPTH: usize = 256;

/// A key within a v1 B-tree node.
#[derive(Debug, Clone)]
pub enum BTreeV1Key {
    /// Type 0 (group) key: offset into the local heap for the link name.
    Group { local_heap_offset: u64 },
    /// Type 1 (raw data chunk) key: chunk size, filter mask, and per-dimension
    /// offsets (including one extra for the dataset element offset).
    RawData {
        chunk_size: u32,
        filter_mask: u32,
        offsets: Vec<u64>,
    },
}

/// A parsed v1 B-tree node.
#[derive(Debug, Clone)]
pub struct BTreeV1Node {
    /// Node type: 0 = group, 1 = raw data chunk.
    pub node_type: u8,
    /// Node level: 0 = leaf, > 0 = internal.
    pub level: u8,
    /// Number of entries (children) actually in use.
    pub entries_used: u16,
    /// Address of the left sibling node (undefined if none).
    pub left_sibling: u64,
    /// Address of the right sibling node (undefined if none).
    pub right_sibling: u64,
    /// Keys bracketing the children. Length is `entries_used + 1`.
    pub keys: Vec<BTreeV1Key>,
    /// Child addresses. Length is `entries_used`.
    pub children: Vec<u64>,
}

impl BTreeV1Node {
    /// Parse a v1 B-tree node at the current cursor position.
    ///
    /// `ndims` is required for type-1 (raw data chunk) nodes — it is the
    /// number of dimensions of the dataset's dataspace. For type-0 (group)
    /// nodes pass `None`.
    ///
    /// Format:
    /// - Signature: `TREE` (4 bytes)
    /// - Node type (u8): 0 = group, 1 = raw data
    /// - Node level (u8)
    /// - Entries used (u16 LE)
    /// - Left sibling address (`offset_size` bytes)
    /// - Right sibling address (`offset_size` bytes)
    /// - Then interleaved keys and child pointers:
    ///   `key[0], child[0], key[1], child[1], ..., key[K-1], child[K-1], key[K]`
    ///   where K = entries_used.
    pub fn parse(
        cursor: &mut Cursor,
        offset_size: u8,
        length_size: u8,
        ndims: Option<u32>,
    ) -> Result<Self> {
        let sig = cursor.read_bytes(4)?;
        if sig != BTREE_V1_SIGNATURE {
            return Err(Error::InvalidBTreeSignature);
        }

        let node_type = cursor.read_u8()?;
        let level = cursor.read_u8()?;
        let entries_used = cursor.read_u16_le()?;
        let left_sibling = cursor.read_offset(offset_size)?;
        let right_sibling = cursor.read_offset(offset_size)?;

        let num_keys = entries_used as usize + 1;
        let num_children = entries_used as usize;

        let mut keys = Vec::with_capacity(num_keys);
        let mut children = Vec::with_capacity(num_children);

        // Read interleaved keys and children:
        // key[0], child[0], key[1], child[1], ..., key[K-1], child[K-1], key[K]
        for i in 0..num_keys {
            let key = match node_type {
                0 => parse_group_key(cursor, length_size)?,
                1 => parse_raw_data_key(cursor, ndims)?,
                _ => {
                    return Err(Error::InvalidData(format!(
                        "unknown v1 B-tree node type: {node_type}"
                    )));
                }
            };
            keys.push(key);

            // Read child address after every key except the last.
            if i < num_children {
                let child_addr = cursor.read_offset(offset_size)?;
                children.push(child_addr);
            }
        }

        Ok(BTreeV1Node {
            node_type,
            level,
            entries_used,
            left_sibling,
            right_sibling,
            keys,
            children,
        })
    }
}

/// Parse a type-0 (group) key: just a length-sized offset into the local heap.
fn parse_group_key(cursor: &mut Cursor, length_size: u8) -> Result<BTreeV1Key> {
    let local_heap_offset = cursor.read_length(length_size)?;
    Ok(BTreeV1Key::Group { local_heap_offset })
}

/// Parse a type-1 (raw data chunk) key.
///
/// Format:
/// - chunk_size (u32 LE) — size of the chunk in bytes after filters
/// - filter_mask (u32 LE) — bit mask of filters to skip
/// - (ndims + 1) chunk-offset values per dimension (plus the trailing element
///   offset, typically 0)
///
/// Each chunk-offset value is **always 8 bytes**, independent of the
/// superblock's "size of offsets" field. The HDF5 format spec fixes these key
/// dimension offsets at 64 bits (see the "Version 1 B-trees" / raw-data chunk
/// key layout); reading them as `size_of_offsets` bytes corrupts the cursor
/// position on 32-bit-superblock files (`size_of_offsets = 4`), yielding a
/// garbage chunk address and a failed chunk decode.
fn parse_raw_data_key(cursor: &mut Cursor, ndims: Option<u32>) -> Result<BTreeV1Key> {
    let nd = ndims.ok_or_else(|| {
        Error::InvalidData("ndims required for raw data chunk B-tree keys".into())
    })?;

    let chunk_size = cursor.read_u32_le()?;
    let filter_mask = cursor.read_u32_le()?;

    let num_offsets = nd as usize + 1;
    let mut offsets = Vec::with_capacity(num_offsets);
    for _ in 0..num_offsets {
        // Fixed 64-bit dimension offset per the HDF5 spec — NOT size_of_offsets.
        offsets.push(cursor.read_u64_le()?);
    }

    Ok(BTreeV1Key::RawData {
        chunk_size,
        filter_mask,
        offsets,
    })
}

/// Walk a v1 B-tree and collect all (key, child_address) pairs from leaf nodes.
///
/// For leaf nodes (level 0), this returns one entry per child paired with the
/// key that precedes it (`key[i]` for `child[i]`). For internal nodes, this
/// recurses into each child node.
///
/// `data` must be the full file data (or at least the region containing the
/// B-tree nodes). `root_address` is the file offset of the root node.
pub fn collect_btree_v1_leaves(
    data: &[u8],
    root_address: u64,
    offset_size: u8,
    length_size: u8,
    ndims: Option<u32>,
    chunk_dims: &[u32],
    chunk_bounds: Option<(&[u64], &[u64])>,
) -> Result<Vec<(BTreeV1Key, u64)>> {
    let mut results = Vec::new();
    let mut visited = HashSet::new();
    collect_recursive(
        BTreeV1CollectContext {
            data,
            offset_size,
            length_size,
            ndims,
            chunk_dims,
            chunk_bounds,
        },
        root_address,
        MAX_BTREE_V1_RECURSION_DEPTH,
        &mut visited,
        &mut results,
    )?;
    Ok(results)
}

/// Walk a v1 B-tree from random-access storage and collect leaf entries.
pub fn collect_btree_v1_leaves_storage(
    storage: &dyn Storage,
    root_address: u64,
    offset_size: u8,
    length_size: u8,
    ndims: Option<u32>,
    chunk_dims: &[u32],
    chunk_bounds: Option<(&[u64], &[u64])>,
) -> Result<Vec<(BTreeV1Key, u64)>> {
    let mut results = Vec::new();
    let mut visited = HashSet::new();
    collect_recursive_storage(
        BTreeV1CollectContextStorage {
            storage,
            offset_size,
            length_size,
            ndims,
            chunk_dims,
            chunk_bounds,
        },
        root_address,
        MAX_BTREE_V1_RECURSION_DEPTH,
        &mut visited,
        &mut results,
    )?;
    Ok(results)
}

#[derive(Clone, Copy)]
struct BTreeV1CollectContext<'a> {
    data: &'a [u8],
    offset_size: u8,
    length_size: u8,
    ndims: Option<u32>,
    chunk_dims: &'a [u32],
    chunk_bounds: Option<(&'a [u64], &'a [u64])>,
}

#[derive(Clone, Copy)]
struct BTreeV1CollectContextStorage<'a> {
    storage: &'a dyn Storage,
    offset_size: u8,
    length_size: u8,
    ndims: Option<u32>,
    chunk_dims: &'a [u32],
    chunk_bounds: Option<(&'a [u64], &'a [u64])>,
}

/// Recursive helper for tree traversal.
fn collect_recursive(
    context: BTreeV1CollectContext<'_>,
    address: u64,
    depth_remaining: usize,
    visited: &mut HashSet<u64>,
    results: &mut Vec<(BTreeV1Key, u64)>,
) -> Result<()> {
    if Cursor::is_undefined_offset(address, context.offset_size) {
        return Ok(());
    }
    if depth_remaining == 0 {
        return Err(Error::InvalidData(
            "B-tree v1 traversal exceeded recursion limit".into(),
        ));
    }
    if !visited.insert(address) {
        return Err(Error::InvalidData(format!(
            "B-tree v1 traversal revisits node at offset {address:#x}"
        )));
    }

    if address as usize >= context.data.len() {
        return Err(Error::OffsetOutOfBounds(address));
    }

    let mut cursor = Cursor::new(context.data);
    cursor.set_position(address);

    let node = BTreeV1Node::parse(
        &mut cursor,
        context.offset_size,
        context.length_size,
        context.ndims,
    )?;

    if node.level == 0 {
        // Leaf node — collect (key[i], child[i]) pairs.
        for (i, child_addr) in node.children.iter().enumerate() {
            let include = match &node.keys[i] {
                BTreeV1Key::RawData { offsets, .. } => {
                    context.chunk_bounds.map_or(true, |(first, last)| {
                        offsets
                            .iter()
                            .zip(context.chunk_dims.iter())
                            .enumerate()
                            .all(|(dim, (offset, chunk_dim))| {
                                let chunk_index = *offset / u64::from(*chunk_dim);
                                chunk_index >= first[dim] && chunk_index <= last[dim]
                            })
                    })
                }
                _ => true,
            };

            if include {
                results.push((node.keys[i].clone(), *child_addr));
            }
        }
    } else {
        // Internal node — recurse into each child.
        for child_addr in &node.children {
            collect_recursive(context, *child_addr, depth_remaining - 1, visited, results)?;
        }
    }

    Ok(())
}

fn collect_recursive_storage(
    context: BTreeV1CollectContextStorage<'_>,
    address: u64,
    depth_remaining: usize,
    visited: &mut HashSet<u64>,
    results: &mut Vec<(BTreeV1Key, u64)>,
) -> Result<()> {
    if Cursor::is_undefined_offset(address, context.offset_size) {
        return Ok(());
    }
    if depth_remaining == 0 {
        return Err(Error::InvalidData(
            "B-tree v1 traversal exceeded recursion limit".into(),
        ));
    }
    if !visited.insert(address) {
        return Err(Error::InvalidData(format!(
            "B-tree v1 traversal revisits node at offset {address:#x}"
        )));
    }

    let header_len = 4 + 1 + 1 + 2 + 2 * usize::from(context.offset_size);
    let header = context.storage.read_range(address, header_len)?;
    let mut header_cursor = Cursor::new(header.as_ref());
    let sig = header_cursor.read_bytes(4)?;
    if sig != BTREE_V1_SIGNATURE {
        return Err(Error::InvalidBTreeSignature);
    }

    let node_type = header_cursor.read_u8()?;
    let _level = header_cursor.read_u8()?;
    let entries_used = header_cursor.read_u16_le()?;
    let key_size = match node_type {
        0 => usize::from(context.length_size),
        1 => {
            let ndims = context.ndims.ok_or_else(|| {
                Error::InvalidData("ndims required for raw data chunk B-tree keys".into())
            })?;
            // chunk_size (4) + filter_mask (4), then (ndims + 1) dimension
            // offsets at a fixed 8 bytes each — NOT size_of_offsets (HDF5 spec).
            8 + (usize::try_from(ndims).map_err(|_| {
                Error::InvalidData("B-tree v1 ndims exceeds platform usize capacity".into())
            })? + 1)
                * 8
        }
        _ => {
            return Err(Error::InvalidData(format!(
                "unknown v1 B-tree node type: {node_type}"
            )));
        }
    };
    let node_len = header_len
        + (usize::from(entries_used) + 1) * key_size
        + usize::from(entries_used) * usize::from(context.offset_size);
    let node_bytes = context.storage.read_range(address, node_len)?;
    let mut cursor = Cursor::new(node_bytes.as_ref());
    let node = BTreeV1Node::parse(
        &mut cursor,
        context.offset_size,
        context.length_size,
        context.ndims,
    )?;

    if node.level == 0 {
        for (i, child_addr) in node.children.iter().enumerate() {
            let include = match &node.keys[i] {
                BTreeV1Key::RawData { offsets, .. } => {
                    context.chunk_bounds.map_or(true, |(first, last)| {
                        offsets
                            .iter()
                            .zip(context.chunk_dims.iter())
                            .enumerate()
                            .all(|(dim, (offset, chunk_dim))| {
                                let chunk_index = *offset / u64::from(*chunk_dim);
                                chunk_index >= first[dim] && chunk_index <= last[dim]
                            })
                    })
                }
                _ => true,
            };

            if include {
                results.push((node.keys[i].clone(), *child_addr));
            }
        }
    } else {
        for child_addr in &node.children {
            collect_recursive_storage(context, *child_addr, depth_remaining - 1, visited, results)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a v1 B-tree node (type 0, group) from the given parameters.
    #[allow(clippy::too_many_arguments)]
    fn build_group_node(
        level: u8,
        entries_used: u16,
        left_sibling: u64,
        right_sibling: u64,
        keys: &[u64],     // local heap offsets
        children: &[u64], // child addresses
        offset_size: u8,
        length_size: u8,
    ) -> Vec<u8> {
        assert_eq!(keys.len(), entries_used as usize + 1);
        assert_eq!(children.len(), entries_used as usize);

        let mut buf = Vec::new();
        buf.extend_from_slice(b"TREE");
        buf.push(0); // type 0 = group
        buf.push(level);
        buf.extend_from_slice(&entries_used.to_le_bytes());
        push_offset(&mut buf, left_sibling, offset_size);
        push_offset(&mut buf, right_sibling, offset_size);

        for i in 0..keys.len() {
            push_length(&mut buf, keys[i], length_size);
            if i < children.len() {
                push_offset(&mut buf, children[i], offset_size);
            }
        }

        buf
    }

    /// Build a v1 B-tree node (type 1, raw data) from the given parameters.
    fn build_rawdata_node(
        level: u8,
        entries_used: u16,
        left_sibling: u64,
        right_sibling: u64,
        keys: &[(u32, u32, Vec<u64>)], // (chunk_size, filter_mask, offsets)
        children: &[u64],
        offset_size: u8,
    ) -> Vec<u8> {
        assert_eq!(keys.len(), entries_used as usize + 1);
        assert_eq!(children.len(), entries_used as usize);

        let mut buf = Vec::new();
        buf.extend_from_slice(b"TREE");
        buf.push(1); // type 1 = raw data
        buf.push(level);
        buf.extend_from_slice(&entries_used.to_le_bytes());
        push_offset(&mut buf, left_sibling, offset_size);
        push_offset(&mut buf, right_sibling, offset_size);

        for i in 0..keys.len() {
            let (cs, fm, ref offs) = keys[i];
            buf.extend_from_slice(&cs.to_le_bytes());
            buf.extend_from_slice(&fm.to_le_bytes());
            for &o in offs {
                // Chunk-key dimension offsets are ALWAYS 8 bytes, independent of
                // the superblock's size_of_offsets (HDF5 format spec). Child
                // pointers below remain offset_size-byte file addresses.
                buf.extend_from_slice(&o.to_le_bytes());
            }
            if i < children.len() {
                push_offset(&mut buf, children[i], offset_size);
            }
        }

        buf
    }

    fn push_offset(buf: &mut Vec<u8>, val: u64, size: u8) {
        match size {
            4 => buf.extend_from_slice(&(val as u32).to_le_bytes()),
            8 => buf.extend_from_slice(&val.to_le_bytes()),
            _ => panic!("unsupported offset size in test"),
        }
    }

    fn push_length(buf: &mut Vec<u8>, val: u64, size: u8) {
        match size {
            4 => buf.extend_from_slice(&(val as u32).to_le_bytes()),
            8 => buf.extend_from_slice(&val.to_le_bytes()),
            _ => panic!("unsupported length size in test"),
        }
    }

    #[test]
    fn parse_group_leaf_node() {
        let undef8 = 0xFFFF_FFFF_FFFF_FFFFu64;
        let data = build_group_node(
            0,                 // leaf
            2,                 // 2 entries
            undef8,            // no left sibling
            undef8,            // no right sibling
            &[0, 8, 16],       // 3 keys (offsets into local heap)
            &[0x1000, 0x2000], // 2 children (SNOD addresses)
            8,
            8,
        );

        let mut cursor = Cursor::new(&data);
        let node = BTreeV1Node::parse(&mut cursor, 8, 8, None).unwrap();

        assert_eq!(node.node_type, 0);
        assert_eq!(node.level, 0);
        assert_eq!(node.entries_used, 2);
        assert!(Cursor::is_undefined_offset(node.left_sibling, 8));
        assert!(Cursor::is_undefined_offset(node.right_sibling, 8));
        assert_eq!(node.keys.len(), 3);
        assert_eq!(node.children.len(), 2);

        match &node.keys[0] {
            BTreeV1Key::Group { local_heap_offset } => assert_eq!(*local_heap_offset, 0),
            _ => panic!("expected Group key"),
        }
        match &node.keys[1] {
            BTreeV1Key::Group { local_heap_offset } => assert_eq!(*local_heap_offset, 8),
            _ => panic!("expected Group key"),
        }
        assert_eq!(node.children[0], 0x1000);
        assert_eq!(node.children[1], 0x2000);
    }

    #[test]
    fn parse_rawdata_leaf_node() {
        let undef8 = 0xFFFF_FFFF_FFFF_FFFFu64;
        // 2D dataset, ndims=2, so keys have 3 offsets each (ndims+1)
        let data = build_rawdata_node(
            0, // leaf
            1, // 1 entry
            undef8,
            undef8,
            &[
                (1024, 0, vec![0, 0, 0]),   // key[0]: chunk at origin
                (1024, 0, vec![10, 20, 0]), // key[1]: next chunk boundary
            ],
            &[0x5000], // 1 child = chunk data address
            8,
        );

        let mut cursor = Cursor::new(&data);
        let node = BTreeV1Node::parse(&mut cursor, 8, 8, Some(2)).unwrap();

        assert_eq!(node.node_type, 1);
        assert_eq!(node.level, 0);
        assert_eq!(node.entries_used, 1);
        assert_eq!(node.keys.len(), 2);
        assert_eq!(node.children.len(), 1);

        match &node.keys[0] {
            BTreeV1Key::RawData {
                chunk_size,
                filter_mask,
                offsets,
            } => {
                assert_eq!(*chunk_size, 1024);
                assert_eq!(*filter_mask, 0);
                assert_eq!(offsets, &[0, 0, 0]);
            }
            _ => panic!("expected RawData key"),
        }

        assert_eq!(node.children[0], 0x5000);
    }

    /// Regression test for 32-bit-superblock files (`size_of_offsets = 4`):
    /// chunk-key dimension offsets are fixed 8-byte values, so the child chunk
    /// address (a 4-byte file offset here) must be read at the correct cursor
    /// position. Reading the dimension offsets as 4 bytes (the old bug)
    /// misaligns the cursor and yields a garbage child address.
    #[test]
    fn parse_rawdata_leaf_4byte() {
        let undef4 = 0xFFFF_FFFFu64;
        let data = build_rawdata_node(
            0,
            1,
            undef4,
            undef4,
            &[
                (512, 0x03, vec![0, 0]), // ndims=1, key has 2 offsets (each 8 bytes)
                (512, 0, vec![100, 0]),
            ],
            &[0x3000],
            4,
        );

        let mut cursor = Cursor::new(&data);
        let node = BTreeV1Node::parse(&mut cursor, 4, 4, Some(1)).unwrap();

        assert_eq!(node.node_type, 1);
        match &node.keys[0] {
            BTreeV1Key::RawData {
                chunk_size,
                filter_mask,
                offsets,
            } => {
                assert_eq!(*chunk_size, 512);
                assert_eq!(*filter_mask, 0x03);
                assert_eq!(offsets, &[0, 0]);
            }
            _ => panic!("expected RawData key"),
        }
        // The load-bearing assertion: the child chunk address is located only
        // when the 8-byte dimension offsets are parsed at full width.
        assert_eq!(node.children[0], 0x3000);
    }

    #[test]
    fn bad_signature() {
        let mut data = build_group_node(0, 0, u64::MAX, u64::MAX, &[0], &[], 8, 8);
        data[0] = b'X'; // corrupt
        let mut cursor = Cursor::new(&data);
        assert!(matches!(
            BTreeV1Node::parse(&mut cursor, 8, 8, None),
            Err(Error::InvalidBTreeSignature)
        ));
    }

    #[test]
    fn collect_leaves_single_leaf() {
        let undef8 = u64::MAX;
        // Put the leaf node at offset 0 in our fake file data.
        let node_data = build_group_node(0, 2, undef8, undef8, &[0, 5, 10], &[0x100, 0x200], 8, 8);

        let results = collect_btree_v1_leaves(&node_data, 0, 8, 8, None, &[], None).unwrap();

        assert_eq!(results.len(), 2);
        match &results[0].0 {
            BTreeV1Key::Group { local_heap_offset } => assert_eq!(*local_heap_offset, 0),
            _ => panic!("expected Group key"),
        }
        assert_eq!(results[0].1, 0x100);
        assert_eq!(results[1].1, 0x200);
    }

    #[test]
    fn collect_leaves_two_level_tree() {
        let undef8 = u64::MAX;

        // Build a two-level tree:
        //   Root (level 1) at offset 0, with 2 children
        //     Leaf A at offset 1000
        //     Leaf B at offset 2000

        let leaf_a = build_group_node(0, 1, undef8, undef8, &[0, 5], &[0xA00], 8, 8);

        let leaf_b = build_group_node(0, 1, undef8, undef8, &[10, 15], &[0xB00], 8, 8);

        let root = build_group_node(
            1,
            2,
            undef8,
            undef8,
            &[0, 5, 15],   // 3 keys for 2 children
            &[1000, 2000], // children at offsets 1000 and 2000
            8,
            8,
        );

        // Assemble into a single buffer: root at 0, leaf_a at 1000, leaf_b at 2000.
        let total_size = 3000 + leaf_b.len();
        let mut file_data = vec![0u8; total_size];
        file_data[..root.len()].copy_from_slice(&root);
        file_data[1000..1000 + leaf_a.len()].copy_from_slice(&leaf_a);
        file_data[2000..2000 + leaf_b.len()].copy_from_slice(&leaf_b);

        let results = collect_btree_v1_leaves(&file_data, 0, 8, 8, None, &[], None).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].1, 0xA00);
        assert_eq!(results[1].1, 0xB00);
    }

    #[test]
    fn collect_rejects_revisited_node() {
        let undef8 = u64::MAX;
        let root = build_group_node(
            1,
            1,
            undef8,
            undef8,
            &[0, 8],
            &[0], // corrupt: child points back to the root node
            8,
            8,
        );

        let err = collect_btree_v1_leaves(&root, 0, 8, 8, None, &[], None).unwrap_err();
        assert!(matches!(err, Error::InvalidData(message) if message.contains("revisits node")));

        let storage = crate::storage::BytesStorage::new(root);
        let err = collect_btree_v1_leaves_storage(&storage, 0, 8, 8, None, &[], None).unwrap_err();
        assert!(matches!(err, Error::InvalidData(message) if message.contains("revisits node")));
    }

    #[test]
    fn collect_undefined_root() {
        // Undefined root address should produce empty results.
        let data = vec![0u8; 100];
        let results = collect_btree_v1_leaves(&data, u64::MAX, 8, 8, None, &[], None).unwrap();
        assert!(results.is_empty());
    }
}
