use crate::error::{Error, Result};

const SCALE_FLOAT_DSCALE: u32 = 0;
const SCALE_FLOAT_ESCALE: u32 = 1;
const SCALE_INT: u32 = 2;

const CLASS_INTEGER: u32 = 0;
const CLASS_FLOAT: u32 = 1;

const SIGN_UNSIGNED: u32 = 0;
const SIGN_SIGNED: u32 = 1;

const ORDER_LE: u32 = 0;
const ORDER_BE: u32 = 1;

const FILL_UNDEFINED: u32 = 0;
const HEADER_SIZE: usize = 21;
const TOTAL_PARAMS: usize = 20;

#[derive(Clone, Copy)]
struct ScaleOffsetParams<'a> {
    raw: &'a [u32],
    element_count: usize,
    class: u32,
    size: usize,
    sign: u32,
    order: u32,
    fill_available: u32,
    scale_type: u32,
    scale_factor: i32,
}

#[derive(Clone, Copy)]
struct PackedParams {
    size: usize,
    minbits: usize,
    native_order: u32,
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
                "scaleoffset attempted to read more than one byte of packed bits",
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
                .ok_or_else(|| filter_error("scaleoffset packed stream ended early"))?;
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
    let params = parse_params(client_data)?;
    let full_width_bits = params
        .size
        .checked_mul(8)
        .ok_or_else(|| filter_error("scaleoffset datatype width overflowed"))?;

    if params.class == CLASS_FLOAT && params.scale_type == SCALE_FLOAT_ESCALE {
        return Err(Error::UnsupportedFilter("scaleoffset float E-scale".into()));
    }

    if params.scale_type != SCALE_FLOAT_DSCALE && params.scale_factor as usize == full_width_bits {
        validate_output_limit(data.len(), max_output_len)?;
        return Ok(data.to_vec());
    }

    if params.class == CLASS_FLOAT {
        if params.scale_type != SCALE_FLOAT_DSCALE && params.scale_type != SCALE_FLOAT_ESCALE {
            return Err(filter_error(
                "scaleoffset float filter has an invalid scale type",
            ));
        }
    } else if params.class == CLASS_INTEGER {
        if params.scale_type != SCALE_INT {
            return Err(filter_error(
                "scaleoffset integer filter has an invalid scale type",
            ));
        }
    } else {
        return Err(filter_error("scaleoffset datatype class is not supported"));
    }

    if data.len() < HEADER_SIZE {
        return Err(filter_error(
            "scaleoffset payload is shorter than the filter header",
        ));
    }

    let minbits = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if minbits > full_width_bits {
        return Err(filter_error(
            "scaleoffset header encodes more bits than the datatype can hold",
        ));
    }

    let minval_size = usize::min(std::mem::size_of::<u64>(), data[4] as usize);
    if HEADER_SIZE > data.len() || 5 + minval_size > data.len() {
        return Err(filter_error("scaleoffset header is truncated"));
    }

    let mut minval_bytes = [0u8; 8];
    minval_bytes[..minval_size].copy_from_slice(&data[5..5 + minval_size]);
    let minval = u64::from_le_bytes(minval_bytes);

    let total_size = params
        .element_count
        .checked_mul(params.size)
        .ok_or_else(|| filter_error("scaleoffset output size overflowed"))?;
    validate_output_limit(total_size, max_output_len)?;
    let mut out = super::zeroed_output(total_size, "scaleoffset")?;

    if minbits == full_width_bits {
        let payload_end = HEADER_SIZE
            .checked_add(total_size)
            .ok_or_else(|| filter_error("scaleoffset payload size overflowed"))?;
        let payload = data.get(HEADER_SIZE..payload_end).ok_or_else(|| {
            filter_error("scaleoffset payload is shorter than the expected output")
        })?;
        out.copy_from_slice(payload);

        if params.order != native_order() {
            swap_endian_in_place(&mut out, params.size);
        }

        return Ok(out);
    } else if minbits != 0 {
        let packed = PackedParams {
            size: params.size,
            minbits,
            native_order: native_order(),
        };
        unpack_packed(&mut out, params.element_count, &data[HEADER_SIZE..], packed)?;
    }

    match params.class {
        CLASS_INTEGER => postprocess_integers(&mut out, params, minbits, minval)?,
        CLASS_FLOAT => postprocess_floats(&mut out, params, minbits, minval)?,
        _ => unreachable!(),
    }

