use netcdf_reader::classic::data::NcReadType;
use netcdf_reader::{NcSliceInfo, NcSliceInfoElem};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Helpers: encode a Vec of numeric values as big-endian bytes
// ---------------------------------------------------------------------------

fn encode_be<T: Copy + ToBeBytes>(values: &[T]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(std::mem::size_of_val(values));
    for v in values {
        buf.extend_from_slice(v.to_be_bytes_vec().as_slice());
    }
    buf
}

/// Helper trait to get big-endian bytes from primitive types.
trait ToBeBytes {
    fn to_be_bytes_vec(&self) -> Vec<u8>;
}

macro_rules! impl_to_be_bytes {
    ($ty:ty) => {
        impl ToBeBytes for $ty {
            fn to_be_bytes_vec(&self) -> Vec<u8> {
                self.to_be_bytes().to_vec()
            }
        }
    };
}

impl_to_be_bytes!(i8);
impl_to_be_bytes!(i16);
impl_to_be_bytes!(i32);
impl_to_be_bytes!(i64);
impl_to_be_bytes!(u8);
impl_to_be_bytes!(u16);
impl_to_be_bytes!(u32);
impl_to_be_bytes!(u64);
impl_to_be_bytes!(f32);
impl_to_be_bytes!(f64);

// ---------------------------------------------------------------------------
// Endian roundtrip: encode to big-endian, decode via decode_bulk_be
// ---------------------------------------------------------------------------

/// Generic roundtrip test: encode values as BE bytes, then decode_bulk_be,
/// and verify equality.
fn roundtrip_check<T>(values: Vec<T>)
where
    T: NcReadType + ToBeBytes + Copy + PartialEq + std::fmt::Debug,
{
    let raw = encode_be(&values);
    let decoded = T::decode_bulk_be(&raw, values.len()).expect("decode_bulk_be must succeed");
    assert_eq!(decoded, values);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // -- Endian conversion roundtrip for each numeric type --

    #[test]
    fn roundtrip_i8(values in prop::collection::vec(any::<i8>(), 0..128)) {
        roundtrip_check(values);
    }

    #[test]
    fn roundtrip_u8(values in prop::collection::vec(any::<u8>(), 0..128)) {
        roundtrip_check(values);
    }

    #[test]
    fn roundtrip_i16(values in prop::collection::vec(any::<i16>(), 0..128)) {
        roundtrip_check(values);
    }

    #[test]
    fn roundtrip_u16(values in prop::collection::vec(any::<u16>(), 0..128)) {
        roundtrip_check(values);
    }

    #[test]
    fn roundtrip_i32(values in prop::collection::vec(any::<i32>(), 0..128)) {
        roundtrip_check(values);
    }

    #[test]
    fn roundtrip_u32(values in prop::collection::vec(any::<u32>(), 0..128)) {
        roundtrip_check(values);
    }

    #[test]
    fn roundtrip_i64(values in prop::collection::vec(any::<i64>(), 0..128)) {
        roundtrip_check(values);
    }

    #[test]
    fn roundtrip_u64(values in prop::collection::vec(any::<u64>(), 0..128)) {
        roundtrip_check(values);
    }

    #[test]
    fn roundtrip_f32(values in prop::collection::vec(any::<f32>(), 0..128)) {
        // Encode and decode; NaN bit patterns are preserved because we compare
        // byte-level via to_be_bytes rather than floating equality.
        let raw = encode_be(&values);
        let decoded = f32::decode_bulk_be(&raw, values.len()).unwrap();
        for (orig, dec) in values.iter().zip(decoded.iter()) {
            assert_eq!(orig.to_be_bytes(), dec.to_be_bytes());
        }
    }

    #[test]
    fn roundtrip_f64(values in prop::collection::vec(any::<f64>(), 0..128)) {
        let raw = encode_be(&values);
        let decoded = f64::decode_bulk_be(&raw, values.len()).unwrap();
        for (orig, dec) in values.iter().zip(decoded.iter()) {
            assert_eq!(orig.to_be_bytes(), dec.to_be_bytes());
        }
    }

    // -- Bulk decode matches per-element decode --

    #[test]
    fn bulk_vs_individual_i32(values in prop::collection::vec(any::<i32>(), 1..64)) {
        let raw = encode_be(&values);
        let bulk = i32::decode_bulk_be(&raw, values.len()).unwrap();
        let individual: Vec<i32> = raw
            .chunks_exact(4)
            .map(|chunk| <i32 as NcReadType>::from_be_bytes(chunk).unwrap())
            .collect();
        prop_assert_eq!(bulk, individual);
    }

    #[test]
    fn bulk_vs_individual_f64(values in prop::collection::vec(any::<f64>(), 1..64)) {
        let raw = encode_be(&values);
        let bulk = f64::decode_bulk_be(&raw, values.len()).unwrap();
        let individual: Vec<f64> = raw
            .chunks_exact(8)
            .map(|chunk| <f64 as NcReadType>::from_be_bytes(chunk).unwrap())
            .collect();
        // Compare bit patterns for NaN safety.
        for (b, i) in bulk.iter().zip(individual.iter()) {
            prop_assert_eq!(b.to_bits(), i.to_bits());
        }
    }

    #[test]
    fn bulk_vs_individual_i16(values in prop::collection::vec(any::<i16>(), 1..64)) {
        let raw = encode_be(&values);
        let bulk = i16::decode_bulk_be(&raw, values.len()).unwrap();
        let individual: Vec<i16> = raw
            .chunks_exact(2)
            .map(|chunk| <i16 as NcReadType>::from_be_bytes(chunk).unwrap())
            .collect();
        prop_assert_eq!(bulk, individual);
    }

    // -- NcSliceInfo::all(ndim) correctness --

    #[test]
    fn slice_info_all_creates_correct_count(ndim in 0usize..16) {
        let info = NcSliceInfo::all(ndim);
        prop_assert_eq!(info.selections.len(), ndim);
        for sel in &info.selections {
            match sel {
                NcSliceInfoElem::Slice { start, end, step } => {
                    prop_assert_eq!(*start, 0);
                    prop_assert_eq!(*end, u64::MAX);
                    prop_assert_eq!(*step, 1);
                }
                NcSliceInfoElem::Index(_) => {
                    prop_assert!(false, "NcSliceInfo::all should not produce Index selections");
                }
            }
        }
    }
}
