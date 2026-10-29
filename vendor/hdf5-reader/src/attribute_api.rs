use crate::error::{Error, Result};
use crate::filters::FilterRegistry;
use crate::fractal_heap::FractalHeap;
use crate::global_heap::GlobalHeapCollection;
use crate::io::Cursor;
use crate::messages::attribute::AttributeMessage;
use crate::messages::attribute_info::AttributeInfoMessage;
use crate::messages::dataspace::DataspaceType;
use crate::messages::datatype::{Datatype, StringEncoding, StringPadding, StringSize, VarLenKind};
use crate::messages::HdfMessage;
use crate::object_header::ObjectHeader;
use crate::storage::Storage;
use crate::{btree_v2, messages};

fn checked_usize(value: u64, context: &str) -> Result<usize> {
    usize::try_from(value).map_err(|_| {
        Error::InvalidData(format!(
            "{context} value {value} exceeds platform usize capacity"
        ))
    })
}

/// A parsed, high-level HDF5 attribute.
#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: String,
    pub datatype: Datatype,
    pub shape: Vec<u64>,
    pub raw_data: Vec<u8>,
    pub decoded_strings: Option<Vec<String>>,
}

impl Attribute {
    /// Create from a parsed attribute message.
    pub fn from_message(msg: AttributeMessage) -> Self {
        Self::from_message_with_context(msg, None, 0)
    }

    /// Create from a parsed attribute message with optional file context for
    /// resolving variable-length byte attributes stored in the global heap.
    pub fn from_message_with_context(
        msg: AttributeMessage,
        file_data: Option<&[u8]>,
        offset_size: u8,
    ) -> Self {
        let shape = match msg.dataspace.dataspace_type {
            DataspaceType::Scalar => vec![],
            DataspaceType::Null => vec![0],
            DataspaceType::Simple => msg.dataspace.dims.clone(),
        };
        let decoded_strings = file_data.and_then(|file_data| {
            decoded_vlen_strings(
                &msg.datatype,
                &msg.raw_data,
                file_data,
                offset_size,
                msg.dataspace.num_elements().ok()?,
            )
        });
        let raw_data = if let (Some(file_data), Datatype::VarLen { base, kind, .. }) =
            (file_data, &msg.datatype)
        {
            if *kind == VarLenKind::String && is_byte_vlen(base) && shape.is_empty() {
                resolve_vlen_bytes(&msg.raw_data, file_data, offset_size)
                    .unwrap_or_else(|| msg.raw_data.clone())
            } else {
                msg.raw_data.clone()
            }
        } else {
            msg.raw_data.clone()
        };
        Attribute {
            name: msg.name,
            datatype: msg.datatype,
            shape,
            raw_data,
            decoded_strings,
        }
    }

    /// Total number of elements.
    pub fn num_elements(&self) -> Result<u64> {
        if self.shape.is_empty() {
            Ok(1) // scalar
        } else {
            self.shape.iter().try_fold(1u64, |acc, &dim| {
                acc.checked_mul(dim).ok_or_else(|| {
                    Error::InvalidData("attribute element count overflows u64".to_string())
                })
            })
        }
    }

    /// Read the attribute value as a scalar of the given type.
    pub fn read_scalar<T: crate::datatype_api::H5Type>(&self) -> Result<T> {
        T::from_bytes(&self.raw_data, &self.datatype)
    }

    /// Read the attribute as a 1-D vector of the given type.
    pub fn read_1d<T: crate::datatype_api::H5Type>(&self) -> Result<Vec<T>> {
        let elem_size = T::element_size(&self.datatype);
        let n = checked_usize(self.num_elements()?, "attribute element count")?;
        let mut result = Vec::with_capacity(n);
        for i in 0..n {
            let start = i * elem_size;
            let end = start + elem_size;
            if end > self.raw_data.len() {
                return Err(Error::InvalidData(format!(
                    "attribute data too short: need {} bytes, have {}",
                    end,
                    self.raw_data.len()
                )));
            }
            result.push(T::from_bytes(&self.raw_data[start..end], &self.datatype)?);
        }
        Ok(result)
    }