    if params.order != native_order() {
        swap_endian_in_place(&mut out, params.size);
    }

    Ok(out)
}

fn validate_output_limit(len: usize, max_output_len: Option<usize>) -> Result<()> {
    if let Some(max_output_len) = max_output_len {
        if len > max_output_len {
            return Err(filter_error(&format!(
                "scaleoffset output size {len} exceeds limit {max_output_len}"
            )));
        }
    }
    Ok(())
}

fn parse_params(client_data: &[u32]) -> Result<ScaleOffsetParams<'_>> {
    if client_data.len() != TOTAL_PARAMS {
        return Err(filter_error(
            "scaleoffset client data length does not match the required parameter count",
        ));
    }

    Ok(ScaleOffsetParams {
        raw: client_data,
        element_count: usize_from_u32(client_data[2], "scaleoffset element count")?,
        class: client_data[3],
        size: usize_from_u32(client_data[4], "scaleoffset datatype size")?,
        sign: client_data[5],
        order: client_data[6],
        fill_available: client_data[7],
        scale_type: client_data[0],
        scale_factor: client_data[1] as i32,
    })
}

fn unpack_packed(
    out: &mut [u8],
    element_count: usize,
    data: &[u8],
    packed: PackedParams,
) -> Result<()> {
    let mut bits = BitReader::new(data);
    let dtype_len = packed
        .size
        .checked_mul(8)
        .ok_or_else(|| filter_error("scaleoffset datatype width overflowed"))?;

    for i in 0..element_count {
        let offset = i
            .checked_mul(packed.size)
            .ok_or_else(|| filter_error("scaleoffset output offset overflowed"))?;
        let slice = out
            .get_mut(offset..offset + packed.size)
            .ok_or_else(|| filter_error("scaleoffset output range is out of bounds"))?;

        match packed.native_order {
            ORDER_LE => {
                let begin = packed.size - 1 - (dtype_len - packed.minbits) / 8;
                for k in (0..=begin).rev() {
                    let bit_count = if k == begin {
                        let remainder = (dtype_len - packed.minbits) % 8;
                        if remainder == 0 {
                            8
                        } else {
                            8 - remainder
                        }
                    } else {
                        8
                    };
                    slice[k] = bits.read_bits(bit_count)?;
                }
            }
            ORDER_BE => {
                let begin = (dtype_len - packed.minbits) / 8;
                for (k, byte) in slice.iter_mut().enumerate().take(packed.size).skip(begin) {
                    let bit_count = if k == begin {
                        let remainder = (dtype_len - packed.minbits) % 8;
                        if remainder == 0 {
                            8
                        } else {
                            8 - remainder
                        }
                    } else {
                        8
                    };
                    *byte = bits.read_bits(bit_count)?;
                }
            }
            _ => {
                return Err(filter_error(
                    "scaleoffset encountered an unknown byte order",
                ))
            }
        }
    }

    Ok(())
}

fn postprocess_integers(
    out: &mut [u8],
    params: ScaleOffsetParams<'_>,
    minbits: usize,
    minval: u64,
) -> Result<()> {
    let fill = if params.fill_available != FILL_UNDEFINED {
        Some(extract_fill_bytes(params.raw, params.size)?)
    } else {
        None
    };
    let fill_marker = bit_marker(minbits);

    match params.sign {
        SIGN_UNSIGNED => {
            let min = truncate_unsigned(minval, params.size)?;
            for chunk in out.chunks_exact_mut(params.size) {
                let raw = read_unsigned_native(chunk)?;
                let value = if let Some(fill_bytes) = &fill {
                    if raw == fill_marker {
                        read_unsigned_native(fill_bytes)?
                    } else {
                        raw.wrapping_add(min)
                    }
                } else {
                    raw.wrapping_add(min)
                };
                write_unsigned_native(chunk, value)?;
            }
        }
        SIGN_SIGNED => {
            let min = truncate_signed(minval, params.size)?;
            for chunk in out.chunks_exact_mut(params.size) {
                let raw_unsigned = read_unsigned_native(chunk)?;
                let value = if let Some(fill_bytes) = &fill {
                    if raw_unsigned == fill_marker {
                        read_signed_native(fill_bytes)?
                    } else {
                        read_signed_native(chunk)?.wrapping_add(min)
                    }
                } else {
                    read_signed_native(chunk)?.wrapping_add(min)
                };
                write_signed_native(chunk, value)?;
            }
        }
        _ => {
            return Err(filter_error(
                "scaleoffset integer sign code is not supported",
            ))
        }
    }

    Ok(())
}

