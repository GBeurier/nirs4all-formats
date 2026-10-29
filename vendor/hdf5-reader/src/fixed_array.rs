//! HDF5 Fixed Array (FA) chunk index.
//!
//! This is the default chunk index for fixed-size chunked datasets created with
//! `libver='latest'`. It stores chunk entries in a flat array (optionally paged)
//! with a single-level header → data block structure.
//!
//! Structures:
//! - `FAHD` — Fixed Array Header
//! - `FADB` — Fixed Array Data Block

use crate::checksum::jenkins_lookup3;
use crate::chunk_index::ChunkEntry;
use crate::error::{Error, Result};
use crate::io::Cursor;
use crate::storage::Storage;

const FAHD_SIGNATURE: [u8; 4] = *b"FAHD";
const FADB_SIGNATURE: [u8; 4] = *b"FADB";

/// Parsed Fixed Array Header.
#[derive(Debug)]
struct FaHeader {
    client_id: u8,
    entry_size: u8,
    page_bits: u8,
    num_entries: u64,
    data_block_address: u64,
}

/// Parse the Fixed Array Header at the given address.
///
/// On-disk layout (from H5FA_HEADER_SIZE):
/// sig(4) + ver(1) + client_id(1) + entry_size(1) + page_bits(1)
/// + nelmts(length_size) + dblk_addr(offset_size) + checksum(4)
fn parse_header(data: &[u8], address: u64, offset_size: u8, length_size: u8) -> Result<FaHeader> {
    let mut cursor = Cursor::new(data);
    cursor.set_position(address);

    let sig = cursor.read_bytes(4)?;
    if sig != FAHD_SIGNATURE {
        return Err(Error::InvalidFixedArraySignature {
            context: "header signature mismatch",
        });
    }

    let version = cursor.read_u8()?;
    if version != 0 {
        return Err(Error::Other(format!(
            "unsupported fixed array header version {}",
            version
        )));
    }

    let client_id = cursor.read_u8()?;
    let entry_size = cursor.read_u8()?;
    let page_bits = cursor.read_u8()?;
    let num_entries = cursor.read_length(length_size)?;
    let data_block_address = cursor.read_offset(offset_size)?;

    // Checksum covers everything from signature through data_block_address.
    let header_end = cursor.position();
    let header_bytes = &data[address as usize..header_end as usize];
    let stored_checksum = cursor.read_u32_le()?;
    let computed = jenkins_lookup3(header_bytes);
    if stored_checksum != computed {
        return Err(Error::ChecksumMismatch {
            expected: stored_checksum,
            actual: computed,
        });
    }

    Ok(FaHeader {
        client_id,
        entry_size,
        page_bits,
        num_entries,
        data_block_address,
    })
}

fn parse_header_storage(
    storage: &dyn Storage,
    address: u64,
    offset_size: u8,
    length_size: u8,
) -> Result<FaHeader> {
    let header_len = 4 + 1 + 1 + 1 + 1 + usize::from(length_size) + usize::from(offset_size) + 4;
    let bytes = storage.read_range(address, header_len)?;
    parse_header(bytes.as_ref(), 0, offset_size, length_size)
}

/// A single raw fixed-array entry (before conversion to ChunkEntry).
#[derive(Debug)]
struct FaRawEntry {
    address: u64,
    chunk_size: u64,
    filter_mask: u32,
}