    /// Read the attribute as a string (for string-typed attributes).
    ///
    /// For variable-length strings, use `read_vlen_string()` with the file data
    /// and offset_size — this method will return an error directing you there.
    pub fn read_string(&self) -> Result<String> {
        if let Some(strings) = &self.decoded_strings {
            return match strings.len() {
                1 => Ok(strings[0].clone()),
                0 => Err(Error::InvalidData(format!(
                    "attribute '{}' contains no string elements",
                    self.name
                ))),
                count => Err(Error::InvalidData(format!(
                    "attribute '{}' contains {count} string elements; use read_strings()",
                    self.name
                ))),
            };
        }
        match &self.datatype {
            Datatype::VarLen {
                base,
                kind: VarLenKind::String,
                ..
            } if is_byte_vlen(base) => decode_varlen_byte_string(&self.raw_data),
            Datatype::String {
                size,
                encoding,
                padding,
            } => match size {
                StringSize::Fixed(len) => {
                    let len = *len as usize;
                    let bytes = if self.raw_data.len() < len {
                        &self.raw_data
                    } else {
                        &self.raw_data[..len]
                    };
                    decode_string(bytes, *padding, *encoding)
                }
                StringSize::Variable => {
                    // For inline vlen strings in attributes, try direct decode.
                    // If it looks like a global heap reference (>= 12 bytes for
                    // seq_len + addr + index), suggest read_vlen_string instead.
                    if self.raw_data.len() >= 12 {
                        // Try to decode directly first — some files inline the string
                        let trimmed = match padding {
                            StringPadding::NullTerminate => {
                                let end = self
                                    .raw_data
                                    .iter()
                                    .position(|&b| b == 0)
                                    .unwrap_or(self.raw_data.len());
                                &self.raw_data[..end]
                            }
                            _ => &self.raw_data,
                        };
                        if let Ok(s) = String::from_utf8(trimmed.to_vec()) {
                            if s.chars()
                                .all(|c| !c.is_control() || c == '\n' || c == '\r' || c == '\t')
                            {
                                return Ok(s);
                            }
                        }
                    }
                    decode_string(&self.raw_data, *padding, *encoding)
                }
            },
            _ => Err(Error::TypeMismatch {
                expected: "String".into(),
                actual: format!("{:?}", self.datatype),
            }),
        }
    }

    /// Read a variable-length string attribute from the global heap.
    ///
    /// Variable-length strings in HDF5 are stored as references into a global
    /// heap collection. Each reference is: `seq_len(u32) + heap_addr(offset_size) + index(u32)`.
    pub fn read_vlen_string(&self, file_data: &[u8], offset_size: u8) -> Result<String> {
        if let Some(strings) = &self.decoded_strings {
            return match strings.len() {
                1 => Ok(strings[0].clone()),
                0 => Err(Error::InvalidData(format!(
                    "attribute '{}' contains no string elements",
                    self.name
                ))),
                count => Err(Error::InvalidData(format!(
                    "attribute '{}' contains {count} string elements; use read_vlen_strings()",
                    self.name
                ))),
            };
        }
        match &self.datatype {
            Datatype::String {
                size: StringSize::Variable,
                encoding,
                padding,
            } => {
                let ref_size = 4 + offset_size as usize + 4; // seq_len + addr + index
                if self.raw_data.len() < ref_size {
                    // Fallback: try direct decode
                    return decode_string(&self.raw_data, *padding, *encoding);
                }
                let bytes = read_one_vlen_string(
                    &self.raw_data,
                    0,
                    file_data,
                    offset_size,
                    *padding,
                    *encoding,
                )?;
                Ok(bytes)
            }
            Datatype::String {
                size: StringSize::Fixed(_),
                ..
            } => self.read_string(),
            _ => Err(Error::TypeMismatch {
                expected: "String".into(),
                actual: format!("{:?}", self.datatype),
            }),
        }
    }

    /// Read an array of variable-length strings from the global heap.
    pub fn read_vlen_strings(&self, file_data: &[u8], offset_size: u8) -> Result<Vec<String>> {
        if let Some(strings) = &self.decoded_strings {
            return Ok(strings.clone());
        }
        match &self.datatype {
            Datatype::String {
                size: StringSize::Variable,
                encoding,
                padding,
            } => {
                let ref_size = 4 + offset_size as usize + 4;
                let n = checked_usize(self.num_elements()?, "attribute string element count")?;
                let mut result = Vec::with_capacity(n);
                for i in 0..n {
                    let offset = i * ref_size;
                    if offset + ref_size > self.raw_data.len() {
                        break;
                    }
                    result.push(read_one_vlen_string(
                        &self.raw_data,
                        offset,
                        file_data,
                        offset_size,
                        *padding,
                        *encoding,
                    )?);
                }
                Ok(result)
            }
            Datatype::String {
                size: StringSize::Fixed(_),
                ..
            } => self.read_strings(),
            _ => Err(Error::TypeMismatch {
                expected: "String array".into(),
                actual: format!("{:?}", self.datatype),
            }),
        }
    }

