//! Decoders for NetCDF-4 user-defined data values.

use hdf5_reader::{ByteOrder, Dataset, Datatype, StringPadding, StringSize, VarLenKind};
use ndarray::{ArrayD, IxDyn};

use crate::error::{Error, Result};
use crate::types::{NcIntegerValue, NcType};

/// A decoded NetCDF-4 enum value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NcEnumValue {
    /// The stored integer value.
    pub value: NcIntegerValue,
    /// The matching enum member name, if the stored value is declared by the type.
    pub member: Option<String>,
}

/// A decoded field of a NetCDF-4 compound value.
#[derive(Debug, Clone, PartialEq)]
pub struct NcCompoundValueField {
    pub name: String,
    pub value: NcValue,
}

/// A decoded fixed-size NetCDF-4 array value.
#[derive(Debug, Clone, PartialEq)]
pub struct NcArrayValue {
    pub dims: Vec<u64>,
    pub values: Vec<NcValue>,
}

/// A decoded NetCDF-4 value.
#[derive(Debug, Clone, PartialEq)]
pub enum NcValue {
    Byte(i8),
    Char(u8),
    Short(i16),
    Int(i32),
    Float(f32),
    Double(f64),
    UByte(u8),
    UShort(u16),
    UInt(u32),
    Int64(i64),
    UInt64(u64),
    String(String),
    Enum(NcEnumValue),
    Opaque(Vec<u8>),
    Compound(Vec<NcCompoundValueField>),
    Array(NcArrayValue),
    VLen(Vec<NcValue>),
}

/// A borrowed view of one logical NetCDF-4 value.
///
/// `NcValueView` lets callers decode directly into domain types without first
/// allocating a full [`NcValue`] tree. Use [`NcValueView::to_owned_value`] for
/// the dynamic representation.
#[derive(Clone, Copy)]
pub struct NcValueView<'a> {
    dataset: &'a Dataset,
    dtype: &'a Datatype,
    bytes: &'a [u8],
}

impl<'a> NcValueView<'a> {
    pub(crate) fn new(dataset: &'a Dataset, dtype: &'a Datatype, bytes: &'a [u8]) -> Self {
        Self {
            dataset,
            dtype,
            bytes,
        }
    }

    /// The NetCDF type represented by this value.
    pub fn nc_type(&self) -> Result<NcType> {
        crate::nc4::types::hdf5_to_nc_type(self.dtype)
    }

    /// Decode this value into the dynamic owned representation.
    pub fn to_owned_value(&self) -> Result<NcValue> {
        decode_value(self.dataset, self.dtype, self.bytes)
    }

    /// Decode this value as a NetCDF integer or enum base integer.
    pub fn integer(&self) -> Result<NcIntegerValue> {
        match self.dtype {
            Datatype::FixedPoint {
                size,
                signed,
                byte_order,
            } => crate::nc4::types::decode_fixed_point_integer(
                self.bytes,
                *size,
                *signed,
                *byte_order,
            ),
            Datatype::Enum { base, .. } => crate::nc4::types::decode_enum_integer(base, self.bytes),
            other => Err(Error::TypeMismatch {
                expected: "integer or enum value".to_string(),
                actual: format!("{other:?}"),
            }),
        }
    }

    /// Decode this value as `f32`.
    pub fn f32(&self) -> Result<f32> {
        match self.dtype {
            Datatype::FloatingPoint {
                size: 4,
                byte_order,
            } => Ok(f32::from_ne_bytes(read_ordered_bytes::<4>(
                self.bytes,
                *byte_order,
            )?)),
            other => Err(Error::TypeMismatch {
                expected: "f32".to_string(),
                actual: format!("{other:?}"),
            }),
        }
    }