/// Read entries from a Fixed Array Data Block.
fn parse_data_block(
    data: &[u8],
    address: u64,
    header: &FaHeader,
    offset_size: u8,
) -> Result<Vec<FaRawEntry>> {
    let mut cursor = Cursor::new(data);
    cursor.set_position(address);

    let sig = cursor.read_bytes(4)?;
    if sig != FADB_SIGNATURE {
        return Err(Error::InvalidFixedArraySignature {
            context: "data block signature mismatch",
        });
    }

    let version = cursor.read_u8()?;
    if version != 0 {
        return Err(Error::Other(format!(
            "unsupported fixed array data block version {}",
            version
        )));
    }

    let _client_id = cursor.read_u8()?;
    let _header_address = cursor.read_offset(offset_size)?;

    let num_entries = header.num_entries as usize;
    let is_filtered = header.client_id == 1;

    // Paging is used only when nelmts exceeds 2^page_bits.
    let use_paging = header.page_bits > 0 && num_entries > (1usize << header.page_bits);

    if !use_paging {
        // Non-paged: all entries inline followed by a single checksum.
        let entries = read_entries(
            &mut cursor,
            num_entries,
            is_filtered,
            offset_size,
            header.entry_size,
        )?;
        // Skip the trailing checksum (already verified structurally).
        let _checksum = cursor.read_u32_le()?;
        Ok(entries)
    } else {
        // Paged: entries are split into pages of `2^page_bits` entries each.
        let entries_per_page = 1usize << header.page_bits;
        let num_pages = num_entries.div_ceil(entries_per_page);

        // Page init bitmap: ceil(num_pages / 8) bytes — tells which pages
        // have been initialized. We read all pages regardless (uninitialized
        // pages have undefined addresses that we filter out later).
        let bitmap_bytes = num_pages.div_ceil(8);
        let page_bitmap = cursor.read_bytes(bitmap_bytes)?.to_vec();

        let mut all_entries = Vec::with_capacity(num_entries);

        for page_idx in 0..num_pages {
            let byte_idx = page_idx / 8;
            let bit_idx = page_idx % 8;
            let page_initialized =
                byte_idx < page_bitmap.len() && (page_bitmap[byte_idx] & (1 << bit_idx)) != 0;

            let entries_in_this_page = if page_idx == num_pages - 1 {
                let remainder = num_entries % entries_per_page;
                if remainder == 0 {
                    entries_per_page
                } else {
                    remainder
                }
            } else {
                entries_per_page
            };

            if page_initialized {
                let page_entries = read_entries(
                    &mut cursor,
                    entries_in_this_page,
                    is_filtered,
                    offset_size,
                    header.entry_size,
                )?;
                // Each page has its own checksum.
                let _page_checksum = cursor.read_u32_le()?;
                all_entries.extend(page_entries);
            } else {
                // Uninitialized page — fill with undefined entries.
                for _ in 0..entries_in_this_page {
                    all_entries.push(FaRawEntry {
                        address: u64::MAX,
                        chunk_size: 0,
                        filter_mask: 0,
                    });
                }
            }
        }

        Ok(all_entries)
    }
}

/// Read `count` entries from the cursor.
fn read_entries(
    cursor: &mut Cursor<'_>,
    count: usize,
    is_filtered: bool,
    offset_size: u8,
    entry_size: u8,
) -> Result<Vec<FaRawEntry>> {
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let address = cursor.read_offset(offset_size)?;
        let (chunk_size, filter_mask) = if is_filtered {
            let chunk_size_len = entry_size
                .checked_sub(offset_size)
                .and_then(|remaining| remaining.checked_sub(4))
                .ok_or_else(|| Error::InvalidData("invalid fixed array entry size".into()))?;
            // 1–8 bytes wide (sized to the chunk), not a "length" of the superblock size.
            let cs = cursor.read_uvar(chunk_size_len as usize)?;
            let fm = cursor.read_u32_le()?;
            (cs, fm)
        } else {
            (0, 0)
        };
        entries.push(FaRawEntry {
            address,
            chunk_size,
            filter_mask,
        });
    }
    Ok(entries)
}

fn read_entry_at(
    data: &[u8],
    position: u64,
    is_filtered: bool,
    offset_size: u8,
    entry_size: u8,
) -> Result<FaRawEntry> {
    let mut cursor = Cursor::new(data);
    cursor.set_position(position);
    let mut entries = read_entries(&mut cursor, 1, is_filtered, offset_size, entry_size)?;
    entries
        .pop()
        .ok_or_else(|| Error::InvalidData("missing fixed array entry".into()))
}

fn read_entry_at_storage(
    storage: &dyn Storage,
    position: u64,
    is_filtered: bool,
    offset_size: u8,
    entry_size: u8,
) -> Result<FaRawEntry> {
    let bytes = storage.read_range(position, usize::from(entry_size))?;
    let mut cursor = Cursor::new(bytes.as_ref());
    let mut entries = read_entries(&mut cursor, 1, is_filtered, offset_size, entry_size)?;
    entries
        .pop()
        .ok_or_else(|| Error::InvalidData("missing fixed array entry".into()))
}