    /// Read the attribute as a vector of strings.
    pub fn read_strings(&self) -> Result<Vec<String>> {
        if let Some(strings) = &self.decoded_strings {
            return Ok(strings.clone());
        }
        match &self.datatype {
            Datatype::String {
                size: StringSize::Fixed(len),
                encoding,
                padding,
            } => {
                let len = *len as usize;
                let n = checked_usize(self.num_elements()?, "attribute string element count")?;
                let mut result = Vec::with_capacity(n);
                for i in 0..n {
                    let start = i * len;
                    let end = (start + len).min(self.raw_data.len());
                    if start >= self.raw_data.len() {
                        break;
                    }
                    result.push(decode_string(
                        &self.raw_data[start..end],
                        *padding,
                        *encoding,
                    )?);
                }
                Ok(result)
            }
            _ => Err(Error::TypeMismatch {
                expected: "String array".into(),
                actual: format!("{:?}", self.datatype),
            }),
        }
    }

    /// Read an attribute as f64 (with automatic promotion from int types).
    pub fn read_as_f64(&self) -> Result<f64> {
        match &self.datatype {
            Datatype::FloatingPoint { size, .. } => {
                let val: f64 = match size {
                    4 => {
                        let v = self.read_scalar::<f32>()?;
                        v as f64
                    }
                    8 => self.read_scalar::<f64>()?,
                    _ => {
                        return Err(Error::TypeMismatch {
                            expected: "f32 or f64".into(),
                            actual: format!("FloatingPoint(size={})", size),
                        })
                    }
                };
                Ok(val)
            }
            Datatype::FixedPoint { size, signed, .. } => {
                let val = match (size, signed) {
                    (1, true) => self.read_scalar::<i8>()? as f64,
                    (1, false) => self.read_scalar::<u8>()? as f64,
                    (2, true) => self.read_scalar::<i16>()? as f64,
                    (2, false) => self.read_scalar::<u16>()? as f64,
                    (4, true) => self.read_scalar::<i32>()? as f64,
                    (4, false) => self.read_scalar::<u32>()? as f64,
                    (8, true) => self.read_scalar::<i64>()? as f64,
                    (8, false) => self.read_scalar::<u64>()? as f64,
                    _ => {
                        return Err(Error::TypeMismatch {
                            expected: "numeric".into(),
                            actual: format!("FixedPoint(size={})", size),
                        })
                    }
                };
                Ok(val)
            }
            _ => Err(Error::TypeMismatch {
                expected: "numeric".into(),
                actual: format!("{:?}", self.datatype),
            }),
        }
    }
}

pub(crate) fn collect_attribute_messages_storage(
    header: &ObjectHeader,
    storage: &dyn Storage,
    offset_size: u8,
    length_size: u8,
    filter_registry: Option<&FilterRegistry>,
) -> Result<Vec<AttributeMessage>> {
    let mut attributes = Vec::new();
    let mut attribute_info = None;

    for msg in &header.messages {
        match msg {
            HdfMessage::Attribute(attr) => attributes.push(attr.clone()),
            HdfMessage::AttributeInfo(info) => attribute_info = Some(info.clone()),
            _ => {}
        }
    }

    if let Some(info) = attribute_info {
        attributes.extend(load_dense_attribute_messages_storage(
            &info,
            storage,
            offset_size,
            length_size,
            filter_registry,
        )?);
    }

    Ok(attributes)
}