fn postprocess_floats(
    out: &mut [u8],
    params: ScaleOffsetParams<'_>,
    minbits: usize,
    minval: u64,
) -> Result<()> {
    if params.scale_type != SCALE_FLOAT_DSCALE {
        return Err(Error::UnsupportedFilter("scaleoffset float E-scale".into()));
    }

    let scale = 10f64.powi(params.scale_factor);
    let fill_marker = bit_marker(minbits);
    let fill = if params.fill_available != FILL_UNDEFINED {
        Some(extract_fill_bytes(params.raw, params.size)?)
    } else {
        None
    };

    match params.size {
        4 => {
            let min_bytes = truncate_minval_bytes(minval, 4)?;
            let min = f32::from_ne_bytes(min_bytes[..4].try_into().unwrap());
            for chunk in out.chunks_exact_mut(4) {
                let raw = i32::from_ne_bytes(chunk.try_into().unwrap());
                let value = if let Some(fill_bytes) = &fill {
                    if raw as u32 as u64 == fill_marker {
                        f32::from_ne_bytes(fill_bytes.clone().try_into().unwrap())
                    } else {
                        (raw as f64 / scale) as f32 + min
                    }
                } else {
                    (raw as f64 / scale) as f32 + min
                };
                chunk.copy_from_slice(&value.to_ne_bytes());
            }
        }
        8 => {
            let min_bytes = truncate_minval_bytes(minval, 8)?;
            let min = f64::from_ne_bytes(min_bytes[..8].try_into().unwrap());
            for chunk in out.chunks_exact_mut(8) {
                let raw = i64::from_ne_bytes(chunk.try_into().unwrap());
                let value = if let Some(fill_bytes) = &fill {
                    if raw as u64 == fill_marker {
                        f64::from_ne_bytes(fill_bytes.clone().try_into().unwrap())
                    } else {
                        raw as f64 / scale + min
                    }
                } else {
                    raw as f64 / scale + min
                };
                chunk.copy_from_slice(&value.to_ne_bytes());
            }
        }
        _ => {
            return Err(filter_error(
                "scaleoffset floating-point decode only supports 4-byte and 8-byte values",
            ))
        }
    }

    Ok(())
}

fn extract_fill_bytes(params: &[u32], size: usize) -> Result<Vec<u8>> {
    let mut out = vec![0u8; size];
    let mut idx = 8usize;
    if native_order() == ORDER_LE {
        let mut pos = 0usize;
        while pos < size {
            let word = *params
                .get(idx)
                .ok_or_else(|| filter_error("scaleoffset fill value is truncated"))?;
            let bytes = word.to_ne_bytes();
            let take = (size - pos).min(4);
            out[pos..pos + take].copy_from_slice(&bytes[..take]);
            pos += take;
            idx += 1;
        }
    } else {
        let mut remaining = size;
        let mut pos = size.saturating_sub(remaining.min(4));
        while remaining >= 4 {
            let word = *params
                .get(idx)
                .ok_or_else(|| filter_error("scaleoffset fill value is truncated"))?;
            out[pos..pos + 4].copy_from_slice(&word.to_ne_bytes());
            idx += 1;
            remaining -= 4;
            if remaining >= 4 {
                pos -= 4;
            } else if remaining > 0 {
                pos -= remaining;
            }
        }
        if remaining > 0 {
            let word = *params
                .get(idx)
                .ok_or_else(|| filter_error("scaleoffset fill value is truncated"))?;
            let bytes = word.to_ne_bytes();
            out[..remaining].copy_from_slice(&bytes[4 - remaining..]);
        }
    }
    Ok(out)
}

fn truncate_minval_bytes(minval: u64, size: usize) -> Result<Vec<u8>> {
    match size {
        4 | 8 => {
            let bytes = minval.to_ne_bytes();
            if native_order() == ORDER_LE {
                Ok(bytes[..size].to_vec())
            } else {
                Ok(bytes[8 - size..].to_vec())
            }
        }
        _ => Err(filter_error(
            "scaleoffset floating-point size is not supported",
        )),
    }
}

