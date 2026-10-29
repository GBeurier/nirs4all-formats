use crate::error::{ByteOrder, Error, Result};
use crate::messages::datatype::Datatype;

// Re-export types from the datatype message module so users don't need to
// reach into messages::datatype.
pub use crate::messages::datatype::{
    CompoundField, EnumMember, ReferenceType, StringEncoding, StringPadding, StringSize, VarLenKind,
};

/// Trait for types that can be read from HDF5 datasets.
///
/// Implemented for primitive numeric types. Users can implement this
/// for custom types (e.g., compound types).
pub trait H5Type: Sized + Send + Clone {
    /// The HDF5 datatype that this Rust type corresponds to.
    fn hdf5_type() -> Datatype;

    /// Decode a single value from raw bytes with the given datatype.
    fn from_bytes(bytes: &[u8], dtype: &Datatype) -> Result<Self>;

    /// Size of a single element in bytes.
    fn element_size(dtype: &Datatype) -> usize;

    /// Decode many values at once when the datatype has an efficient bulk path.
    ///
    /// Returning `None` falls back to per-element decoding.
    fn decode_vec(_raw: &[u8], _dtype: &Datatype, _count: usize) -> Option<Result<Vec<Self>>> {
        None
    }

    /// Whether raw bytes for this datatype can be copied directly into a `Vec<Self>`
    /// without any further decoding or byte swapping.
    fn native_copy_compatible(_dtype: &Datatype) -> bool {
        false
    }
}

/// Read a numeric value from bytes, handling byte-order conversion.
fn read_numeric<const N: usize>(bytes: &[u8], byte_order: ByteOrder) -> Result<[u8; N]> {
    if bytes.len() < N {
        return Err(Error::InvalidData(format!(
            "expected {} bytes, got {}",
            N,
            bytes.len()
        )));
    }
    let mut arr = [0u8; N];
    arr.copy_from_slice(&bytes[..N]);

    // Swap bytes if the source endianness doesn't match native
    #[cfg(target_endian = "little")]
    if byte_order == ByteOrder::BigEndian {
        arr.reverse();
    }
    #[cfg(target_endian = "big")]
    if byte_order == ByteOrder::LittleEndian {
        arr.reverse();
    }

    Ok(arr)
}

fn byte_order_is_native(byte_order: ByteOrder) -> bool {
    #[cfg(target_endian = "little")]
    {
        byte_order == ByteOrder::LittleEndian
    }
    #[cfg(target_endian = "big")]
    {
        byte_order == ByteOrder::BigEndian
    }
}