    /// Decode this value as `f64`.
    pub fn f64(&self) -> Result<f64> {
        match self.dtype {
            Datatype::FloatingPoint {
                size: 8,
                byte_order,
            } => Ok(f64::from_ne_bytes(read_ordered_bytes::<8>(
                self.bytes,
                *byte_order,
            )?)),
            other => Err(Error::TypeMismatch {
                expected: "f64".to_string(),
                actual: format!("{other:?}"),
            }),
        }
    }

    /// Decode this value as a NetCDF-4 enum.
    pub fn enum_value(&self) -> Result<NcEnumValue> {
        match self.dtype {
            Datatype::Enum { base, members } => {
                let value = crate::nc4::types::decode_enum_integer(base, self.bytes)?;
                let mut member_name = None;
                for member in members {
                    if crate::nc4::types::decode_enum_integer(base, &member.value)? == value {
                        member_name = Some(member.name.clone());
                        break;
                    }
                }
                Ok(NcEnumValue {
                    value,
                    member: member_name,
                })
            }
            other => Err(Error::TypeMismatch {
                expected: "enum value".to_string(),
                actual: format!("{other:?}"),
            }),
        }
    }

    /// Borrow this value as an opaque byte blob.
    pub fn opaque_bytes(&self) -> Result<&'a [u8]> {
        match self.dtype {
            Datatype::Opaque { size, .. } => {
                let size = checked_usize(*size as u64, "opaque byte size")?;
                require_len(self.bytes, size, "opaque value")?;
                Ok(&self.bytes[..size])
            }
            other => Err(Error::TypeMismatch {
                expected: "opaque value".to_string(),
                actual: format!("{other:?}"),
            }),
        }
    }

    /// Borrow a field from a compound value by name.
    pub fn compound_field(&self, name: &str) -> Result<NcValueView<'a>> {
        match self.dtype {
            Datatype::Compound { fields, .. } => {
                let field = fields
                    .iter()
                    .find(|field| field.name == name)
                    .ok_or_else(|| {
                        Error::InvalidData(format!("compound field not found: {name}"))
                    })?;
                let start = checked_usize(field.byte_offset as u64, "compound field offset")?;
                let len = value_size(self.dataset, &field.datatype)?;
                let end = checked_add_usize(start, len, "compound field end")?;
                require_len(self.bytes, end, "compound value")?;
                Ok(NcValueView::new(
                    self.dataset,
                    &field.datatype,
                    &self.bytes[start..end],
                ))
            }
            other => Err(Error::TypeMismatch {
                expected: "compound value".to_string(),
                actual: format!("{other:?}"),
            }),
        }
    }

    /// Borrow all fields from a compound value in declaration order.
    pub fn compound_fields(&self) -> Result<Vec<NcCompoundFieldView<'a>>> {
        match self.dtype {
            Datatype::Compound { fields, .. } => fields
                .iter()
                .map(|field| {
                    let start = checked_usize(field.byte_offset as u64, "compound field offset")?;
                    let len = value_size(self.dataset, &field.datatype)?;
                    let end = checked_add_usize(start, len, "compound field end")?;
                    require_len(self.bytes, end, "compound value")?;
                    Ok(NcCompoundFieldView {
                        name: &field.name,
                        value: NcValueView::new(
                            self.dataset,
                            &field.datatype,
                            &self.bytes[start..end],
                        ),
                    })
                })
                .collect(),
            other => Err(Error::TypeMismatch {
                expected: "compound value".to_string(),
                actual: format!("{other:?}"),
            }),
        }
    }

    /// Borrow all fixed-size array elements in row-major order.
    pub fn array_elements(&self) -> Result<Vec<NcValueView<'a>>> {
        match self.dtype {
            Datatype::Array { base, dims } => {
                let count = checked_product_u64(dims, "array element count")?;
                let elem_size = value_size(self.dataset, base)?;
                let total = checked_mul_usize(count, elem_size, "array byte size")?;
                require_len(self.bytes, total, "array value")?;
                let mut values = Vec::with_capacity(count);
                for index in 0..count {
                    let start = checked_mul_usize(index, elem_size, "array element offset")?;
                    let end = checked_add_usize(start, elem_size, "array element end")?;
                    values.push(NcValueView::new(
                        self.dataset,
                        base,
                        &self.bytes[start..end],
                    ));
                }
                Ok(values)
            }
            other => Err(Error::TypeMismatch {
                expected: "array value".to_string(),
                actual: format!("{other:?}"),
            }),
        }
    }

    /// Decode a non-string vlen value into owned values.
    pub fn vlen_values(&self) -> Result<Vec<NcValue>> {
        match self.dtype {
            Datatype::VarLen {
                kind: VarLenKind::String,
                ..
            } => Err(Error::TypeMismatch {
                expected: "non-string vlen value".to_string(),
                actual: format!("{:?}", self.dtype),
            }),
            Datatype::VarLen { base, .. } => decode_vlen_values(self.dataset, base, self.bytes),
            other => Err(Error::TypeMismatch {
                expected: "vlen value".to_string(),
                actual: format!("{other:?}"),
            }),
        }
    }
}