fn load_dense_attribute_messages_storage(
    info: &AttributeInfoMessage,
    storage: &dyn Storage,
    offset_size: u8,
    length_size: u8,
    filter_registry: Option<&FilterRegistry>,
) -> Result<Vec<AttributeMessage>> {
    if Cursor::is_undefined_offset(info.fractal_heap_address, offset_size) {
        return Ok(Vec::new());
    }

    let heap = FractalHeap::parse_at_storage(
        storage,
        info.fractal_heap_address,
        offset_size,
        length_size,
    )?;

    let records = load_dense_attribute_records_storage(info, storage, offset_size, length_size)?;

    let mut attributes = Vec::new();
    for record in records {
        let heap_id = match record {
            btree_v2::BTreeV2Record::AttributeNameHash { heap_id, .. }
            | btree_v2::BTreeV2Record::AttributeCreationOrder { heap_id, .. } => heap_id,
            _ => continue,
        };

        let managed_bytes = heap.get_object_storage_with_registry(
            &heap_id,
            storage,
            offset_size,
            length_size,
            filter_registry,
        )?;

        let mut attr_cursor = Cursor::new(&managed_bytes);
        let attr = messages::attribute::parse(
            &mut attr_cursor,
            offset_size,
            length_size,
            managed_bytes.len(),
        )?;
        attributes.push(attr);
    }

    Ok(attributes)
}

fn load_dense_attribute_records_storage(
    info: &AttributeInfoMessage,
    storage: &dyn Storage,
    offset_size: u8,
    length_size: u8,
) -> Result<Vec<btree_v2::BTreeV2Record>> {
    let mut addrs = Vec::new();
    // Match compact-attribute and NetCDF enumeration order when the object
    // provides a creation-order index; the name index remains the fallback.
    if let Some(creation_order_addr) = info.btree_creation_order_address {
        addrs.push(("creation-order", creation_order_addr));
    }
    addrs.push(("name", info.btree_name_index_address));

    let mut last_error = None;
    for (index_name, addr) in addrs {
        if Cursor::is_undefined_offset(addr, offset_size) {
            continue;
        }

        let header = match btree_v2::BTreeV2Header::parse_at_storage(
            storage,
            addr,
            offset_size,
            length_size,
        ) {
            Ok(header) => header,
            Err(err) => {
                last_error = Some(format!(
                    "failed to parse dense attribute {index_name} B-tree at {addr:#x}: {err}"
                ));
                continue;
            }
        };

        match btree_v2::collect_btree_v2_records_storage(
            storage,
            &header,
            offset_size,
            length_size,
            None,
            &[],
            None,
        ) {
            Ok(records) => return Ok(records),
            Err(err) => {
                last_error = Some(format!(
                    "failed to read dense attribute {index_name} B-tree at {addr:#x}: {err}"
                ));
            }
        }
    }

    if let Some(err) = last_error {
        Err(Error::InvalidData(format!(
            "failed to load dense attribute records: {err}"
        )))
    } else {
        Ok(Vec::new())
    }
}

/// Read one variable-length string from a vlen reference in raw_data.
pub(crate) fn read_one_vlen_string(
    raw_data: &[u8],
    offset: usize,
    file_data: &[u8],
    offset_size: u8,
    padding: StringPadding,
    encoding: StringEncoding,
) -> Result<String> {
    let mut cursor = Cursor::new(&raw_data[offset..]);
    let _seq_len = cursor.read_u32_le()?;
    let heap_addr = cursor.read_offset(offset_size)?;
    let obj_index = cursor.read_u32_le()?;

    if Cursor::is_undefined_offset(heap_addr, offset_size) || obj_index == 0 {
        return Ok(String::new());
    }

    let mut heap_cursor = Cursor::new(file_data);
    heap_cursor.set_position(heap_addr);
    let collection = GlobalHeapCollection::parse(&mut heap_cursor, offset_size, offset_size)?;

    match collection.get_object(obj_index as u16) {
        Some(obj) => decode_string(&obj.data, padding, encoding),
        None => Ok(String::new()),
    }
}

pub(crate) fn read_one_vlen_string_storage(
    raw_data: &[u8],
    offset: usize,
    storage: &dyn Storage,
    offset_size: u8,
    length_size: u8,
    padding: StringPadding,
    encoding: StringEncoding,
) -> Result<String> {
    let mut cursor = Cursor::new(&raw_data[offset..]);
    let _seq_len = cursor.read_u32_le()?;
    let heap_addr = cursor.read_offset(offset_size)?;
    let obj_index = cursor.read_u32_le()?;

    if Cursor::is_undefined_offset(heap_addr, offset_size) || obj_index == 0 {
        return Ok(String::new());
    }

    let collection =
        GlobalHeapCollection::parse_at_storage(storage, heap_addr, offset_size, length_size)?;
    match collection.get_object(obj_index as u16) {
        Some(obj) => decode_string(&obj.data, padding, encoding),
        None => Ok(String::new()),
    }
}

