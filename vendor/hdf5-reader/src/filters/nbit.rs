use crate::error::{Error, Result};

const NBIT_CLASS_ATOMIC: u32 = 1;
const NBIT_CLASS_ARRAY: u32 = 2;
const NBIT_CLASS_COMPOUND: u32 = 3;
const NBIT_CLASS_NOOP: u32 = 4;
const NBIT_ORDER_LE: u32 = 0;
const NBIT_ORDER_BE: u32 = 1;

#[derive(Debug, Clone)]
struct AtomicDesc {
    size: usize,
    order: u32,
    precision: usize,
    offset: usize,
}

#[derive(Debug, Clone)]
enum TypeDesc {
    Atomic(AtomicDesc),
    Array {
        size: usize,
        base: Box<TypeDesc>,
    },
    Compound {
        size: usize,
        members: Vec<(usize, TypeDesc)>,
    },
    NoOp {
        size: usize,
    },
}

impl TypeDesc {
    fn size(&self) -> usize {
        match self {
            TypeDesc::Atomic(desc) => desc.size,
            TypeDesc::Array { size, .. } => *size,
            TypeDesc::Compound { size, .. } => *size,
            TypeDesc::NoOp { size } => *size,
        }
    }

    fn decode_into(&self, out: &mut [u8], offset: usize, bits: &mut BitReader<'_>) -> Result<()> {
        match self {
            TypeDesc::Atomic(desc) => decode_atomic(out, offset, bits, desc),
            TypeDesc::Array { size, base } => {
                let base_size = base.size();
                if base_size == 0 || size % base_size != 0 {
                    return Err(filter_error(
                        "nbit array size is inconsistent with its base type",
                    ));
                }
                for i in 0..(*size / base_size) {
                    base.decode_into(out, offset + i * base_size, bits)?;
                }
                Ok(())
            }
            TypeDesc::Compound { size, members } => {
                for (member_offset, member) in members {
                    let member_end = member_offset
                        .checked_add(member.size())
                        .ok_or_else(|| filter_error("nbit compound member offset overflowed"))?;
                    if member_end > *size {
                        return Err(filter_error(
                            "nbit compound member extends past the record size",
                        ));
                    }
                    member.decode_into(out, offset + member_offset, bits)?;
                }
                Ok(())
            }
            TypeDesc::NoOp { size } => {
                let end = offset
                    .checked_add(*size)
                    .ok_or_else(|| filter_error("nbit output size overflowed"))?;
                let slice = out
                    .get_mut(offset..end)
                    .ok_or_else(|| filter_error("nbit output range is out of bounds"))?;
                for byte in slice {
                    *byte = bits.read_bits(8)?;
                }
                Ok(())
            }
        }
    }
}

struct BitReader<'a> {
    data: &'a [u8],
    byte_idx: usize,
    bits_left: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_idx: 0,
            bits_left: 8,
        }
    }

    fn read_bits(&mut self, count: usize) -> Result<u8> {
        if count > 8 {
            return Err(filter_error(
                "nbit attempted to read more than one byte of packed bits",
            ));
        }
        if count == 0 {
            return Ok(0);
        }

        let mut remaining = count;
        let mut value = 0u16;

        while remaining > 0 {
            let current = self
                .data
                .get(self.byte_idx)
                .copied()
                .ok_or_else(|| filter_error("nbit packed stream ended early"))?;
            let take = remaining.min(self.bits_left as usize);
            let shift = self.bits_left as usize - take;
            let chunk = (current >> shift) & low_mask(take);
            value = (value << take) | u16::from(chunk);
            self.bits_left -= take as u8;
            remaining -= take;

            if self.bits_left == 0 {
                self.byte_idx += 1;
                self.bits_left = 8;
            }
        }

        Ok(value as u8)
    }
}

pub fn decompress(data: &[u8], client_data: &[u32]) -> Result<Vec<u8>> {
    decompress_inner(data, client_data, None)
}

pub fn decompress_with_limit(
    data: &[u8],
    client_data: &[u32],
    max_output_len: usize,
) -> Result<Vec<u8>> {
    decompress_inner(data, client_data, Some(max_output_len))
}