/// A borrowed compound field view.
#[derive(Clone, Copy)]
pub struct NcCompoundFieldView<'a> {
    pub name: &'a str,
    pub value: NcValueView<'a>,
}

pub(crate) fn read_dataset_with_decoder<T, F>(
    dataset: &Dataset,
    mut decoder: F,
) -> Result<ArrayD<T>>
where
    F: FnMut(NcValueView<'_>) -> Result<T>,
{
    let raw = dataset.read_raw_bytes()?;
    let count = checked_usize(dataset.num_elements()?, "NetCDF-4 variable element count")?;
    let elem_size = value_size(dataset, dataset.dtype())?;
    let total = checked_mul_usize(count, elem_size, "NetCDF-4 variable byte size")?;
    require_len(&raw, total, "NetCDF-4 variable data")?;

    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        let start = checked_mul_usize(index, elem_size, "NetCDF-4 element byte offset")?;
        let end = checked_add_usize(start, elem_size, "NetCDF-4 element byte end")?;
        values.push(decoder(NcValueView::new(
            dataset,
            dataset.dtype(),
            &raw[start..end],
        ))?);
    }

    let shape = dataset
        .shape()
        .iter()
        .map(|&dim| checked_usize(dim, "NetCDF-4 variable dimension"))
        .collect::<Result<Vec<_>>>()?;
    ArrayD::from_shape_vec(IxDyn(&shape), values)
        .map_err(|err| Error::InvalidData(format!("array shape error: {err}")))
}

pub(crate) fn read_dataset_values(dataset: &Dataset) -> Result<ArrayD<NcValue>> {
    read_dataset_with_decoder(dataset, |value| value.to_owned_value())
}

