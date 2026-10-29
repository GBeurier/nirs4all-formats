use crate::error::{Error, Result};
use flate2::read::ZlibDecoder;
use flate2::{Decompress, FlushDecompress, Status};
use std::io::Read;

/// Decompress DEFLATE/zlib-compressed data.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    decompress_inner(data, None)
}

/// Decompress DEFLATE/zlib-compressed data, reading at most `max_output_len`
/// bytes from the decoded stream.
pub fn decompress_with_limit(data: &[u8], max_output_len: usize) -> Result<Vec<u8>> {
    decompress_inner(data, Some(max_output_len))
}

fn decompress_inner(data: &[u8], max_output_len: Option<usize>) -> Result<Vec<u8>> {
    if let Some(max_output_len) = max_output_len {
        return decompress_bounded(data, max_output_len);
    }

    let mut decoder = ZlibDecoder::new(data);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map_err(|e| Error::DecompressionError(format!("DEFLATE decompression failed: {e}")))?;
    Ok(output)
}

fn decompress_bounded(data: &[u8], max_output_len: usize) -> Result<Vec<u8>> {
    let mut decoder = Decompress::new(true);
    // DEFLATE expands at most ~1032:1, so no valid stream needs more room than this; a corrupted
    // chunk shape can no longer ask for a multi-gigabyte buffer before a byte is decoded.
    let capacity = max_output_len.min(data.len().saturating_mul(1032).saturating_add(64));
    let mut output = Vec::new();
    output.try_reserve_exact(capacity).map_err(|_| {
        Error::DecompressionError(format!("DEFLATE: cannot allocate {capacity} output bytes"))
    })?;
    let status = decoder
        .decompress_vec(data, &mut output, FlushDecompress::Finish)
        .map_err(|e| Error::DecompressionError(format!("DEFLATE decompression failed: {e}")))?;

    if matches!(status, Status::StreamEnd) || output.len() == max_output_len {
        return Ok(output);
    }

    Err(Error::DecompressionError(
        "DEFLATE decompression failed: stream did not finish".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A corrupted chunk shape gives an absurd output limit: the stream still decodes,
    /// without reserving the limit up front (it used to abort on allocation failure).
    #[test]
    fn absurd_output_limit_does_not_preallocate() {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        std::io::Write::write_all(&mut encoder, &[7u8; 300]).unwrap();
        let compressed = encoder.finish().unwrap();
        let out = decompress_with_limit(&compressed, usize::MAX / 2).unwrap();
        assert_eq!(out, vec![7u8; 300]);
        assert!(out.capacity() <= compressed.len() * 1032 + 64);
    }
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    #[test]
    fn roundtrip() {
        let original = b"hello world, this is a test of zlib compression!";
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn decompress_with_limit_stops_at_limit() {
        let original = vec![0x5a; 4096];
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&original).unwrap();
        let compressed = encoder.finish().unwrap();

        let decompressed = decompress_with_limit(&compressed, 17).unwrap();
        assert_eq!(decompressed.len(), 17);
        assert_eq!(decompressed, original[..17]);
    }
}