macro_rules! impl_h5type_int {
    ($ty:ty, $size:literal, $signed:literal) => {
        impl H5Type for $ty {
            fn hdf5_type() -> Datatype {
                Datatype::FixedPoint {
                    size: $size,
                    signed: $signed,
                    byte_order: if cfg!(target_endian = "little") {
                        ByteOrder::LittleEndian
                    } else {
                        ByteOrder::BigEndian
                    },
                }
            }

            fn from_bytes(bytes: &[u8], dtype: &Datatype) -> Result<Self> {
                match dtype {
                    Datatype::FixedPoint {
                        size,
                        signed,
                        byte_order,
                    } => {
                        if *size as usize != std::mem::size_of::<$ty>() || *signed != $signed {
                            return Err(Error::TypeMismatch {
                                expected: stringify!($ty).into(),
                                actual: format!("FixedPoint(size={}, signed={})", size, signed),
                            });
                        }
                        let arr = read_numeric::<$size>(bytes, *byte_order)?;
                        Ok(<$ty>::from_ne_bytes(arr))
                    }
                    _ => Err(Error::TypeMismatch {
                        expected: stringify!($ty).into(),
                        actual: format!("{:?}", dtype),
                    }),
                }
            }

            fn element_size(_dtype: &Datatype) -> usize {
                $size
            }

            fn decode_vec(raw: &[u8], dtype: &Datatype, count: usize) -> Option<Result<Vec<Self>>> {
                match dtype {
                    Datatype::FixedPoint {
                        size,
                        signed,
                        byte_order,
                    } if *size as usize == $size && *signed == $signed => {
                        let total_bytes = count.checked_mul($size)?;
                        if raw.len() < total_bytes {
                            return None;
                        }

                        let bytes = &raw[..total_bytes];
                        if byte_order_is_native(*byte_order) {
                            let mut values = Vec::<$ty>::with_capacity(count);
                            // SAFETY: `values` has capacity for `count`
                            // elements, `bytes` contains exactly
                            // `count * size_of::<$ty>()` bytes, and integer
                            // types admit every bit pattern.
                            unsafe {
                                std::ptr::copy_nonoverlapping(
                                    bytes.as_ptr(),
                                    values.as_mut_ptr() as *mut u8,
                                    total_bytes,
                                );
                                values.set_len(count);
                            }
                            Some(Ok(values))
                        } else {
                            Some(Ok(bytes
                                .chunks_exact($size)
                                .map(|chunk| {
                                    let mut arr = [0u8; $size];
                                    arr.copy_from_slice(chunk);
                                    arr.reverse();
                                    <$ty>::from_ne_bytes(arr)
                                })
                                .collect()))
                        }
                    }
                    _ => None,
                }
            }

            fn native_copy_compatible(dtype: &Datatype) -> bool {
                matches!(
                    dtype,
                    Datatype::FixedPoint {
                        size,
                        signed,
                        byte_order,
                    } if *size as usize == $size
                        && *signed == $signed
                        && byte_order_is_native(*byte_order)
                )
            }
        }
    };
}

impl_h5type_int!(i8, 1, true);
impl_h5type_int!(u8, 1, false);
impl_h5type_int!(i16, 2, true);
impl_h5type_int!(u16, 2, false);
impl_h5type_int!(i32, 4, true);
impl_h5type_int!(u32, 4, false);
impl_h5type_int!(i64, 8, true);
impl_h5type_int!(u64, 8, false);

impl H5Type for f32 {
    fn hdf5_type() -> Datatype {
        Datatype::FloatingPoint {
            size: 4,
            byte_order: if cfg!(target_endian = "little") {
                ByteOrder::LittleEndian
            } else {
                ByteOrder::BigEndian
            },
        }
    }

    fn from_bytes(bytes: &[u8], dtype: &Datatype) -> Result<Self> {
        match dtype {
            Datatype::FloatingPoint { size, byte_order } => {
                if *size != 4 {
                    return Err(Error::TypeMismatch {
                        expected: "f32".into(),
                        actual: format!("FloatingPoint(size={})", size),
                    });
                }
                let arr = read_numeric::<4>(bytes, *byte_order)?;
                Ok(f32::from_ne_bytes(arr))
            }
            _ => Err(Error::TypeMismatch {
                expected: "f32".into(),
                actual: format!("{:?}", dtype),
            }),
        }
    }

    fn element_size(_dtype: &Datatype) -> usize {
        4
    }

    fn decode_vec(raw: &[u8], dtype: &Datatype, count: usize) -> Option<Result<Vec<Self>>> {
        match dtype {
            Datatype::FloatingPoint { size, byte_order } if *size == 4 => {
                let total_bytes = count.checked_mul(4)?;
                if raw.len() < total_bytes {
                    return None;
                }

                let bytes = &raw[..total_bytes];
                if byte_order_is_native(*byte_order) {
                    let mut values = Vec::<f32>::with_capacity(count);
                    // SAFETY: `values` has capacity for `count` elements,
                    // `bytes` contains exactly `count * size_of::<f32>()`
                    // bytes, and `f32` admits every bit pattern.
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            bytes.as_ptr(),
                            values.as_mut_ptr() as *mut u8,
                            total_bytes,
                        );
                        values.set_len(count);
                    }
                    Some(Ok(values))
                } else {
                    Some(Ok(bytes
                        .chunks_exact(4)
                        .map(|chunk| {
                            let mut arr = [0u8; 4];
                            arr.copy_from_slice(chunk);
                            arr.reverse();
                            f32::from_ne_bytes(arr)
                        })
                        .collect()))
                }
            }
            _ => None,
        }
    }

    fn native_copy_compatible(dtype: &Datatype) -> bool {
        matches!(
            dtype,
            Datatype::FloatingPoint { size, byte_order }
                if *size == 4 && byte_order_is_native(*byte_order)
        )
    }
}

