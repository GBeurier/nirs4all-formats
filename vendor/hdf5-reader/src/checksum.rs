pub use hdf5_core::{fletcher32, jenkins_lookup3};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jenkins_empty() {
        let hash = jenkins_lookup3(b"");
        // Empty input produces a deterministic hash from the initial state
        assert_ne!(hash, 0);
    }

    #[test]
    fn jenkins_known_value() {
        // Test with known input — the hash is deterministic
        let h1 = jenkins_lookup3(b"hello");
        let h2 = jenkins_lookup3(b"hello");
        assert_eq!(h1, h2);

        // Different input -> different hash (with overwhelming probability)
        let h3 = jenkins_lookup3(b"world");
        assert_ne!(h1, h3);
    }

    #[test]
    fn fletcher32_simple() {
        let data = [0x01, 0x02, 0x03, 0x04];
        let checksum = fletcher32(&data);
        // Verify it's deterministic
        assert_eq!(checksum, fletcher32(&data));
    }

    #[test]
    fn fletcher32_known_reference() {
        // Known reference: 4 bytes [0x00, 0x01, 0x00, 0x02] interpreted as
        // big-endian u16 words: 0x0001 and 0x0002.
        // sum1 = (0 + 1) % 65535 = 1; sum2 = (0 + 1) % 65535 = 1
        // sum1 = (1 + 2) % 65535 = 3; sum2 = (1 + 3) % 65535 = 4
        // result = (4 << 16) | 3 = 0x0004_0003
        let data = [0x00, 0x01, 0x00, 0x02];
        assert_eq!(fletcher32(&data), 0x0004_0003);
    }

    #[test]
    fn fletcher32_roundtrip_with_filter() {
        // Build a payload + checksum and verify the filter can strip it
        let payload = vec![0x00u8, 0x80, 0x3F, 0x80, 0x00, 0x00, 0x40, 0x00];
        let ck = fletcher32(&payload);
        let mut data = payload.clone();
        data.extend_from_slice(&ck.to_le_bytes());
        // The filter stores checksum in little-endian.
        let stripped = crate::filters::fletcher32::verify_and_strip(&data).unwrap();
        assert_eq!(stripped, payload);
    }
}