fn decompress_inner(
    data: &[u8],
    client_data: &[u32],
    max_output_len: Option<usize>,
) -> Result<Vec<u8>> {
    if client_data.len() < 3 {
        return Err(filter_error("nbit filter is missing required client data"));
    }

    let expected = usize_from_u32(client_data[0], "nbit parameter count")?;
    if client_data.len() != expected {
        return Err(filter_error(
            "nbit client data length does not match the encoded parameter count",
        ));
    }

    if client_data[1] != 0 {
        validate_output_limit(data.len(), max_output_len)?;
        return Ok(data.to_vec());
    }

    let element_count = usize_from_u32(client_data[2], "nbit element count")?;
    let mut index = 3;
    let desc = parse_desc(client_data, &mut index)?;
    if index != client_data.len() {
        return Err(filter_error(
            "nbit client data contained trailing parameters",
        ));
    }

    let size = desc.size();
    let total_size = element_count
        .checked_mul(size)
        .ok_or_else(|| filter_error("nbit output size overflowed"))?;
    validate_output_limit(total_size, max_output_len)?;
    let mut out = super::zeroed_output(total_size, "nbit")?;
    let mut bits = BitReader::new(data);

    for i in 0..element_count {
        desc.decode_into(&mut out, i * size, &mut bits)?;
    }

    Ok(out)
}

fn validate_output_limit(len: usize, max_output_len: Option<usize>) -> Result<()> {
    if let Some(max_output_len) = max_output_len {
        if len > max_output_len {
            return Err(filter_error(&format!(
                "nbit output size {len} exceeds limit {max_output_len}"
            )));
        }
    }
    Ok(())
}

fn parse_desc(params: &[u32], index: &mut usize) -> Result<TypeDesc> {
    let class = *params
        .get(*index)
        .ok_or_else(|| filter_error("nbit client data ended while parsing a type descriptor"))?;
    *index += 1;

    match class {
        NBIT_CLASS_ATOMIC => {
            let size = next_usize(params, index, "nbit atomic size")?;
            let order = next_u32(params, index, "nbit atomic byte order")?;
            let precision = next_usize(params, index, "nbit atomic precision")?;
            let offset = next_usize(params, index, "nbit atomic offset")?;
            let desc = AtomicDesc {
                size,
                order,
                precision,
                offset,
            };
            validate_atomic(&desc)?;
            Ok(TypeDesc::Atomic(desc))
        }
        NBIT_CLASS_ARRAY => {
            let size = next_usize(params, index, "nbit array size")?;
            let base = parse_desc(params, index)?;
            let base_size = base.size();
            if base_size == 0 || size % base_size != 0 {
                return Err(filter_error(
                    "nbit array size is inconsistent with its base type",
                ));
            }
            Ok(TypeDesc::Array {
                size,
                base: Box::new(base),
            })
        }
        NBIT_CLASS_COMPOUND => {
            let size = next_usize(params, index, "nbit compound size")?;
            let member_count = next_usize(params, index, "nbit compound member count")?;
            let mut members = Vec::with_capacity(member_count);
            for _ in 0..member_count {
                let member_offset = next_usize(params, index, "nbit compound member offset")?;
                let member = parse_desc(params, index)?;
                members.push((member_offset, member));
            }
            Ok(TypeDesc::Compound { size, members })
        }
        NBIT_CLASS_NOOP => Ok(TypeDesc::NoOp {
            size: next_usize(params, index, "nbit no-op size")?,
        }),
        _ => Err(filter_error("nbit encountered an unknown descriptor class")),
    }
}

fn decode_atomic(
    out: &mut [u8],
    offset: usize,
    bits: &mut BitReader<'_>,
    desc: &AtomicDesc,
) -> Result<()> {
    let end = offset
        .checked_add(desc.size)
        .ok_or_else(|| filter_error("nbit output size overflowed"))?;
    let slice = out
        .get_mut(offset..end)
        .ok_or_else(|| filter_error("nbit output range is out of bounds"))?;

    let datatype_len = desc.size * 8;
    let (begin, end_idx, ascending) = match desc.order {
        NBIT_ORDER_LE => {
            let begin = if (desc.precision + desc.offset) % 8 != 0 {
                (desc.precision + desc.offset) / 8
            } else {
                (desc.precision + desc.offset) / 8 - 1
            };
            (begin, desc.offset / 8, false)
        }
        NBIT_ORDER_BE => {
            let begin = (datatype_len - desc.precision - desc.offset) / 8;
            let end = if desc.offset % 8 != 0 {
                (datatype_len - desc.offset) / 8
            } else {
                (datatype_len - desc.offset) / 8 - 1
            };
            (begin, end, true)
        }
        _ => return Err(filter_error("nbit encountered an unknown byte order")),
    };

    if ascending {
        for (k, byte) in slice.iter_mut().enumerate().take(end_idx + 1).skip(begin) {
            *byte = read_atomic_byte(bits, desc, datatype_len, begin, end_idx, k)?;
        }
    } else {
        for k in (end_idx..=begin).rev() {
            slice[k] = read_atomic_byte(bits, desc, datatype_len, begin, end_idx, k)?;
        }
    }

    Ok(())
}