fn linear_target_offsets(
    dataset_shape: &[u64],
    chunk_dims: &[u32],
    chunk_bounds: Option<(&[u64], &[u64])>,
) -> Vec<(usize, Vec<u64>)> {
    let ndim = dataset_shape.len();
    let chunks_per_dim: Vec<u64> = (0..ndim)
        .map(|i| dataset_shape[i].div_ceil(chunk_dims[i] as u64))
        .collect();

    if ndim == 0 {
        return vec![(0, Vec::new())];
    }

    let (first_chunk, last_chunk): (Vec<u64>, Vec<u64>) = match chunk_bounds {
        Some((first, last)) => (first.to_vec(), last.to_vec()),
        None => (
            vec![0u64; ndim],
            chunks_per_dim
                .iter()
                .map(|count| count.saturating_sub(1))
                .collect(),
        ),
    };

    let mut targets = Vec::new();
    let mut chunk_indices = first_chunk.clone();
    loop {
        let mut linear_idx = 0u64;
        for (dim, chunk_index) in chunk_indices.iter().enumerate() {
            linear_idx = linear_idx * chunks_per_dim[dim] + chunk_index;
        }
        let offsets = chunk_indices
            .iter()
            .enumerate()
            .map(|(dim, chunk_index)| chunk_index * u64::from(chunk_dims[dim]))
            .collect();
        targets.push((linear_idx as usize, offsets));

        let mut advanced = false;
        for dim in (0..ndim).rev() {
            if chunk_indices[dim] < last_chunk[dim] {
                chunk_indices[dim] += 1;
                if dim + 1 < ndim {
                    chunk_indices[(dim + 1)..ndim].copy_from_slice(&first_chunk[(dim + 1)..ndim]);
                }
                advanced = true;
                break;
            }
        }

        if !advanced {
            break;
        }
    }

    targets
}

fn collect_fixed_array_chunk_entries_bounded(
    data: &[u8],
    header: &FaHeader,
    offset_size: u8,
    dataset_shape: &[u64],
    chunk_dims: &[u32],
    chunk_bounds: (&[u64], &[u64]),
) -> Result<Vec<ChunkEntry>> {
    let targets = linear_target_offsets(dataset_shape, chunk_dims, Some(chunk_bounds));
    let mut cursor = Cursor::new(data);
    cursor.set_position(header.data_block_address);

    let sig = cursor.read_bytes(4)?;
    if sig != FADB_SIGNATURE {
        return Err(Error::InvalidFixedArraySignature {
            context: "data block signature mismatch",
        });
    }

    let version = cursor.read_u8()?;
    if version != 0 {
        return Err(Error::Other(format!(
            "unsupported fixed array data block version {}",
            version
        )));
    }

    let _client_id = cursor.read_u8()?;
    let _header_address = cursor.read_offset(offset_size)?;

    let num_entries = header.num_entries as usize;
    let is_filtered = header.client_id == 1;
    let entry_bytes = header.entry_size as usize;
    let use_paging = header.page_bits > 0 && num_entries > (1usize << header.page_bits);

    if !use_paging {
        let entries_start = cursor.position();
        let mut entries = Vec::new();
        for (linear_idx, offsets) in targets {
            let position = entries_start + (linear_idx * entry_bytes) as u64;
            let raw = read_entry_at(data, position, is_filtered, offset_size, header.entry_size)?;
            if Cursor::is_undefined_offset(raw.address, offset_size) {
                continue;
            }
            entries.push(ChunkEntry {
                address: raw.address,
                size: raw.chunk_size,
                filter_mask: raw.filter_mask,
                offsets,
            });
        }
        return Ok(entries);
    }

    let entries_per_page = 1usize << header.page_bits;
    let num_pages = num_entries.div_ceil(entries_per_page);
    let bitmap_bytes = num_pages.div_ceil(8);
    let page_bitmap = cursor.read_bytes(bitmap_bytes)?.to_vec();
    let pages_start = cursor.position();

    let mut page_offsets = vec![None; num_pages];
    let mut next_page_start = pages_start;
    for (page_idx, page_offset) in page_offsets.iter_mut().enumerate().take(num_pages) {
        let byte_idx = page_idx / 8;
        let bit_idx = page_idx % 8;
        let page_initialized =
            byte_idx < page_bitmap.len() && (page_bitmap[byte_idx] & (1 << bit_idx)) != 0;

        let entries_in_page = if page_idx == num_pages - 1 {
            let remainder = num_entries % entries_per_page;
            if remainder == 0 {
                entries_per_page
            } else {
                remainder
            }
        } else {
            entries_per_page
        };

        if page_initialized {
            *page_offset = Some(next_page_start);
            next_page_start += (entries_in_page * entry_bytes + 4) as u64;
        }
    }

    let mut entries = Vec::new();
    for (linear_idx, offsets) in targets {
        let page_idx = linear_idx / entries_per_page;
        let within_page = linear_idx % entries_per_page;
        let Some(page_start) = page_offsets[page_idx] else {
            continue;
        };
        let position = page_start + (within_page * entry_bytes) as u64;
        let raw = read_entry_at(data, position, is_filtered, offset_size, header.entry_size)?;
        if Cursor::is_undefined_offset(raw.address, offset_size) {
            continue;
        }
        entries.push(ChunkEntry {
            address: raw.address,
            size: raw.chunk_size,
            filter_mask: raw.filter_mask,
            offsets,
        });
    }

    Ok(entries)
}