fn decode_value(dataset: &Dataset, dtype: &Datatype, bytes: &[u8]) -> Result<NcValue> {
    match dtype {
        Datatype::FixedPoint {
            size,
            signed,
            byte_order,
        } => integer_to_value(crate::nc4::types::decode_fixed_point_integer(
            bytes,
            *size,
            *signed,
            *byte_order,
        )?),
        Datatype::FloatingPoint {
            size: 4,
            byte_order,
        } => Ok(NcValue::Float(f32::from_ne_bytes(read_ordered_bytes::<4>(
            bytes,
            *byte_order,
        )?))),
        Datatype::FloatingPoint {
            size: 8,
            byte_order,
        } => Ok(NcValue::Double(f64::from_ne_bytes(
            read_ordered_bytes::<8>(bytes, *byte_order)?,
        ))),
        Datatype::FloatingPoint { size, .. } => Err(Error::InvalidData(format!(
            "unsupported floating-point size {size}"
        ))),
        Datatype::String {
            size: StringSize::Fixed(len),
            padding,
            ..
        } => {
            let len = checked_usize(*len as u64, "fixed string length")?;
            require_len(bytes, len, "fixed string value")?;
            Ok(NcValue::String(decode_string_bytes(
                &bytes[..len],
                *padding,
            )?))
        }
        Datatype::String {
            size: StringSize::Variable,
            padding,
            ..
        } => {
            let raw = dataset.resolve_vlen_reference_bytes(bytes, 1)?;
            Ok(NcValue::String(decode_string_bytes(&raw, *padding)?))
        }
        Datatype::Enum { .. } => Ok(NcValue::Enum(
            NcValueView::new(dataset, dtype, bytes).enum_value()?,
        )),
        Datatype::Opaque { size, .. } => {
            let size = checked_usize(*size as u64, "opaque byte size")?;
            require_len(bytes, size, "opaque value")?;
            Ok(NcValue::Opaque(bytes[..size].to_vec()))
        }
        Datatype::Compound { fields, .. } => {
            let mut decoded = Vec::with_capacity(fields.len());
            for field in fields {
                let start = checked_usize(field.byte_offset as u64, "compound field offset")?;
                let len = value_size(dataset, &field.datatype)?;
                let end = checked_add_usize(start, len, "compound field end")?;
                require_len(bytes, end, "compound value")?;
                decoded.push(NcCompoundValueField {
                    name: field.name.clone(),
                    value: decode_value(dataset, &field.datatype, &bytes[start..end])?,
                });
            }
            Ok(NcValue::Compound(decoded))
        }
        Datatype::Array { base, dims } => {
            let count = checked_product_u64(dims, "array element count")?;
            let elem_size = value_size(dataset, base)?;
            let total = checked_mul_usize(count, elem_size, "array byte size")?;
            require_len(bytes, total, "array value")?;
            let mut values = Vec::with_capacity(count);
            for index in 0..count {
                let start = checked_mul_usize(index, elem_size, "array element offset")?;
                let end = checked_add_usize(start, elem_size, "array element end")?;
                values.push(decode_value(dataset, base, &bytes[start..end])?);
            }
            Ok(NcValue::Array(NcArrayValue {
                dims: dims.clone(),
                values,
            }))
        }
        Datatype::VarLen {
            base,
            kind: VarLenKind::String,
            padding,
            ..
        } if base.is_vlen_string_base() => {
            let raw = dataset.resolve_vlen_reference_bytes(bytes, 1)?;
            Ok(NcValue::String(decode_string_bytes(&raw, *padding)?))
        }
        Datatype::VarLen { base, .. } => {
            Ok(NcValue::VLen(decode_vlen_values(dataset, base, bytes)?))
        }
        other => Err(Error::InvalidData(format!(
            "unsupported NetCDF-4 user-defined datatype: {other:?}"
        ))),
    }
}

fn decode_vlen_values(
    dataset: &Dataset,
    base: &Datatype,
    reference: &[u8],
) -> Result<Vec<NcValue>> {
    let elem_size = value_size(dataset, base)?;
    let raw = dataset.resolve_vlen_reference_bytes(reference, elem_size)?;
    if elem_size == 0 {
        return Err(Error::InvalidData(
            "vlen base type has zero byte size".to_string(),
        ));
    }
    if raw.len() % elem_size != 0 {
        return Err(Error::InvalidData(format!(
            "vlen payload has {} bytes, not a multiple of element size {}",
            raw.len(),
            elem_size
        )));
    }

    let count = raw.len() / elem_size;
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        let start = checked_mul_usize(index, elem_size, "vlen element offset")?;
        let end = checked_add_usize(start, elem_size, "vlen element end")?;
        values.push(decode_value(dataset, base, &raw[start..end])?);
    }
    Ok(values)
}