fn read_atomic_byte(
    bits: &mut BitReader<'_>,
    desc: &AtomicDesc,
    datatype_len: usize,
    begin: usize,
    end: usize,
    byte_index: usize,
) -> Result<u8> {
    let (bit_count, bit_offset) = if begin != end {
        if byte_index == begin {
            let remainder = (datatype_len - desc.precision - desc.offset) % 8;
            (if remainder == 0 { 8 } else { 8 - remainder }, 0)
        } else if byte_index == end {
            let bit_count = if desc.offset % 8 == 0 {
                8
            } else {
                8 - (desc.offset % 8)
            };
            (bit_count, 8 - bit_count)
        } else {
            (8, 0)
        }
    } else {
        (desc.precision, desc.offset % 8)
    };

    let value = bits.read_bits(bit_count)?;
    Ok(value << bit_offset)
}

fn validate_atomic(desc: &AtomicDesc) -> Result<()> {
    if desc.size == 0 {
        return Err(filter_error("nbit atomic size must be non-zero"));
    }
    if desc.precision == 0 || desc.precision > desc.size * 8 {
        return Err(filter_error("nbit atomic precision is invalid"));
    }
    if desc.precision + desc.offset > desc.size * 8 {
        return Err(filter_error(
            "nbit atomic precision and offset exceed the type width",
        ));
    }
    Ok(())
}

fn next_u32(params: &[u32], index: &mut usize, what: &str) -> Result<u32> {
    let value = *params
        .get(*index)
        .ok_or_else(|| filter_error(&format!("{what} is missing")))?;
    *index += 1;
    Ok(value)
}

fn next_usize(params: &[u32], index: &mut usize, what: &str) -> Result<usize> {
    usize_from_u32(next_u32(params, index, what)?, what)
}

fn usize_from_u32(value: u32, what: &str) -> Result<usize> {
    usize::try_from(value).map_err(|_| filter_error(&format!("{what} does not fit in usize")))
}

fn low_mask(bits: usize) -> u8 {
    if bits >= 8 {
        u8::MAX
    } else {
        ((1u16 << bits) - 1) as u8
    }
}