fn collect_fixed_array_chunk_entries_bounded_storage(
    storage: &dyn Storage,
    header: &FaHeader,
    offset_size: u8,
    dataset_shape: &[u64],
    chunk_dims: &[u32],
    chunk_bounds: (&[u64], &[u64]),
) -> Result<Vec<ChunkEntry>> {
    let targets = linear_target_offsets(dataset_shape, chunk_dims, Some(chunk_bounds));
    let block_header_len = 4 + 1 + 1 + usize::from(offset_size);
    let header_bytes = storage.read_range(header.data_block_address, block_header_len)?;
    let mut cursor = Cursor::new(header_bytes.as_ref());

    let sig = cursor.read_bytes(4)?;
    if sig != FADB_SIGNATURE {
        return Err(Error::InvalidFixedArraySignature {
            context: "data block signature mismatch",
        });
    }

    let version = cursor.read_u8()?;
    if version != 0 {
        return Err(Error::Other(format!(
            "unsupported fixed array data block version {}",
            version
        )));
    }

    let _client_id = cursor.read_u8()?;
    let _header_address = cursor.read_offset(offset_size)?;

    let num_entries = usize::try_from(header.num_entries).map_err(|_| {
        Error::InvalidData("fixed array entry count exceeds platform usize capacity".into())
    })?;
    let is_filtered = header.client_id == 1;
    let entry_bytes = usize::from(header.entry_size);
    let use_paging = header.page_bits > 0 && num_entries > (1usize << header.page_bits);
    let entries_start = header.data_block_address
        + u64::try_from(block_header_len)
            .map_err(|_| Error::OffsetOutOfBounds(header.data_block_address))?;

    if !use_paging {
        let mut entries = Vec::new();
        for (linear_idx, offsets) in targets {
            let position = entries_start
                + u64::try_from(linear_idx * entry_bytes).map_err(|_| {
                    Error::InvalidData("fixed array entry offset exceeds u64 capacity".into())
                })?;
            let raw = read_entry_at_storage(
                storage,
                position,
                is_filtered,
                offset_size,
                header.entry_size,
            )?;
            if Cursor::is_undefined_offset(raw.address, offset_size) {
                continue;
            }
            entries.push(ChunkEntry {
                address: raw.address,
                size: raw.chunk_size,
                filter_mask: raw.filter_mask,
                offsets,
            });
        }
        return Ok(entries);
    }

    let entries_per_page = 1usize << header.page_bits;
    let num_pages = num_entries.div_ceil(entries_per_page);
    let bitmap_bytes = num_pages.div_ceil(8);
    let page_bitmap = storage.read_range(entries_start, bitmap_bytes)?;
    let pages_start = entries_start
        + u64::try_from(bitmap_bytes).map_err(|_| {
            Error::InvalidData("fixed array bitmap size exceeds u64 capacity".into())
        })?;

    let mut page_offsets = vec![None; num_pages];
    let mut next_page_start = pages_start;
    for (page_idx, page_offset) in page_offsets.iter_mut().enumerate().take(num_pages) {
        let byte_idx = page_idx / 8;
        let bit_idx = page_idx % 8;
        let page_initialized =
            byte_idx < page_bitmap.len() && (page_bitmap[byte_idx] & (1 << bit_idx)) != 0;

        let entries_in_page = if page_idx == num_pages - 1 {
            let remainder = num_entries % entries_per_page;
            if remainder == 0 {
                entries_per_page
            } else {
                remainder
            }
        } else {
            entries_per_page
        };

        if page_initialized {
            *page_offset = Some(next_page_start);
            next_page_start += u64::try_from(entries_in_page * entry_bytes + 4).map_err(|_| {
                Error::InvalidData("fixed array page size exceeds u64 capacity".into())
            })?;
        }
    }

    let mut entries = Vec::new();
    for (linear_idx, offsets) in targets {
        let page_idx = linear_idx / entries_per_page;
        let within_page = linear_idx % entries_per_page;
        let Some(page_start) = page_offsets[page_idx] else {
            continue;
        };
        let position = page_start
            + u64::try_from(within_page * entry_bytes).map_err(|_| {
                Error::InvalidData("fixed array page entry offset exceeds u64 capacity".into())
            })?;
        let raw = read_entry_at_storage(
            storage,
            position,
            is_filtered,
            offset_size,
            header.entry_size,
        )?;
        if Cursor::is_undefined_offset(raw.address, offset_size) {
            continue;
        }
        entries.push(ChunkEntry {
            address: raw.address,
            size: raw.chunk_size,
            filter_mask: raw.filter_mask,
            offsets,
        });
    }

    Ok(entries)
}