pub(crate) fn decoded_vlen_strings_storage(
    datatype: &Datatype,
    raw_data: &[u8],
    storage: &dyn Storage,
    offset_size: u8,
    length_size: u8,
    element_count: u64,
) -> Option<Vec<String>> {
    let (padding, encoding) = vlen_string_format(datatype)?;
    let ref_size = 4 + offset_size as usize + 4;
    let n = checked_usize(element_count, "attribute string element count").ok()?;
    if raw_data.len() < n.checked_mul(ref_size)? {
        return None;
    }
    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        result.push(
            read_one_vlen_string_storage(
                raw_data,
                i * ref_size,
                storage,
                offset_size,
                length_size,
                padding,
                encoding,
            )
            .ok()?,
        );
    }
    Some(result)
}

fn decoded_vlen_strings(
    datatype: &Datatype,
    raw_data: &[u8],
    file_data: &[u8],
    offset_size: u8,
    element_count: u64,
) -> Option<Vec<String>> {
    let (padding, encoding) = vlen_string_format(datatype)?;
    let ref_size = 4 + offset_size as usize + 4;
    let n = checked_usize(element_count, "attribute string element count").ok()?;
    if raw_data.len() < n.checked_mul(ref_size)? {
        return None;
    }
    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        result.push(
            read_one_vlen_string(
                raw_data,
                i * ref_size,
                file_data,
                offset_size,
                padding,
                encoding,
            )
            .ok()?,
        );
    }
    Some(result)
}

fn vlen_string_format(datatype: &Datatype) -> Option<(StringPadding, StringEncoding)> {
    match datatype {
        Datatype::String {
            size: StringSize::Variable,
            padding,
            encoding,
        } => Some((*padding, *encoding)),
        Datatype::VarLen {
            base,
            kind: VarLenKind::String,
            padding,
            encoding,
        } if is_byte_vlen(base) => Some((*padding, *encoding)),
        _ => None,
    }
}

/// Decode a byte slice into a String, handling padding and encoding.
///
/// HDF5 supports ASCII and UTF-8 string encodings. Both are valid UTF-8
/// (ASCII is a strict subset), so we decode uniformly via `from_utf8`.
pub(crate) fn decode_string(
    bytes: &[u8],
    padding: StringPadding,
    _encoding: StringEncoding,
) -> Result<String> {
    let trimmed = match padding {
        StringPadding::NullTerminate => {
            let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
            &bytes[..end]
        }
        StringPadding::NullPad => {
            let end = bytes.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
            &bytes[..end]
        }
        StringPadding::SpacePad => {
            let end = bytes.iter().rposition(|&b| b != b' ').map_or(0, |i| i + 1);
            &bytes[..end]
        }
    };

    String::from_utf8(trimmed.to_vec())
        .map_err(|e| Error::InvalidData(format!("invalid string data: {e}")))
}

fn is_byte_vlen(base: &Datatype) -> bool {
    base.is_vlen_string_base()
}

pub(crate) fn decode_varlen_byte_string(bytes: &[u8]) -> Result<String> {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8(bytes[..end].to_vec())
        .map_err(|e| Error::InvalidData(format!("invalid string data: {e}")))
}

pub(crate) fn resolve_vlen_bytes(
    raw_data: &[u8],
    file_data: &[u8],
    offset_size: u8,
) -> Option<Vec<u8>> {
    if raw_data.len() < 4 + offset_size as usize + 4 {
        return None;
    }

    let mut cursor = Cursor::new(raw_data);
    let seq_len = cursor.read_u32_le().ok()? as usize;
    let heap_addr = cursor.read_offset(offset_size).ok()?;
    let obj_index = cursor.read_u32_le().ok()? as u16;

    if Cursor::is_undefined_offset(heap_addr, offset_size) || obj_index == 0 {
        return Some(Vec::new());
    }

    let mut heap_cursor = Cursor::new(file_data);
    heap_cursor.set_position(heap_addr);
    let collection =
        GlobalHeapCollection::parse(&mut heap_cursor, offset_size, offset_size).ok()?;
    let object = collection.get_object(obj_index)?;
    Some(object.data[..object.data.len().min(seq_len)].to_vec())
}