fn filter_error(message: &str) -> Error {
    Error::FilterError(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct BitWriter {
        data: Vec<u8>,
        bits_left: u8,
    }

    impl BitWriter {
        fn new() -> Self {
            Self {
                data: vec![0],
                bits_left: 8,
            }
        }

        fn write_bits(&mut self, value: u8, count: usize) {
            if count == 0 {
                return;
            }

            let mut remaining = count;
            while remaining > 0 {
                let take = remaining.min(self.bits_left as usize);
                let shift = remaining - take;
                let chunk = (value >> shift) & low_mask(take);
                let idx = self.data.len() - 1;
                self.data[idx] |= chunk << (self.bits_left as usize - take);
                self.bits_left -= take as u8;
                remaining -= take;
                if self.bits_left == 0 {
                    self.data.push(0);
                    self.bits_left = 8;
                }
            }
        }

        fn finish(mut self) -> Vec<u8> {
            if self.bits_left == 8 {
                self.data.pop();
            }
            self.data
        }
    }

    fn encode_atomic(writer: &mut BitWriter, bytes: &[u8], desc: &AtomicDesc) {
        let datatype_len = desc.size * 8;
        match desc.order {
            NBIT_ORDER_LE => {
                let begin = if (desc.precision + desc.offset) % 8 != 0 {
                    (desc.precision + desc.offset) / 8
                } else {
                    (desc.precision + desc.offset) / 8 - 1
                };
                let end = desc.offset / 8;
                for k in (end..=begin).rev() {
                    let (bit_count, bit_offset) = if begin != end {
                        if k == begin {
                            let remainder = (datatype_len - desc.precision - desc.offset) % 8;
                            (if remainder == 0 { 8 } else { 8 - remainder }, 0)
                        } else if k == end {
                            let bit_count = if desc.offset % 8 == 0 {
                                8
                            } else {
                                8 - (desc.offset % 8)
                            };
                            (bit_count, 8 - bit_count)
                        } else {
                            (8, 0)
                        }
                    } else {
                        (desc.precision, desc.offset % 8)
                    };
                    writer.write_bits((bytes[k] >> bit_offset) & low_mask(bit_count), bit_count);
                }
            }
            NBIT_ORDER_BE => {
                let begin = (datatype_len - desc.precision - desc.offset) / 8;
                let end = if desc.offset % 8 != 0 {
                    (datatype_len - desc.offset) / 8
                } else {
                    (datatype_len - desc.offset) / 8 - 1
                };
                for (k, byte) in bytes.iter().enumerate().take(end + 1).skip(begin) {
                    let (bit_count, bit_offset) = if begin != end {
                        if k == begin {
                            let remainder = (datatype_len - desc.precision - desc.offset) % 8;
                            (if remainder == 0 { 8 } else { 8 - remainder }, 0)
                        } else if k == end {
                            let bit_count = if desc.offset % 8 == 0 {
                                8
                            } else {
                                8 - (desc.offset % 8)
                            };
                            (bit_count, 8 - bit_count)
                        } else {
                            (8, 0)
                        }
                    } else {
                        (desc.precision, desc.offset % 8)
                    };
                    writer.write_bits((*byte >> bit_offset) & low_mask(bit_count), bit_count);
                }
            }
            _ => panic!("unexpected order"),
        }
    }

    fn encode_desc(writer: &mut BitWriter, bytes: &[u8], desc: &TypeDesc) {
        match desc {
            TypeDesc::Atomic(atomic) => encode_atomic(writer, bytes, atomic),
            TypeDesc::Array { size, base } => {
                let base_size = base.size();
                for i in 0..(*size / base_size) {
                    encode_desc(writer, &bytes[i * base_size..(i + 1) * base_size], base);
                }
            }
            TypeDesc::Compound { members, .. } => {
                for (offset, member) in members {
                    let end = offset + member.size();
                    encode_desc(writer, &bytes[*offset..end], member);
                }
            }
            TypeDesc::NoOp { size } => {
                for byte in &bytes[..*size] {
                    writer.write_bits(*byte, 8);
                }
            }
        }
    }

    fn encode_stream(desc: &TypeDesc, element_count: usize, raw: &[u8]) -> Vec<u8> {
        let size = desc.size();
        let mut writer = BitWriter::new();
        for i in 0..element_count {
            encode_desc(&mut writer, &raw[i * size..(i + 1) * size], desc);
        }
        writer.finish()
    }

    #[test]
    fn decompresses_atomic_little_endian_values() {
        let desc = TypeDesc::Atomic(AtomicDesc {
            size: 2,
            order: NBIT_ORDER_LE,
            precision: 12,
            offset: 0,
        });
        let raw = vec![0xBC, 0x0A, 0x23, 0x01, 0xFF, 0x0F];
        let encoded = encode_stream(&desc, 3, &raw);
        let client_data = vec![8, 0, 3, NBIT_CLASS_ATOMIC, 2, NBIT_ORDER_LE, 12, 0];

        let decoded = decompress(&encoded, &client_data).unwrap();
        assert_eq!(decoded, raw);
    }

    #[test]
    fn decompresses_compound_records_with_nested_array_and_noop_members() {
        let desc = TypeDesc::Compound {
            size: 6,
            members: vec![
                (
                    0,
                    TypeDesc::Array {
                        size: 4,
                        base: Box::new(TypeDesc::Atomic(AtomicDesc {
                            size: 2,
                            order: NBIT_ORDER_LE,
                            precision: 10,
                            offset: 0,
                        })),
                    },
                ),
                (4, TypeDesc::NoOp { size: 2 }),
            ],
        };
        let raw = vec![
            0xFF, 0x03, 0x55, 0x01, 0xDE, 0xAD, 0x00, 0x00, 0x99, 0x02, 0xBE, 0xEF,
        ];
        let encoded = encode_stream(&desc, 2, &raw);
        let client_data = vec![
            17,
            0,
            2,
            NBIT_CLASS_COMPOUND,
            6,
            2,
            0,
            NBIT_CLASS_ARRAY,
            4,
            NBIT_CLASS_ATOMIC,
            2,
            NBIT_ORDER_LE,
            10,
            0,
            4,
            NBIT_CLASS_NOOP,
            2,
        ];

        let decoded = decompress(&encoded, &client_data).unwrap();
        assert_eq!(decoded, raw);
    }
}
