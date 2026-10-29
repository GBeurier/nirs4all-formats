//! HDF5 LZ4 filter (filter ID 32004).
//!
//! On-disk format per the [HDF5 LZ4 plugin](https://github.com/HDFGroup/hdf5_plugins):
//! - `orig_size: u64 BE` — total uncompressed size of the chunk
//! - Repeated blocks:
//!   - `orig_block_size: u32 BE` — uncompressed size of this block
//!   - `compressed_size: u32 BE` — compressed size of this block
//!   - `compressed_data: [u8; compressed_size]`
//!
//! When `compressed_size == orig_block_size` the block is stored uncompressed.

use crate::error::{Error, Result};

/// Decompress HDF5 LZ4-filtered data.
/// LZ4 cannot expand data by more than ~255x; bound the unlimited path's
/// declared output size by this multiple of the input so a crafted header
/// cannot force a huge up-front allocation.
const LZ4_MAX_EXPANSION: usize = 256;

pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    // Even without a caller-supplied limit, cap the declared output size by
    // what the input could plausibly expand to.
    let implicit_limit = data.len().saturating_mul(LZ4_MAX_EXPANSION);
    decompress_inner(data, Some(implicit_limit))
}

/// Decompress HDF5 LZ4-filtered data, rejecting declared output sizes above
/// `max_output_len` before allocating the output buffer.
pub fn decompress_with_limit(data: &[u8], max_output_len: usize) -> Result<Vec<u8>> {
    decompress_inner(data, Some(max_output_len))
}

fn decompress_inner(data: &[u8], max_output_len: Option<usize>) -> Result<Vec<u8>> {
    if data.len() < 8 {
        return Err(Error::DecompressionError(
            "LZ4: input too short for header".into(),
        ));
    }

    let declared_orig_size = u64::from_be_bytes([
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
    ]);
    let orig_size = usize::try_from(declared_orig_size).map_err(|_| {
        Error::DecompressionError(format!(
            "LZ4: declared output size {declared_orig_size} exceeds platform usize capacity"
        ))
    })?;
    if let Some(max_output_len) = max_output_len {
        if orig_size > max_output_len {
            return Err(Error::DecompressionError(format!(
                "LZ4: declared output size {orig_size} exceeds limit {max_output_len}"
            )));
        }
    }

    // The output grows as blocks are decoded; reserve no more than LZ4 can expand to
    // (~255:1), so a corrupted size field cannot abort the process.
    let mut output = Vec::new();
    let capacity = orig_size.min(data.len().saturating_mul(255).saturating_add(64));
    output.try_reserve_exact(capacity).map_err(|_| {
        Error::DecompressionError(format!("LZ4: cannot allocate {capacity} output bytes"))
    })?;
    let mut pos = 8;

    while pos < data.len() && output.len() < orig_size {
        if pos + 8 > data.len() {
            return Err(Error::DecompressionError(
                "LZ4: truncated block header".into(),
            ));
        }

        let orig_block_size =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        let compressed_size =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        let block_end = output
            .len()
            .checked_add(orig_block_size)
            .ok_or_else(|| Error::DecompressionError("LZ4: block output size overflowed".into()))?;
        if block_end > orig_size {
            return Err(Error::DecompressionError(
                "LZ4: block output exceeds declared output size".into(),
            ));
        }

        if pos + compressed_size > data.len() {
            return Err(Error::DecompressionError(
                "LZ4: truncated block data".into(),
            ));
        }

        if compressed_size == orig_block_size {
            // Stored uncompressed
            output.extend_from_slice(&data[pos..pos + compressed_size]);
        } else {
            let decompressed =
                lz4_flex::decompress(&data[pos..pos + compressed_size], orig_block_size)
                    .map_err(|e| Error::DecompressionError(format!("LZ4: {}", e)))?;
            output.extend_from_slice(&decompressed);
        }
        pos += compressed_size;
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lz4_round_trip() {
        let original = b"hello world hello world hello world!";
        let raw_compressed = lz4_flex::compress(original);

        let mut hdf5_data = Vec::new();
        hdf5_data.extend_from_slice(&(original.len() as u64).to_be_bytes());
        // Block header: orig_block_size + compressed_size
        hdf5_data.extend_from_slice(&(original.len() as u32).to_be_bytes());
        hdf5_data.extend_from_slice(&(raw_compressed.len() as u32).to_be_bytes());
        hdf5_data.extend_from_slice(&raw_compressed);

        let result = decompress(&hdf5_data).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn lz4_uncompressed_block() {
        let original = b"short";
        let mut hdf5_data = Vec::new();
        hdf5_data.extend_from_slice(&(original.len() as u64).to_be_bytes());
        // compressed_size == orig_block_size => uncompressed
        hdf5_data.extend_from_slice(&(original.len() as u32).to_be_bytes());
        hdf5_data.extend_from_slice(&(original.len() as u32).to_be_bytes());
        hdf5_data.extend_from_slice(original);

        let result = decompress(&hdf5_data).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn lz4_too_short() {
        let data = &[0; 7];
        assert!(decompress(data).is_err());
    }

    #[test]
    fn lz4_truncated_block() {
        let mut data = vec![0, 0, 0, 0, 0, 0, 0, 10]; // orig_size = 10
        data.extend_from_slice(&10u32.to_be_bytes()); // orig_block_size = 10
        data.extend_from_slice(&100u32.to_be_bytes()); // compressed_size = 100
        data.extend_from_slice(&[0; 5]); // only 5 bytes of "100"
        assert!(decompress(&data).is_err());
    }

    #[test]
    fn lz4_declared_size_above_limit_errors_before_allocation() {
        let data = (4096u64).to_be_bytes();
        let err = decompress_with_limit(&data, 32).unwrap_err();
        assert!(err.to_string().contains("exceeds limit"));
    }

    #[test]
    fn lz4_block_cannot_exceed_declared_size() {
        let mut data = Vec::new();
        data.extend_from_slice(&4u64.to_be_bytes());
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(&[0; 8]);

        let err = decompress(&data).unwrap_err();
        assert!(err.to_string().contains("exceeds declared output size"));
    }
}
