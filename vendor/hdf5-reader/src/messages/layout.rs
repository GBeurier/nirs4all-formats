//! HDF5 Data Layout message (type 0x0008).
//!
//! Describes how raw data for a dataset is stored: compact (inline in the
//! object header), contiguous (a single block in the file), or chunked
//! (split into fixed-size chunks, indexed by a B-tree).

use crate::error::{Error, Result};
use crate::io::Cursor;
pub use hdf5_core::{ChunkIndexing, DataLayout, DataLayoutMessage};

/// Parse a data layout message.
pub fn parse(
    cursor: &mut Cursor<'_>,
    offset_size: u8,
    length_size: u8,
    msg_size: usize,
) -> Result<DataLayoutMessage> {
    let start = cursor.position();
    let version = cursor.read_u8()?;

    let layout = match version {
        1 | 2 => parse_v1_v2(cursor, offset_size, length_size, version)?,
        3 => parse_v3(cursor, offset_size, length_size)?,
        4 | 5 => parse_v4_v5(cursor, offset_size, length_size, version)?,
        v => return Err(Error::UnsupportedLayoutVersion(v)),
    };

    let consumed = (cursor.position() - start) as usize;
    if consumed < msg_size {
        cursor.skip(msg_size - consumed)?;
    }

    Ok(DataLayoutMessage { layout })
}

// ---------------------------------------------------------------------------
// Version 1 / 2
// ---------------------------------------------------------------------------

fn parse_v1_v2(
    cursor: &mut Cursor<'_>,
    offset_size: u8,
    _length_size: u8,
    version: u8,
) -> Result<DataLayout> {
    let dimensionality = cursor.read_u8()?;
    let layout_class = cursor.read_u8()?;
    let _reserved = cursor.read_bytes(if version == 1 { 5 } else { 3 })?;

    // For v1 there is an optional compact data size field.
    // data_address is only meaningful for contiguous and chunked.
    let data_address = if layout_class != 0 {
        cursor.read_offset(offset_size)?
    } else {
        // For compact, there is no address; skip the offset-sized field.
        cursor.read_offset(offset_size)?
    };

    // Read dimension sizes. Each is 4 bytes. The number of dimensions:
    // For contiguous: dimensionality values (unused data size).
    // For chunked: (dimensionality-1) chunk dims + 1 element size.
    let mut dim_values = Vec::with_capacity(dimensionality as usize);
    for _ in 0..dimensionality {
        dim_values.push(cursor.read_u32_le()?);
    }

    match layout_class {
        0 => {
            // Compact
            let compact_size = cursor.read_u32_le()? as usize;
            let data = cursor.read_bytes(compact_size)?.to_vec();
            Ok(DataLayout::Compact { data })
        }
        1 => {
            // Contiguous
            // Size is not explicitly stored in v1/v2 for contiguous. The dims
            // encode the logical size but the actual file extent comes from the
            // dataspace * element size. We store the product as size.
            let size = if dim_values.is_empty() {
                0
            } else {
                dim_values.iter().map(|d| *d as u64).product()
            };
            Ok(DataLayout::Contiguous {
                address: data_address,
                size,
            })
        }
        2 => {
            // Chunked — last dimension is the element size
            let (element_size, chunk_dims) = if dim_values.is_empty() {
                (0u32, vec![])
            } else {
                let es = *dim_values.last().unwrap();
                let cd: Vec<u32> = dim_values[..dim_values.len() - 1].to_vec();
                (es, cd)
            };
            Ok(DataLayout::Chunked {
                address: data_address,
                dims: chunk_dims,
                element_size,
                chunk_indexing: None,
            })
        }
        c => Err(Error::UnsupportedLayoutClass(c)),
    }
}

// ---------------------------------------------------------------------------
// Version 3
// ---------------------------------------------------------------------------

fn parse_v3(cursor: &mut Cursor<'_>, offset_size: u8, length_size: u8) -> Result<DataLayout> {
    let layout_class = cursor.read_u8()?;

    match layout_class {
        0 => {
            // Compact
            let size = cursor.read_u16_le()? as usize;
            let data = cursor.read_bytes(size)?.to_vec();
            Ok(DataLayout::Compact { data })
        }
        1 => {
            // Contiguous
            let address = cursor.read_offset(offset_size)?;
            let size = cursor.read_length(length_size)?;
            Ok(DataLayout::Contiguous { address, size })
        }
        2 => {
            // Chunked
            let dimensionality = cursor.read_u8()?;
            let address = cursor.read_offset(offset_size)?;

            // (dimensionality - 1) chunk dims + 1 element size (each 4 bytes)
            let n = dimensionality as usize;
            let mut raw_dims = Vec::with_capacity(n);
            for _ in 0..n {
                raw_dims.push(cursor.read_u32_le()?);
            }

            let (element_size, chunk_dims) = if raw_dims.is_empty() {
                (0, vec![])
            } else {
                let es = *raw_dims.last().unwrap();
                let cd = raw_dims[..raw_dims.len() - 1].to_vec();
                (es, cd)
            };

            Ok(DataLayout::Chunked {
                address,
                dims: chunk_dims,
                element_size,
                chunk_indexing: None,
            })
        }
        c => Err(Error::UnsupportedLayoutClass(c)),
    }
}