impl H5Type for f64 {
    fn hdf5_type() -> Datatype {
        Datatype::FloatingPoint {
            size: 8,
            byte_order: if cfg!(target_endian = "little") {
                ByteOrder::LittleEndian
            } else {
                ByteOrder::BigEndian
            },
        }
    }

    fn from_bytes(bytes: &[u8], dtype: &Datatype) -> Result<Self> {
        match dtype {
            Datatype::FloatingPoint { size, byte_order } => {
                if *size != 8 {
                    return Err(Error::TypeMismatch {
                        expected: "f64".into(),
                        actual: format!("FloatingPoint(size={})", size),
                    });
                }
                let arr = read_numeric::<8>(bytes, *byte_order)?;
                Ok(f64::from_ne_bytes(arr))
            }
            _ => Err(Error::TypeMismatch {
                expected: "f64".into(),
                actual: format!("{:?}", dtype),
            }),
        }
    }

    fn element_size(_dtype: &Datatype) -> usize {
        8
    }

    fn decode_vec(raw: &[u8], dtype: &Datatype, count: usize) -> Option<Result<Vec<Self>>> {
        match dtype {
            Datatype::FloatingPoint { size, byte_order } if *size == 8 => {
                let total_bytes = count.checked_mul(8)?;
                if raw.len() < total_bytes {
                    return None;
                }

                let bytes = &raw[..total_bytes];
                if byte_order_is_native(*byte_order) {
                    let mut values = Vec::<f64>::with_capacity(count);
                    // SAFETY: `values` has capacity for `count` elements,
                    // `bytes` contains exactly `count * size_of::<f64>()`
                    // bytes, and `f64` admits every bit pattern.
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            bytes.as_ptr(),
                            values.as_mut_ptr() as *mut u8,
                            total_bytes,
                        );
                        values.set_len(count);
                    }
                    Some(Ok(values))
                } else {
                    Some(Ok(bytes
                        .chunks_exact(8)
                        .map(|chunk| {
                            let mut arr = [0u8; 8];
                            arr.copy_from_slice(chunk);
                            arr.reverse();
                            f64::from_ne_bytes(arr)
                        })
                        .collect()))
                }
            }
            _ => None,
        }
    }

    fn native_copy_compatible(dtype: &Datatype) -> bool {
        matches!(
            dtype,
            Datatype::FloatingPoint { size, byte_order }
                if *size == 8 && byte_order_is_native(*byte_order)
        )
    }
}