fn truncate_unsigned(minval: u64, size: usize) -> Result<u64> {
    Ok(match size {
        1 => minval & 0xFF,
        2 => minval & 0xFFFF,
        4 => minval & 0xFFFF_FFFF,
        8 => minval,
        _ => {
            return Err(filter_error(
                "scaleoffset integer decode only supports 1-, 2-, 4-, and 8-byte values",
            ))
        }
    })
}

fn truncate_signed(minval: u64, size: usize) -> Result<i64> {
    Ok(match size {
        1 => i8::from_ne_bytes([minval.to_ne_bytes()[0]]) as i64,
        2 => i16::from_ne_bytes(minval.to_ne_bytes()[..2].try_into().unwrap()) as i64,
        4 => i32::from_ne_bytes(minval.to_ne_bytes()[..4].try_into().unwrap()) as i64,
        8 => i64::from_ne_bytes(minval.to_ne_bytes()),
        _ => {
            return Err(filter_error(
                "scaleoffset integer decode only supports 1-, 2-, 4-, and 8-byte values",
            ))
        }
    })
}

fn read_unsigned_native(bytes: &[u8]) -> Result<u64> {
    Ok(match bytes.len() {
        1 => bytes[0] as u64,
        2 => u16::from_ne_bytes(bytes.try_into().unwrap()) as u64,
        4 => u32::from_ne_bytes(bytes.try_into().unwrap()) as u64,
        8 => u64::from_ne_bytes(bytes.try_into().unwrap()),
        _ => {
            return Err(filter_error(
                "scaleoffset integer decode only supports 1-, 2-, 4-, and 8-byte values",
            ))
        }
    })
}

fn write_unsigned_native(bytes: &mut [u8], value: u64) -> Result<()> {
    match bytes.len() {
        1 => bytes[0] = value as u8,
        2 => bytes.copy_from_slice(&(value as u16).to_ne_bytes()),
        4 => bytes.copy_from_slice(&(value as u32).to_ne_bytes()),
        8 => bytes.copy_from_slice(&value.to_ne_bytes()),
        _ => {
            return Err(filter_error(
                "scaleoffset integer decode only supports 1-, 2-, 4-, and 8-byte values",
            ))
        }
    }
    Ok(())
}

fn read_signed_native(bytes: &[u8]) -> Result<i64> {
    Ok(match bytes.len() {
        1 => i8::from_ne_bytes([bytes[0]]) as i64,
        2 => i16::from_ne_bytes(bytes.try_into().unwrap()) as i64,
        4 => i32::from_ne_bytes(bytes.try_into().unwrap()) as i64,
        8 => i64::from_ne_bytes(bytes.try_into().unwrap()),
        _ => {
            return Err(filter_error(
                "scaleoffset integer decode only supports 1-, 2-, 4-, and 8-byte values",
            ))
        }
    })
}

fn write_signed_native(bytes: &mut [u8], value: i64) -> Result<()> {
    match bytes.len() {
        1 => bytes[0] = value as i8 as u8,
        2 => bytes.copy_from_slice(&(value as i16).to_ne_bytes()),
        4 => bytes.copy_from_slice(&(value as i32).to_ne_bytes()),
        8 => bytes.copy_from_slice(&value.to_ne_bytes()),
        _ => {
            return Err(filter_error(
                "scaleoffset integer decode only supports 1-, 2-, 4-, and 8-byte values",
            ))
        }
    }
    Ok(())
}

fn swap_endian_in_place(bytes: &mut [u8], element_size: usize) {
    if element_size <= 1 {
        return;
    }
    for chunk in bytes.chunks_exact_mut(element_size) {
        chunk.reverse();
    }
}

fn bit_marker(minbits: usize) -> u64 {
    if minbits == 0 {
        0
    } else if minbits >= 64 {
        u64::MAX
    } else {
        (1u64 << minbits) - 1
    }
}