/// Collect chunk entries from a Fixed Array index.
///
/// Reads the FAHD header and FADB data block, then converts linear entry
/// indices to multi-dimensional chunk offsets.
pub fn collect_fixed_array_chunk_entries(
    data: &[u8],
    header_address: u64,
    offset_size: u8,
    length_size: u8,
    dataset_shape: &[u64],
    chunk_dims: &[u32],
    chunk_bounds: Option<(&[u64], &[u64])>,
) -> Result<Vec<ChunkEntry>> {
    let header = parse_header(data, header_address, offset_size, length_size)?;

    if Cursor::is_undefined_offset(header.data_block_address, offset_size) {
        return Ok(Vec::new());
    }

    if let Some(bounds) = chunk_bounds {
        return collect_fixed_array_chunk_entries_bounded(
            data,
            &header,
            offset_size,
            dataset_shape,
            chunk_dims,
            bounds,
        );
    }

    let raw_entries = parse_data_block(data, header.data_block_address, &header, offset_size)?;

    let ndim = dataset_shape.len();
    let chunks_per_dim: Vec<u64> = (0..ndim)
        .map(|i| dataset_shape[i].div_ceil(chunk_dims[i] as u64))
        .collect();

    let mut entries = Vec::new();
    for (linear_idx, raw) in raw_entries.iter().enumerate() {
        // Skip undefined addresses (unallocated chunks).
        if Cursor::is_undefined_offset(raw.address, offset_size) {
            continue;
        }

        // Convert linear index to multi-dimensional chunk offsets.
        let mut remaining = linear_idx as u64;
        let mut offsets = vec![0u64; ndim];
        for d in (0..ndim).rev() {
            offsets[d] = (remaining % chunks_per_dim[d]) * chunk_dims[d] as u64;
            remaining /= chunks_per_dim[d];
        }

        if let Some((first_chunk, last_chunk)) = chunk_bounds {
            let overlaps = offsets.iter().enumerate().all(|(dim, offset)| {
                let chunk_index = *offset / u64::from(chunk_dims[dim]);
                chunk_index >= first_chunk[dim] && chunk_index <= last_chunk[dim]
            });
            if !overlaps {
                continue;
            }
        }

        entries.push(ChunkEntry {
            address: raw.address,
            size: raw.chunk_size,
            filter_mask: raw.filter_mask,
            offsets,
        });
    }

    Ok(entries)
}