/// Get the element size from a datatype.
pub fn dtype_element_size(dtype: &Datatype) -> Result<usize> {
    match dtype {
        Datatype::FixedPoint { size, .. } => Ok(*size as usize),
        Datatype::FloatingPoint { size, .. } => Ok(*size as usize),
        Datatype::String {
            size: StringSize::Fixed(n),
            ..
        } => Ok(*n as usize),
        Datatype::String {
            size: StringSize::Variable,
            ..
        } => Ok(16),
        Datatype::Compound { size, .. } => Ok(*size as usize),
        Datatype::Array { base, dims } => {
            let base_size = dtype_element_size(base)?;
            let count = dims.iter().try_fold(1usize, |acc, &dim| {
                let dim = usize::try_from(dim).map_err(|_| {
                    Error::InvalidData(
                        "array datatype dimension exceeds platform usize capacity".to_string(),
                    )
                })?;
                acc.checked_mul(dim).ok_or_else(|| {
                    Error::InvalidData(
                        "array datatype element count exceeds platform usize capacity".to_string(),
                    )
                })
            })?;
            base_size.checked_mul(count).ok_or_else(|| {
                Error::InvalidData(
                    "array datatype byte size exceeds platform usize capacity".to_string(),
                )
            })
        }
        Datatype::Enum { base, .. } => dtype_element_size(base),
        Datatype::VarLen { .. } => Ok(16),
        Datatype::Opaque { size, .. } => Ok(*size as usize),
        Datatype::Reference { size, .. } => Ok(*size as usize),
        Datatype::Bitfield { size, .. } => Ok(*size as usize),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_bulk_decode_native_endian() {
        let dtype = <f32 as H5Type>::hdf5_type();
        let raw = [0.5f32.to_ne_bytes(), 1.25f32.to_ne_bytes()].concat();
        let values = <f32 as H5Type>::decode_vec(&raw, &dtype, 2)
            .unwrap()
            .unwrap();
        assert_eq!(values, vec![0.5, 1.25]);
    }

    #[test]
    fn u32_bulk_decode_big_endian() {
        let dtype = Datatype::FixedPoint {
            size: 4,
            signed: false,
            byte_order: ByteOrder::BigEndian,
        };
        let raw = [1u32.to_be_bytes(), 7u32.to_be_bytes()].concat();
        let values = <u32 as H5Type>::decode_vec(&raw, &dtype, 2)
            .unwrap()
            .unwrap();
        assert_eq!(values, vec![1, 7]);
    }

    #[test]
    fn integer_from_bytes_rejects_signedness_mismatch() {
        let dtype = Datatype::FixedPoint {
            size: 2,
            signed: false,
            byte_order: ByteOrder::LittleEndian,
        };

        let err = <i16 as H5Type>::from_bytes(&u16::MAX.to_le_bytes(), &dtype).unwrap_err();
        assert!(matches!(
            err,
            Error::TypeMismatch {
                expected,
                actual
            } if expected == "i16" && actual.contains("signed=false")
        ));
    }

    #[test]
    fn integer_bulk_decode_rejects_signedness_mismatch() {
        let unsigned_dtype = Datatype::FixedPoint {
            size: 2,
            signed: false,
            byte_order: ByteOrder::LittleEndian,
        };
        let signed_dtype = Datatype::FixedPoint {
            size: 2,
            signed: true,
            byte_order: ByteOrder::LittleEndian,
        };

        assert!(<i16 as H5Type>::decode_vec(&[0, 0], &unsigned_dtype, 1).is_none());
        assert!(<u16 as H5Type>::decode_vec(&[0, 0], &signed_dtype, 1).is_none());
    }

    #[test]
    fn integer_native_copy_compatible_rejects_signedness_mismatch() {
        let unsigned_dtype = Datatype::FixedPoint {
            size: 2,
            signed: false,
            byte_order: if cfg!(target_endian = "little") {
                ByteOrder::LittleEndian
            } else {
                ByteOrder::BigEndian
            },
        };
        let signed_dtype = Datatype::FixedPoint {
            size: 2,
            signed: true,
            byte_order: if cfg!(target_endian = "little") {
                ByteOrder::LittleEndian
            } else {
                ByteOrder::BigEndian
            },
        };

        assert!(!<i16 as H5Type>::native_copy_compatible(&unsigned_dtype));
        assert!(!<u16 as H5Type>::native_copy_compatible(&signed_dtype));
        assert!(<u16 as H5Type>::native_copy_compatible(&unsigned_dtype));
        assert!(<i16 as H5Type>::native_copy_compatible(&signed_dtype));
    }

    #[test]
    fn dtype_element_size_rejects_array_overflow() {
        let dtype = Datatype::Array {
            base: Box::new(Datatype::FixedPoint {
                size: 8,
                signed: false,
                byte_order: ByteOrder::LittleEndian,
            }),
            dims: vec![u64::MAX, 2],
        };

        let err = dtype_element_size(&dtype).unwrap_err();
        assert!(err.to_string().contains("array datatype"));
    }
}