// ---------------------------------------------------------------------------
// Version 4
// ---------------------------------------------------------------------------

/// Parse v4/v5 layout messages.
///
/// v4/v5 chunked layouts store only the chunk dimensions here; element size
/// is derived from the datatype. Filtered chunk-index records switched from
/// `length_size` to `offset_size` in v5.
fn parse_v4_v5(
    cursor: &mut Cursor<'_>,
    offset_size: u8,
    length_size: u8,
    version: u8,
) -> Result<DataLayout> {
    let layout_class = cursor.read_u8()?;

    match layout_class {
        0 => {
            // Compact
            let size = cursor.read_u16_le()? as usize;
            let data = cursor.read_bytes(size)?.to_vec();
            Ok(DataLayout::Compact { data })
        }
        1 => {
            // Contiguous
            let address = cursor.read_offset(offset_size)?;
            let size = cursor.read_u64_le()?;
            Ok(DataLayout::Contiguous { address, size })
        }
        2 => {
            let start = cursor.clone();
            let direct = parse_v4_v5_chunked(cursor, offset_size, length_size, version, false);
            match direct {
                Ok(layout) => Ok(layout),
                Err(err) if version == 4 && should_retry_v4_chunked_parse(&err) => {
                    *cursor = start;
                    parse_v4_v5_chunked(cursor, offset_size, length_size, version, true)
                }
                Err(err) => Err(err),
            }
        }
        c => Err(Error::UnsupportedLayoutClass(c)),
    }
}

fn parse_v4_v5_chunked(
    cursor: &mut Cursor<'_>,
    offset_size: u8,
    length_size: u8,
    version: u8,
    legacy_dim_size_encoding: bool,
) -> Result<DataLayout> {
    let flags = cursor.read_u8()?;
    let ndims_raw = cursor.read_u8()? as usize;
    let dim_size_enc = cursor.read_u8()?;
    let dim_bytes = if legacy_dim_size_encoding {
        dim_size_enc as usize + 1
    } else {
        dim_size_enc as usize
    };

    let mut dims = Vec::with_capacity(ndims_raw);
    for _ in 0..ndims_raw {
        let dim = cursor.read_uvar(dim_bytes)?;
        let dim = u32::try_from(dim).map_err(|_| {
            Error::InvalidData(format!("chunk dimension {dim} exceeds u32 capacity"))
        })?;
        dims.push(dim);
    }

    let index_type = cursor.read_u8()?;
    let chunk_size_len = if version >= 5 {
        offset_size
    } else {
        length_size
    };
    let chunk_indexing = parse_chunk_indexing_v4_v5(cursor, flags, index_type, chunk_size_len)?;
    let address = cursor.read_offset(offset_size)?;

    Ok(DataLayout::Chunked {
        address,
        dims,
        element_size: 0,
        chunk_indexing: Some(chunk_indexing),
    })
}

fn should_retry_v4_chunked_parse(err: &Error) -> bool {
    match err {
        Error::UnexpectedEof { .. } | Error::UnsupportedChunkIndexType(_) => true,
        Error::InvalidData(msg) => msg.starts_with("unsupported variable integer size:"),
        _ => false,
    }
}