fn native_order() -> u32 {
    if cfg!(target_endian = "little") {
        ORDER_LE
    } else {
        ORDER_BE
    }
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

    fn pack_values(elements: &[Vec<u8>], size: usize, minbits: usize) -> Vec<u8> {
        let mut writer = BitWriter::new();
        let dtype_len = size * 8;
        match native_order() {
            ORDER_LE => {
                let begin = size - 1 - (dtype_len - minbits) / 8;
                for value in elements {
                    for k in (0..=begin).rev() {
                        let bit_count = if k == begin {
                            let remainder = (dtype_len - minbits) % 8;
                            if remainder == 0 {
                                8
                            } else {
                                8 - remainder
                            }
                        } else {
                            8
                        };
                        writer.write_bits(value[k] & low_mask(bit_count), bit_count);
                    }
                }
            }
            ORDER_BE => {
                let begin = (dtype_len - minbits) / 8;
                for value in elements {
                    for (k, byte) in value.iter().enumerate().take(size).skip(begin) {
                        let bit_count = if k == begin {
                            let remainder = (dtype_len - minbits) % 8;
                            if remainder == 0 {
                                8
                            } else {
                                8 - remainder
                            }
                        } else {
                            8
                        };
                        writer.write_bits(*byte & low_mask(bit_count), bit_count);
                    }
                }
            }
            _ => unreachable!(),
        }
        writer.finish()
    }

    fn header_with_minval(minbits: u32, minval: u64, payload: &[u8]) -> Vec<u8> {
        let mut data = vec![0u8; HEADER_SIZE];
        data[..4].copy_from_slice(&minbits.to_le_bytes());
        data[4] = 8;
        data[5..13].copy_from_slice(&minval.to_le_bytes());
        data.extend_from_slice(payload);
        data
    }

    #[test]
    fn decompresses_unsigned_integer_scaleoffset() {
        let encoded_values = vec![
            0u16.to_ne_bytes().to_vec(),
            1u16.to_ne_bytes().to_vec(),
            7u16.to_ne_bytes().to_vec(),
            10u16.to_ne_bytes().to_vec(),
        ];
        let packed = pack_values(&encoded_values, 2, 4);
        let input = header_with_minval(4, 100, &packed);
        let client_data = vec![
            SCALE_INT,
            0,
            4,
            CLASS_INTEGER,
            2,
            SIGN_UNSIGNED,
            native_order(),
            FILL_UNDEFINED,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];

        let decoded = decompress(&input, &client_data).unwrap();
        let values: Vec<u16> = decoded
            .chunks_exact(2)
            .map(|chunk| u16::from_ne_bytes(chunk.try_into().unwrap()))
            .collect();
        assert_eq!(values, vec![100, 101, 107, 110]);
    }

    #[test]
    fn decompresses_float_dscale_values() {
        let encoded_values = vec![
            0i32.to_ne_bytes().to_vec(),
            25i32.to_ne_bytes().to_vec(),
            75i32.to_ne_bytes().to_vec(),
        ];
        let packed = pack_values(&encoded_values, 4, 7);
        let min = 1.25f32;
        let minval = u64::from_ne_bytes({
            let mut bytes = [0u8; 8];
            bytes[..4].copy_from_slice(&min.to_ne_bytes());
            bytes
        });
        let input = header_with_minval(7, minval, &packed);
        let client_data = vec![
            SCALE_FLOAT_DSCALE,
            2,
            3,
            CLASS_FLOAT,
            4,
            SIGN_SIGNED,
            native_order(),
            FILL_UNDEFINED,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];

        let decoded = decompress(&input, &client_data).unwrap();
        let values: Vec<f32> = decoded
            .chunks_exact(4)
            .map(|chunk| f32::from_ne_bytes(chunk.try_into().unwrap()))
            .collect();
        assert_eq!(values, vec![1.25, 1.5, 2.0]);
    }

    #[test]
    fn full_precision_integer_payload_skips_postprocess() {
        let raw_values = [300u16.to_ne_bytes(), 511u16.to_ne_bytes()];
        let payload: Vec<u8> = raw_values.iter().flat_map(|v| v.iter().copied()).collect();
        let input = header_with_minval(16, 700, &payload);
        let client_data = vec![
            SCALE_INT,
            0,
            2,
            CLASS_INTEGER,
            2,
            SIGN_UNSIGNED,
            native_order(),
            FILL_UNDEFINED,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];

        let decoded = decompress(&input, &client_data).unwrap();
        let values: Vec<u16> = decoded
            .chunks_exact(2)
            .map(|chunk| u16::from_ne_bytes(chunk.try_into().unwrap()))
            .collect();
        assert_eq!(values, vec![300, 511]);
    }
}