pub(crate) fn resolve_vlen_bytes_storage(
    raw_data: &[u8],
    storage: &dyn Storage,
    offset_size: u8,
    length_size: u8,
) -> Option<Vec<u8>> {
    if raw_data.len() < 4 + offset_size as usize + 4 {
        return None;
    }

    let mut cursor = Cursor::new(raw_data);
    let seq_len = cursor.read_u32_le().ok()? as usize;
    let heap_addr = cursor.read_offset(offset_size).ok()?;
    let obj_index = cursor.read_u32_le().ok()? as u16;

    if Cursor::is_undefined_offset(heap_addr, offset_size) || obj_index == 0 {
        return Some(Vec::new());
    }

    let collection =
        GlobalHeapCollection::parse_at_storage(storage, heap_addr, offset_size, length_size)
            .ok()?;
    let object = collection.get_object(obj_index)?;
    Some(object.data[..object.data.len().min(seq_len)].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ByteOrder;
    use crate::storage::BytesStorage;
    use std::f64::consts::PI;

    #[test]
    fn scalar_f64_attribute() {
        let value: f64 = PI;
        let raw_data = value.to_le_bytes().to_vec();
        let attr = Attribute {
            name: "pi".to_string(),
            datatype: Datatype::FloatingPoint {
                size: 8,
                byte_order: ByteOrder::LittleEndian,
            },
            shape: vec![],
            raw_data,
            decoded_strings: None,
        };
        let val = attr.read_scalar::<f64>().unwrap();
        assert!((val - PI).abs() < 1e-10);
    }

    #[test]
    fn one_dimensional_i32_attribute() {
        let values = [1i32, 2, 3, 4];
        let mut raw_data = Vec::new();
        for v in &values {
            raw_data.extend_from_slice(&v.to_le_bytes());
        }
        let attr = Attribute {
            name: "data".to_string(),
            datatype: Datatype::FixedPoint {
                size: 4,
                signed: true,
                byte_order: ByteOrder::LittleEndian,
            },
            shape: vec![4],
            raw_data,
            decoded_strings: None,
        };
        let result = attr.read_1d::<i32>().unwrap();
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    #[test]
    fn string_attribute() {
        let attr = Attribute {
            name: "units".to_string(),
            datatype: Datatype::String {
                size: StringSize::Fixed(10),
                encoding: StringEncoding::Ascii,
                padding: StringPadding::NullPad,
            },
            shape: vec![],
            raw_data: b"meters\0\0\0\0".to_vec(),
            decoded_strings: None,
        };
        assert_eq!(attr.read_string().unwrap(), "meters");
    }

    #[test]
    fn varlen_byte_string_attribute() {
        let attr = Attribute {
            name: "name".to_string(),
            datatype: Datatype::VarLen {
                base: Box::new(Datatype::FixedPoint {
                    size: 1,
                    signed: false,
                    byte_order: ByteOrder::LittleEndian,
                }),
                kind: VarLenKind::String,
                encoding: StringEncoding::Utf8,
                padding: StringPadding::NullTerminate,
            },
            shape: vec![],
            raw_data: b"test_dataset".to_vec(),
            decoded_strings: None,
        };
        assert_eq!(attr.read_string().unwrap(), "test_dataset");
    }

    #[test]
    fn read_as_f64_from_int() {
        let raw_data = 42i32.to_le_bytes().to_vec();
        let attr = Attribute {
            name: "count".to_string(),
            datatype: Datatype::FixedPoint {
                size: 4,
                signed: true,
                byte_order: ByteOrder::LittleEndian,
            },
            shape: vec![],
            raw_data,
            decoded_strings: None,
        };
        let val = attr.read_as_f64().unwrap();
        assert!((val - 42.0).abs() < 1e-10);
    }

    #[test]
    fn dense_attribute_btree_errors_surface() {
        let info = AttributeInfoMessage {
            creation_order_tracked: false,
            creation_order_indexed: false,
            max_creation_index: None,
            fractal_heap_address: 0,
            btree_name_index_address: 0,
            btree_creation_order_address: None,
        };
        let storage = BytesStorage::new(Vec::new());

        let err = load_dense_attribute_records_storage(&info, &storage, 8, 8).unwrap_err();
        assert!(matches!(err, Error::InvalidData(_)));
        assert!(err.to_string().contains("dense attribute"));
    }
}