/// Parse chunk indexing for v4/v5 layout.
/// On-disk values: 1=SingleChunk, 2=Implicit, 3=FixedArray, 4=ExtensibleArray, 5=BTreeV2
fn parse_chunk_indexing_v4_v5(
    cursor: &mut Cursor<'_>,
    flags: u8,
    index_type: u8,
    chunk_size_len: u8,
) -> Result<ChunkIndexing> {
    match index_type {
        1 => {
            // Single chunk. Flag bit 1 ("single index with filter") marks the
            // inline filtered-chunk size and filter mask; bit 0 is the
            // unrelated "don't filter partial edge chunks" option.
            let idx_flags = if (flags & 0x02) != 0 {
                let filtered_size = cursor.read_u64_le()?;
                let filter_mask = cursor.read_u32_le()?;
                Some((filtered_size, filter_mask))
            } else {
                None
            };
            let (fs, fm) = idx_flags.unwrap_or((0, 0));
            Ok(ChunkIndexing::SingleChunk {
                filtered_size: fs,
                filters: fm,
            })
        }
        2 => Ok(ChunkIndexing::Implicit),
        3 => {
            let page_bits = cursor.read_u8()?;
            Ok(ChunkIndexing::FixedArray {
                page_bits,
                chunk_size_len,
            })
        }
        4 => {
            let max_bits = cursor.read_u8()?;
            let index_bits = cursor.read_u8()?;
            let min_pointers = cursor.read_u8()?;
            let min_elements = cursor.read_u8()?;
            let _max_dblk_page_bits = cursor.read_u8()?;
            Ok(ChunkIndexing::ExtensibleArray {
                max_bits,
                index_bits,
                min_pointers,
                min_elements,
                chunk_size_len,
            })
        }
        5 => {
            cursor.skip(6)?;
            Ok(ChunkIndexing::BTreeV2)
        }
        t => Err(Error::UnsupportedChunkIndexType(t)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_v3_contiguous() {
        let mut data = vec![
            0x03, // version 3
            0x01, // layout class = contiguous
        ];
        // address (8 bytes)
        data.extend_from_slice(&0x1000u64.to_le_bytes());
        // size (8 bytes)
        data.extend_from_slice(&4096u64.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        match &msg.layout {
            DataLayout::Contiguous { address, size } => {
                assert_eq!(*address, 0x1000);
                assert_eq!(*size, 4096);
            }
            other => panic!("expected Contiguous, got {:?}", other),
        }
    }

    #[test]
    fn parse_v3_compact() {
        let mut data = vec![
            0x03, // version 3
            0x00, // layout class = compact
        ];
        // compact size = 4
        data.extend_from_slice(&4u16.to_le_bytes());
        // inline data
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        match &msg.layout {
            DataLayout::Compact { data } => {
                assert_eq!(data, &[0x01, 0x02, 0x03, 0x04]);
            }
            other => panic!("expected Compact, got {:?}", other),
        }
    }

    #[test]
    fn parse_v3_chunked() {
        let mut data = vec![
            0x03, // version 3
            0x02, // layout class = chunked
            0x03, // dimensionality = 3 (2 chunk dims + 1 element size)
        ];
        // address
        data.extend_from_slice(&0x2000u64.to_le_bytes());
        // dim[0] = 256
        data.extend_from_slice(&256u32.to_le_bytes());
        // dim[1] = 128
        data.extend_from_slice(&128u32.to_le_bytes());
        // element size = 4
        data.extend_from_slice(&4u32.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        match &msg.layout {
            DataLayout::Chunked {
                address,
                dims,
                element_size,
                chunk_indexing,
            } => {
                assert_eq!(*address, 0x2000);
                assert_eq!(dims, &[256, 128]);
                assert_eq!(*element_size, 4);
                assert!(chunk_indexing.is_none());
            }
            other => panic!("expected Chunked, got {:?}", other),
        }
    }

    #[test]
    fn parse_v4_chunked_direct_dim_size_encoding() {
        let mut data = vec![
            0x04, // version 4
            0x02, // layout class = chunked
            0x00, // flags
            0x02, // ndims
            0x04, // 4 bytes per dimension
        ];
        data.extend_from_slice(&3u32.to_le_bytes());
        data.extend_from_slice(&5u32.to_le_bytes());
        data.push(0x03); // fixed array indexing
        data.push(0x00); // page bits
        data.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        match &msg.layout {
            DataLayout::Chunked {
                address,
                dims,
                element_size,
                chunk_indexing,
            } => {
                assert_eq!(*address, 0x1122_3344_5566_7788);
                assert_eq!(dims, &[3, 5]);
                assert_eq!(*element_size, 0);
                match chunk_indexing {
                    Some(ChunkIndexing::FixedArray {
                        page_bits,
                        chunk_size_len,
                    }) => {
                        assert_eq!(*page_bits, 0);
                        assert_eq!(*chunk_size_len, 8);
                    }
                    other => panic!("expected FixedArray indexing, got {:?}", other),
                }
            }
            other => panic!("expected Chunked, got {:?}", other),
        }
    }

    #[test]
    fn parse_v4_chunked_legacy_dim_size_encoding() {
        let mut data = vec![
            0x04, // version 4
            0x02, // layout class = chunked
            0x00, // flags
            0x02, // ndims
            0x03, // legacy encoding: 4 bytes per dimension stored as 3
        ];
        data.extend_from_slice(&3u32.to_le_bytes());
        data.extend_from_slice(&5u32.to_le_bytes());
        data.push(0x03); // fixed array indexing
        data.push(0x00); // page bits
        data.extend_from_slice(&0x8877_6655_4433_2211u64.to_le_bytes());

        let mut cursor = Cursor::new(&data);
        let msg = parse(&mut cursor, 8, 8, data.len()).unwrap();
        match &msg.layout {
            DataLayout::Chunked {
                address,
                dims,
                element_size,
                chunk_indexing,
            } => {
                assert_eq!(*address, 0x8877_6655_4433_2211);
                assert_eq!(dims, &[3, 5]);
                assert_eq!(*element_size, 0);
                match chunk_indexing {
                    Some(ChunkIndexing::FixedArray {
                        page_bits,
                        chunk_size_len,
                    }) => {
                        assert_eq!(*page_bits, 0);
                        assert_eq!(*chunk_size_len, 8);
                    }
                    other => panic!("expected FixedArray indexing, got {:?}", other),
                }
            }
            other => panic!("expected Chunked, got {:?}", other),
        }
    }
}