/// Collect chunk entries from a Fixed Array index using random-access storage.
pub fn collect_fixed_array_chunk_entries_storage(
    storage: &dyn Storage,
    header_address: u64,
    offset_size: u8,
    length_size: u8,
    dataset_shape: &[u64],
    chunk_dims: &[u32],
    chunk_bounds: Option<(&[u64], &[u64])>,
) -> Result<Vec<ChunkEntry>> {
    let header = parse_header_storage(storage, header_address, offset_size, length_size)?;

    if Cursor::is_undefined_offset(header.data_block_address, offset_size) {
        return Ok(Vec::new());
    }

    if let Some(bounds) = chunk_bounds {
        return collect_fixed_array_chunk_entries_bounded_storage(
            storage,
            &header,
            offset_size,
            dataset_shape,
            chunk_dims,
            bounds,
        );
    }

    let num_entries = usize::try_from(header.num_entries).map_err(|_| {
        Error::InvalidData("fixed array entry count exceeds platform usize capacity".into())
    })?;
    let is_filtered = header.client_id == 1;
    let header_len = 4 + 1 + 1 + usize::from(offset_size);
    let use_paging = header.page_bits > 0 && num_entries > (1usize << header.page_bits);
    let block_len = if !use_paging {
        header_len + num_entries * usize::from(header.entry_size) + 4
    } else {
        let entries_per_page = 1usize << header.page_bits;
        let num_pages = num_entries.div_ceil(entries_per_page);
        let bitmap_bytes = num_pages.div_ceil(8);
        let mut len = header_len + bitmap_bytes;
        for page_idx in 0..num_pages {
            let entries_in_page = if page_idx == num_pages - 1 {
                let remainder = num_entries % entries_per_page;
                if remainder == 0 {
                    entries_per_page
                } else {
                    remainder
                }
            } else {
                entries_per_page
            };
            len += entries_in_page * usize::from(header.entry_size) + 4;
        }
        len
    };
    let block = storage.read_range(header.data_block_address, block_len)?;
    let raw_entries = parse_data_block(block.as_ref(), 0, &header, offset_size)?;

    let ndim = dataset_shape.len();
    let chunks_per_dim: Vec<u64> = (0..ndim)
        .map(|i| dataset_shape[i].div_ceil(chunk_dims[i] as u64))
        .collect();

    let mut entries = Vec::new();
    for (linear_idx, raw) in raw_entries.iter().enumerate() {
        if Cursor::is_undefined_offset(raw.address, offset_size) {
            continue;
        }
        let mut remaining = linear_idx as u64;
        let mut offsets = vec![0u64; ndim];
        for d in (0..ndim).rev() {
            offsets[d] = (remaining % chunks_per_dim[d]) * chunk_dims[d] as u64;
            remaining /= chunks_per_dim[d];
        }

        if let Some((first_chunk, last_chunk)) = chunk_bounds {
            let overlaps = offsets.iter().enumerate().all(|(dim, offset)| {
                let chunk_index = *offset / u64::from(chunk_dims[dim]);
                chunk_index >= first_chunk[dim] && chunk_index <= last_chunk[dim]
            });
            if !overlaps {
                continue;
            }
        }

        entries.push(ChunkEntry {
            address: raw.address,
            size: if is_filtered { raw.chunk_size } else { 0 },
            filter_mask: raw.filter_mask,
            offsets,
        });
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fahd_bad_signature() {
        let mut data = vec![0u8; 64];
        data[0..4].copy_from_slice(b"XXXX");
        let err = parse_header(&data, 0, 8, 8).unwrap_err();
        assert!(matches!(err, Error::InvalidFixedArraySignature { .. }));
    }

    #[test]
    fn fadb_bad_signature() {
        let header = FaHeader {
            client_id: 0,
            entry_size: 8,
            page_bits: 0,
            num_entries: 1,
            data_block_address: 0,
        };
        let mut data = vec![0u8; 64];
        data[0..4].copy_from_slice(b"XXXX");
        let err = parse_data_block(&data, 0, &header, 8).unwrap_err();
        assert!(matches!(err, Error::InvalidFixedArraySignature { .. }));
    }
}
