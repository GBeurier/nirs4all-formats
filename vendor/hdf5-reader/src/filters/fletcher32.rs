use crate::error::{Error, Result};

/// Verify Fletcher-32 checksum and strip it from the data.
///
/// The checksum is stored as the last 4 bytes of the chunk data.
pub fn verify_and_strip(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 4 {
        return Err(Error::FilterError(
            "data too short for Fletcher-32 checksum".into(),
        ));
    }

    let payload = &data[..data.len() - 4];
    let stored = u32::from_le_bytes([
        data[data.len() - 4],
        data[data.len() - 3],
        data[data.len() - 2],
        data[data.len() - 1],
    ]);

    let computed = crate::checksum::fletcher32(payload);
    if computed != stored {
        return Err(Error::ChecksumMismatch {
            expected: stored,
            actual: computed,
        });
    }

    Ok(payload.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_valid() {
        let payload = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        let checksum = crate::checksum::fletcher32(&payload);
        let mut data = payload.clone();
        data.extend_from_slice(&checksum.to_le_bytes());
        let result = verify_and_strip(&data).unwrap();
        assert_eq!(result, payload);
    }

    #[test]
    fn verify_invalid() {
        let mut data = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        data.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
        assert!(verify_and_strip(&data).is_err());
    }
}