fn integer_to_value(value: NcIntegerValue) -> Result<NcValue> {
    Ok(match value {
        NcIntegerValue::I8(value) => NcValue::Byte(value),
        NcIntegerValue::U8(value) => NcValue::UByte(value),
        NcIntegerValue::I16(value) => NcValue::Short(value),
        NcIntegerValue::U16(value) => NcValue::UShort(value),
        NcIntegerValue::I32(value) => NcValue::Int(value),
        NcIntegerValue::U32(value) => NcValue::UInt(value),
        NcIntegerValue::I64(value) => NcValue::Int64(value),
        NcIntegerValue::U64(value) => NcValue::UInt64(value),
    })
}

fn value_size(dataset: &Dataset, dtype: &Datatype) -> Result<usize> {
    match dtype {
        Datatype::String {
            size: StringSize::Variable,
            ..
        }
        | Datatype::VarLen { .. } => Ok(dataset.vlen_reference_size()),
        Datatype::Array { base, dims } => {
            let count = checked_product_u64(dims, "array element count")?;
            let elem_size = value_size(dataset, base)?;
            checked_mul_usize(count, elem_size, "array byte size")
        }
        Datatype::Enum { base, .. } => value_size(dataset, base),
        Datatype::FixedPoint { size, .. }
        | Datatype::FloatingPoint { size, .. }
        | Datatype::Bitfield { size, .. }
        | Datatype::Reference { size, .. } => Ok(*size as usize),
        Datatype::String {
            size: StringSize::Fixed(len),
            ..
        } => Ok(*len as usize),
        Datatype::Compound { size, .. } | Datatype::Opaque { size, .. } => Ok(*size as usize),
    }
}

fn decode_string_bytes(bytes: &[u8], padding: StringPadding) -> Result<String> {
    let trimmed = match padding {
        StringPadding::NullTerminate => {
            let end = bytes
                .iter()
                .position(|&byte| byte == 0)
                .unwrap_or(bytes.len());
            &bytes[..end]
        }
        StringPadding::NullPad => {
            let end = bytes
                .iter()
                .rposition(|&byte| byte != 0)
                .map_or(0, |idx| idx + 1);
            &bytes[..end]
        }
        StringPadding::SpacePad => {
            let end = bytes
                .iter()
                .rposition(|&byte| byte != b' ')
                .map_or(0, |idx| idx + 1);
            &bytes[..end]
        }
    };
    String::from_utf8(trimmed.to_vec())
        .map_err(|err| Error::InvalidData(format!("invalid string data: {err}")))
}

fn read_ordered_bytes<const N: usize>(bytes: &[u8], byte_order: ByteOrder) -> Result<[u8; N]> {
    require_len(bytes, N, "numeric value")?;
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes[..N]);
    #[cfg(target_endian = "little")]
    if byte_order == ByteOrder::BigEndian {
        out.reverse();
    }
    #[cfg(target_endian = "big")]
    if byte_order == ByteOrder::LittleEndian {
        out.reverse();
    }
    Ok(out)
}

fn require_len(bytes: &[u8], needed: usize, context: &str) -> Result<()> {
    if bytes.len() < needed {
        return Err(Error::InvalidData(format!(
            "{context} too short: need {needed} bytes, have {}",
            bytes.len()
        )));
    }
    Ok(())
}

fn checked_usize(value: u64, context: &str) -> Result<usize> {
    usize::try_from(value)
        .map_err(|_| Error::InvalidData(format!("{context} exceeds platform usize capacity")))
}

fn checked_add_usize(left: usize, right: usize, context: &str) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| Error::InvalidData(format!("{context} overflowed usize")))
}

fn checked_mul_usize(left: usize, right: usize, context: &str) -> Result<usize> {
    left.checked_mul(right)
        .ok_or_else(|| Error::InvalidData(format!("{context} overflowed usize")))
}

fn checked_product_u64(values: &[u64], context: &str) -> Result<usize> {
    let mut product = 1usize;
    for &value in values {
        product = checked_mul_usize(product, checked_usize(value, context)?, context)?;
    }
    Ok(product)
}
