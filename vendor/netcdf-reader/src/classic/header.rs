//! Parse the NetCDF classic (CDF-1/2/5) binary header.
//!
//! The classic header is a sequence of big-endian fields describing dimensions,
//! global attributes, and variables. All multi-byte integers are big-endian.
//! Strings are padded to 4-byte alignment. CDF-5 uses 8-byte counts and sizes
//! where CDF-1/2 use 4-byte values.

use crate::error::{Error, Result};
use crate::types::{
    checked_mul_u64, checked_usize_from_u64, NcAttrValue, NcAttribute, NcDimension, NcType,
    NcVariable,
};
use crate::NcFormat;

#[cfg(test)]
use super::types::pad_to_4;
use super::types::{nc_type_from_code, padding_to_4};

// Header tag constants.
const ABSENT: u32 = 0x0000_0000;
const NC_DIMENSION: u32 = 0x0000_000A;
const NC_VARIABLE: u32 = 0x0000_000B;
const NC_ATTRIBUTE: u32 = 0x0000_000C;

/// Streaming (indeterminate) record count sentinel.
const STREAMING: u32 = 0xFFFF_FFFF;

/// Result of parsing a classic NetCDF header.
pub struct ClassicHeader {
    pub dimensions: Vec<NcDimension>,
    pub global_attributes: Vec<NcAttribute>,
    pub variables: Vec<NcVariable>,
    pub numrecs: u64,
}

/// A cursor for reading big-endian data from a byte slice.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Cursor { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn ensure(&self, n: usize) -> Result<()> {
        if self.remaining() < n {
            Err(Error::UnexpectedEof {
                offset: self.pos as u64,
                needed: n as u64,
                available: self.remaining() as u64,
            })
        } else {
            Ok(())
        }
    }

    #[allow(dead_code)]
    fn read_u8(&mut self) -> Result<u8> {
        self.ensure(1)?;
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn read_u16_be(&mut self) -> Result<u16> {
        self.ensure(2)?;
        let v = u16::from_be_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    fn read_u32_be(&mut self) -> Result<u32> {
        self.ensure(4)?;
        let v = u32::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    fn read_i32_be(&mut self) -> Result<i32> {
        self.ensure(4)?;
        let v = i32::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    fn read_u64_be(&mut self) -> Result<u64> {
        self.ensure(8)?;
        let v = u64::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
            self.data[self.pos + 4],
            self.data[self.pos + 5],
            self.data[self.pos + 6],
            self.data[self.pos + 7],
        ]);
        self.pos += 8;
        Ok(v)
    }

    fn read_i64_be(&mut self) -> Result<i64> {
        self.ensure(8)?;
        let v = i64::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
            self.data[self.pos + 4],
            self.data[self.pos + 5],
            self.data[self.pos + 6],
            self.data[self.pos + 7],
        ]);
        self.pos += 8;
        Ok(v)
    }

    fn read_f32_be(&mut self) -> Result<f32> {
        self.ensure(4)?;
        let v = f32::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    fn read_f64_be(&mut self) -> Result<f64> {
        self.ensure(8)?;
        let v = f64::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
            self.data[self.pos + 4],
            self.data[self.pos + 5],
            self.data[self.pos + 6],
            self.data[self.pos + 7],
        ]);
        self.pos += 8;
        Ok(v)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        self.ensure(n)?;
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn skip(&mut self, n: usize) -> Result<()> {
        self.ensure(n)?;
        self.pos += n;
        Ok(())
    }

    /// Read a count field: 4 bytes for CDF-1/2, 8 bytes for CDF-5.
    fn read_count(&mut self, format: NcFormat) -> Result<u64> {
        match format {
            NcFormat::Cdf5 => self.read_u64_be(),
            _ => self.read_u32_be().map(|v| v as u64),
        }
    }

    /// Read a padded name: 4-byte length, then chars, then padding to 4-byte boundary.
    /// The name length prefix is always 4 bytes for CDF-1/2 and 8 bytes for CDF-5.
    fn read_name(&mut self, format: NcFormat) -> Result<String> {
        let len = checked_usize_from_u64(self.read_count(format)?, "classic name length")?;
        let bytes = self.read_bytes(len)?;
        let padded_len = checked_pad_to_4(len, "classic name length")?;
        let pad = padded_len - len;
        if pad > 0 {
            self.skip(pad)?;
        }
        String::from_utf8(bytes.to_vec())
            .map_err(|e| Error::InvalidData(format!("invalid UTF-8 name: {}", e)))
    }
}

fn checked_pad_to_4(len: usize, context: &str) -> Result<usize> {
    len.checked_add(padding_to_4(len)).ok_or_else(|| {
        Error::InvalidData(format!("{context} padded length exceeds platform usize"))
    })
}

fn read_list_count(
    cur: &mut Cursor<'_>,
    format: NcFormat,
    min_bytes_per_entry: u64,
    context: &str,
) -> Result<usize> {
    let raw = cur.read_count(format)?;
    let count = checked_usize_from_u64(raw, context)?;
    let min_needed = checked_mul_u64(raw, min_bytes_per_entry, context)?;
    if min_needed > cur.remaining() as u64 {
        return Err(Error::UnexpectedEof {
            offset: cur.pos as u64,
            needed: min_needed,
            available: cur.remaining() as u64,
        });
    }
    Ok(count)
}

/// Parse a complete classic NetCDF header from raw file bytes.
///
/// The `format` parameter must be one of `Classic`, `Offset64`, or `Cdf5`
/// (the caller has already read and validated the magic bytes).
pub fn parse_header(data: &[u8], format: NcFormat) -> Result<ClassicHeader> {
    // Skip past the 4-byte magic (already validated by caller).
    let mut cur = Cursor::new(data);
    cur.skip(4)?;

    // numrecs: 4 bytes for CDF-1/2, 8 bytes for CDF-5.
    let numrecs_raw = cur.read_count(format)?;
    let is_streaming = format != NcFormat::Cdf5 && (numrecs_raw as u32) == STREAMING;
    let numrecs = if is_streaming { 0 } else { numrecs_raw };

    // dim_list
    let mut dimensions = parse_dim_list(&mut cur, format)?;

    // att_list (global attributes)
    let global_attributes = parse_att_list(&mut cur, format)?;

    // var_list
    let mut variables = parse_var_list(&mut cur, format, &dimensions)?;

    if numrecs > 0 {
        apply_unlimited_dimension_size(&mut dimensions, &mut variables, numrecs);
    }

    Ok(ClassicHeader {
        dimensions,
        global_attributes,
        variables,
        numrecs,
    })
}

pub(crate) fn has_streaming_numrecs(data: &[u8], format: NcFormat) -> bool {
    if format == NcFormat::Cdf5 {
        return false;
    }

    let Some(bytes) = data.get(4..8) else {
        return false;
    };

    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) == STREAMING
}

/// Parse the dimension list.
fn parse_dim_list(cur: &mut Cursor<'_>, format: NcFormat) -> Result<Vec<NcDimension>> {
    let tag = cur.read_u32_be()?;

    if tag == ABSENT {
        // ABSENT is a zero tag followed by a zero count.
        let _zero = cur.read_count(format)?;
        return Ok(Vec::new());
    }

    if tag != NC_DIMENSION {
        return Err(Error::InvalidData(format!(
            "expected NC_DIMENSION tag (0x{:08X}), got 0x{:08X}",
            NC_DIMENSION, tag
        )));
    }

    let nelems = read_list_count(cur, format, 16, "dimension count")?;
    let mut dims = Vec::with_capacity(nelems);

    for _ in 0..nelems {
        let name = cur.read_name(format)?;
        let size = cur.read_count(format)?;
        // A dimension with size 0 is the unlimited (record) dimension.
        let is_unlimited = size == 0;
        dims.push(NcDimension {
            name,
            size,
            is_unlimited,
        });
    }

    Ok(dims)
}

/// Parse an attribute list (used for both global and variable attributes).
fn parse_att_list(cur: &mut Cursor<'_>, format: NcFormat) -> Result<Vec<NcAttribute>> {
    let tag = cur.read_u32_be()?;

    if tag == ABSENT {
        let _zero = cur.read_count(format)?;
        return Ok(Vec::new());
    }

    if tag != NC_ATTRIBUTE {
        return Err(Error::InvalidData(format!(
            "expected NC_ATTRIBUTE tag (0x{:08X}), got 0x{:08X}",
            NC_ATTRIBUTE, tag
        )));
    }

    let nelems = read_list_count(cur, format, 12, "attribute count")?;
    let mut attrs = Vec::with_capacity(nelems);

    for _ in 0..nelems {
        let name = cur.read_name(format)?;
        let nc_type = cur.read_u32_be()?;
        let nvalues = checked_usize_from_u64(cur.read_count(format)?, "attribute value count")?;
        let value = read_attr_values(cur, nc_type, nvalues, format)?;

        attrs.push(NcAttribute { name, value });
    }

    Ok(attrs)
}

/// Read attribute values of the given type and count.
/// Values are padded to a 4-byte boundary in the file.
fn read_attr_values(
    cur: &mut Cursor<'_>,
    nc_type: u32,
    nvalues: usize,
    _format: NcFormat,
) -> Result<NcAttrValue> {
    let typ = nc_type_from_code(nc_type)?;
    let elem_size = typ.size()?;
    let raw_bytes = nvalues.checked_mul(elem_size).ok_or_else(|| {
        Error::InvalidData("classic attribute byte count exceeds platform usize".to_string())
    })?;
    let padded = checked_pad_to_4(raw_bytes, "classic attribute byte count")?;
    // Bound the declared value count by the bytes actually present before any
    // arm allocates; a tiny crafted header must not force a huge allocation.
    if padded > cur.remaining() {
        return Err(Error::InvalidData(format!(
            "classic attribute claims {padded} bytes but only {} remain",
            cur.remaining()
        )));
    }

    match typ {
        NcType::Byte => {
            let bytes = cur.read_bytes(raw_bytes)?;
            let values: Vec<i8> = bytes.iter().map(|&b| b as i8).collect();
            cur.skip(padded - raw_bytes)?;
            Ok(NcAttrValue::Bytes(values))
        }
        NcType::Char => {
            let bytes = cur.read_bytes(raw_bytes)?;
            // Trim trailing null bytes (common in NetCDF char attributes).
            let s = String::from_utf8_lossy(bytes);
            let trimmed = s.trim_end_matches('\0').to_string();
            cur.skip(padded - raw_bytes)?;
            Ok(NcAttrValue::Chars(trimmed))
        }
        NcType::Short => {
            let mut values = Vec::with_capacity(nvalues);
            for _ in 0..nvalues {
                values.push(cur.read_u16_be()? as i16);
            }
            let pad = padded - raw_bytes;
            cur.skip(pad)?;
            Ok(NcAttrValue::Shorts(values))
        }
        NcType::Int => {
            let mut values = Vec::with_capacity(nvalues);
            for _ in 0..nvalues {
                values.push(cur.read_i32_be()?);
            }
            Ok(NcAttrValue::Ints(values))
        }
        NcType::Float => {
            let mut values = Vec::with_capacity(nvalues);
            for _ in 0..nvalues {
                values.push(cur.read_f32_be()?);
            }
            Ok(NcAttrValue::Floats(values))
        }
        NcType::Double => {
            let mut values = Vec::with_capacity(nvalues);
            for _ in 0..nvalues {
                values.push(cur.read_f64_be()?);
            }
            Ok(NcAttrValue::Doubles(values))
        }
        NcType::UByte => {
            let bytes = cur.read_bytes(raw_bytes)?;
            cur.skip(padded - raw_bytes)?;
            Ok(NcAttrValue::UBytes(bytes.to_vec()))
        }
        NcType::UShort => {
            let mut values = Vec::with_capacity(nvalues);
            for _ in 0..nvalues {
                values.push(cur.read_u16_be()?);
            }
            let pad = padded - raw_bytes;
            cur.skip(pad)?;
            Ok(NcAttrValue::UShorts(values))
        }
        NcType::UInt => {
            let mut values = Vec::with_capacity(nvalues);
            for _ in 0..nvalues {
                values.push(cur.read_u32_be()?);
            }
            Ok(NcAttrValue::UInts(values))
        }
        NcType::Int64 => {
            let mut values = Vec::with_capacity(nvalues);
            for _ in 0..nvalues {
                values.push(cur.read_i64_be()?);
            }
            Ok(NcAttrValue::Int64s(values))
        }
        NcType::UInt64 => {
            let mut values = Vec::with_capacity(nvalues);
            for _ in 0..nvalues {
                values.push(cur.read_u64_be()?);
            }
            Ok(NcAttrValue::UInt64s(values))
        }
        NcType::String
        | NcType::Enum { .. }
        | NcType::Compound { .. }
        | NcType::Opaque { .. }
        | NcType::Array { .. }
        | NcType::VLen { .. } => Err(Error::InvalidData(format!(
            "{:?} is not valid in classic format attributes",
            typ
        ))),
    }
}

/// Parse the variable list.
fn parse_var_list(
    cur: &mut Cursor<'_>,
    format: NcFormat,
    dims: &[NcDimension],
) -> Result<Vec<NcVariable>> {
    let tag = cur.read_u32_be()?;

    if tag == ABSENT {
        let _zero = cur.read_count(format)?;
        return Ok(Vec::new());
    }

    if tag != NC_VARIABLE {
        return Err(Error::InvalidData(format!(
            "expected NC_VARIABLE tag (0x{:08X}), got 0x{:08X}",
            NC_VARIABLE, tag
        )));
    }

    let nelems = read_list_count(cur, format, 28, "variable count")?;
    let mut vars = Vec::with_capacity(nelems);

    for _ in 0..nelems {
        let name = cur.read_name(format)?;

        // Number of dimensions for this variable. Each dimension id is a
        // NON_NEG value (at least 4 bytes), so bound the count by the bytes
        // remaining before allocating.
        let ndims = read_list_count(cur, format, 4, "variable dimension count")?;

        // Dimension IDs are NON_NEG values and widen to 64 bits in CDF-5.
        let mut var_dims = Vec::with_capacity(ndims);
        let mut is_record_var = false;
        for _ in 0..ndims {
            let dimid = checked_usize_from_u64(cur.read_count(format)?, "dimension id")?;
            if dimid >= dims.len() {
                return Err(Error::InvalidData(format!(
                    "variable '{}' references dimension index {} but only {} dimensions exist",
                    name,
                    dimid,
                    dims.len()
                )));
            }
            if dims[dimid].is_unlimited {
                is_record_var = true;
            }
            var_dims.push(dims[dimid].clone());
        }

        // Variable attributes.
        let attributes = parse_att_list(cur, format)?;

        // nc_type (always 4 bytes).
        let nc_type_code = cur.read_u32_be()?;
        let dtype = nc_type_from_code(nc_type_code)?;

        // vsize: the padded size of one record's worth of data for this
        // variable, or the padded total size for non-record variables.
        // 4 bytes for CDF-1/2 (with 2^32 - 1 reserved as an overflow marker),
        // 8 bytes for CDF-5. The field is redundant per the spec, so it is
        // read for validation only; sizes are recomputed from the dimensions
        // below, which also stays correct for writers that stored the
        // unpadded value.
        let _vsize = cur.read_count(format)?;

        // begin (data offset): 4 bytes for CDF-1, 8 bytes for CDF-2/5.
        let data_offset = match format {
            NcFormat::Classic => cur.read_u32_be()? as u64,
            NcFormat::Offset64 | NcFormat::Cdf5 => cur.read_u64_be()?,
            _ => unreachable!("classic parser only handles CDF-1/2/5"),
        };

        // The unpadded per-record slab (record vars) or total byte size
        // (fixed vars), derived from dimensions and element size.
        let elem_size = dtype.size().map_err(|_| {
            Error::InvalidData(format!(
                "variable '{name}' has a type without a classic on-disk size"
            ))
        })? as u64;
        let slab_bytes = var_dims
            .iter()
            .skip(usize::from(is_record_var))
            .try_fold(1u64, |acc, dim| acc.checked_mul(dim.size))
            .and_then(|elements| elements.checked_mul(elem_size));
        let (record_size, data_size) = if is_record_var {
            // A record variable whose single record overflows u64 cannot
            // exist in any file.
            let slab_bytes = slab_bytes.ok_or_else(|| {
                Error::InvalidData(format!(
                    "record variable '{name}' per-record byte size overflows u64"
                ))
            })?;
            (slab_bytes, 0)
        } else {
            // Huge fixed variables may overflow; full reads recompute the
            // size and error, while metadata access and slicing still work.
            (0, slab_bytes.unwrap_or(0))
        };

        vars.push(NcVariable {
            name,
            dimensions: var_dims,
            dtype,
            attributes,
            data_offset,
            _data_size: data_size,
            is_record_var,
            record_size,
        });
    }

    Ok(vars)
}

pub(crate) fn apply_unlimited_dimension_size(
    dimensions: &mut [NcDimension],
    variables: &mut [NcVariable],
    numrecs: u64,
) {
    for dim in dimensions.iter_mut().filter(|dim| dim.is_unlimited) {
        dim.size = numrecs;
    }

    for variable in variables {
        for dim in variable
            .dimensions
            .iter_mut()
            .filter(|dim| dim.is_unlimited)
        {
            dim.size = numrecs;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NcFormat;

    /// Build a minimal CDF-1 file header in memory.
    /// This helper constructs valid header bytes for testing.
    fn build_cdf1_header(
        dims: &[(&str, u32)],
        attrs: &[(&str, u32, &[u8])], // (name, nc_type, raw_value_bytes)
        vars: &[(&str, &[u32], u32, u32, u32)], // (name, dimids, nc_type, vsize, offset)
        numrecs: u32,
    ) -> Vec<u8> {
        let mut buf = Vec::new();

        // Magic: CDF\x01
        buf.extend_from_slice(b"CDF\x01");

        // numrecs (4 bytes)
        buf.extend_from_slice(&numrecs.to_be_bytes());

        // dim_list
        if dims.is_empty() {
            // ABSENT
            buf.extend_from_slice(&ABSENT.to_be_bytes());
            buf.extend_from_slice(&0u32.to_be_bytes());
        } else {
            buf.extend_from_slice(&NC_DIMENSION.to_be_bytes());
            buf.extend_from_slice(&(dims.len() as u32).to_be_bytes());
            for (name, size) in dims {
                write_name_cdf1(&mut buf, name);
                buf.extend_from_slice(&size.to_be_bytes());
            }
        }

        // att_list (global)
        write_att_list_cdf1(&mut buf, attrs);

        // var_list
        if vars.is_empty() {
            buf.extend_from_slice(&ABSENT.to_be_bytes());
            buf.extend_from_slice(&0u32.to_be_bytes());
        } else {
            buf.extend_from_slice(&NC_VARIABLE.to_be_bytes());
            buf.extend_from_slice(&(vars.len() as u32).to_be_bytes());
            for (name, dimids, nc_type, vsize, offset) in vars {
                write_name_cdf1(&mut buf, name);
                // ndims
                buf.extend_from_slice(&(dimids.len() as u32).to_be_bytes());
                // dimids
                for &did in *dimids {
                    buf.extend_from_slice(&did.to_be_bytes());
                }
                // att_list (empty for test vars)
                buf.extend_from_slice(&ABSENT.to_be_bytes());
                buf.extend_from_slice(&0u32.to_be_bytes());
                // nc_type
                buf.extend_from_slice(&nc_type.to_be_bytes());
                // vsize
                buf.extend_from_slice(&vsize.to_be_bytes());
                // begin (offset) -- 4 bytes for CDF-1
                buf.extend_from_slice(&offset.to_be_bytes());
            }
        }

        buf
    }

    fn write_name_cdf1(buf: &mut Vec<u8>, name: &str) {
        let name_bytes = name.as_bytes();
        buf.extend_from_slice(&(name_bytes.len() as u32).to_be_bytes());
        buf.extend_from_slice(name_bytes);
        let pad = pad_to_4(name_bytes.len()) - name_bytes.len();
        for _ in 0..pad {
            buf.push(0);
        }
    }

    fn write_att_list_cdf1(buf: &mut Vec<u8>, attrs: &[(&str, u32, &[u8])]) {
        if attrs.is_empty() {
            buf.extend_from_slice(&ABSENT.to_be_bytes());
            buf.extend_from_slice(&0u32.to_be_bytes());
            return;
        }
        buf.extend_from_slice(&NC_ATTRIBUTE.to_be_bytes());
        buf.extend_from_slice(&(attrs.len() as u32).to_be_bytes());
        for (name, nc_type, value_bytes) in attrs {
            write_name_cdf1(buf, name);
            buf.extend_from_slice(&nc_type.to_be_bytes());
            // For simplicity, nvalues = 1 element (caller provides exactly one element's bytes)
            let elem_size = match nc_type {
                1 => 1, // byte
                2 => 1, // char
                3 => 2, // short
                4 => 4, // int
                5 => 4, // float
                6 => 8, // double
                _ => 1,
            };
            let nvalues = value_bytes.len() / elem_size;
            buf.extend_from_slice(&(nvalues as u32).to_be_bytes());
            buf.extend_from_slice(value_bytes);
            let pad = pad_to_4(value_bytes.len()) - value_bytes.len();
            for _ in 0..pad {
                buf.push(0);
            }
        }
    }

    fn write_count_cdf5(buf: &mut Vec<u8>, value: u64) {
        buf.extend_from_slice(&value.to_be_bytes());
    }

    fn write_name_cdf5(buf: &mut Vec<u8>, name: &str) {
        let name_bytes = name.as_bytes();
        write_count_cdf5(buf, name_bytes.len() as u64);
        buf.extend_from_slice(name_bytes);
        let pad = pad_to_4(name_bytes.len()) - name_bytes.len();
        for _ in 0..pad {
            buf.push(0);
        }
    }

    fn build_cdf5_header(
        dims: &[(&str, u64)],
        vars: &[(&str, &[u64], u32, u64, u64)],
        numrecs: u64,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"CDF\x05");
        write_count_cdf5(&mut buf, numrecs);

        if dims.is_empty() {
            buf.extend_from_slice(&ABSENT.to_be_bytes());
            write_count_cdf5(&mut buf, 0);
        } else {
            buf.extend_from_slice(&NC_DIMENSION.to_be_bytes());
            write_count_cdf5(&mut buf, dims.len() as u64);
            for (name, size) in dims {
                write_name_cdf5(&mut buf, name);
                write_count_cdf5(&mut buf, *size);
            }
        }

        buf.extend_from_slice(&ABSENT.to_be_bytes());
        write_count_cdf5(&mut buf, 0);

        if vars.is_empty() {
            buf.extend_from_slice(&ABSENT.to_be_bytes());
            write_count_cdf5(&mut buf, 0);
        } else {
            buf.extend_from_slice(&NC_VARIABLE.to_be_bytes());
            write_count_cdf5(&mut buf, vars.len() as u64);
            for (name, dimids, nc_type, vsize, offset) in vars {
                write_name_cdf5(&mut buf, name);
                write_count_cdf5(&mut buf, dimids.len() as u64);
                for dimid in *dimids {
                    write_count_cdf5(&mut buf, *dimid);
                }
                buf.extend_from_slice(&ABSENT.to_be_bytes());
                write_count_cdf5(&mut buf, 0);
                buf.extend_from_slice(&nc_type.to_be_bytes());
                write_count_cdf5(&mut buf, *vsize);
                buf.extend_from_slice(&offset.to_be_bytes());
            }
        }

        buf
    }

    #[test]
    fn empty_header() {
        let data = build_cdf1_header(&[], &[], &[], 0);
        let header = parse_header(&data, NcFormat::Classic).unwrap();
        assert!(header.dimensions.is_empty());
        assert!(header.global_attributes.is_empty());
        assert!(header.variables.is_empty());
        assert_eq!(header.numrecs, 0);
    }

    #[test]
    fn dimensions() {
        let data = build_cdf1_header(
            &[("x", 10), ("y", 20), ("time", 0)], // time is unlimited
            &[],
            &[],
            5,
        );
        let header = parse_header(&data, NcFormat::Classic).unwrap();
        assert_eq!(header.dimensions.len(), 3);

        assert_eq!(header.dimensions[0].name, "x");
        assert_eq!(header.dimensions[0].size, 10);
        assert!(!header.dimensions[0].is_unlimited);

        assert_eq!(header.dimensions[1].name, "y");
        assert_eq!(header.dimensions[1].size, 20);
        assert!(!header.dimensions[1].is_unlimited);

        assert_eq!(header.dimensions[2].name, "time");
        assert_eq!(header.dimensions[2].size, 5);
        assert!(header.dimensions[2].is_unlimited);

        assert_eq!(header.numrecs, 5);
    }

    #[test]
    fn global_attributes() {
        // One NC_INT attribute with value 42.
        let value_bytes = 42i32.to_be_bytes();
        let data = build_cdf1_header(
            &[],
            &[("answer", 4, &value_bytes)], // NC_INT = 4
            &[],
            0,
        );
        let header = parse_header(&data, NcFormat::Classic).unwrap();
        assert_eq!(header.global_attributes.len(), 1);
        assert_eq!(header.global_attributes[0].name, "answer");
        if let NcAttrValue::Ints(ref v) = header.global_attributes[0].value {
            assert_eq!(v, &[42]);
        } else {
            panic!("expected Ints attribute");
        }
    }

    #[test]
    fn char_attribute() {
        let text = b"hello";
        let data = build_cdf1_header(
            &[],
            &[("greeting", 2, text)], // NC_CHAR = 2
            &[],
            0,
        );
        let header = parse_header(&data, NcFormat::Classic).unwrap();
        assert_eq!(header.global_attributes.len(), 1);
        assert_eq!(header.global_attributes[0].name, "greeting");
        if let NcAttrValue::Chars(ref s) = header.global_attributes[0].value {
            assert_eq!(s, "hello");
        } else {
            panic!("expected Chars attribute");
        }
    }

    #[test]
    fn variables() {
        let data = build_cdf1_header(
            &[("x", 10), ("y", 20)],
            &[],
            &[
                ("temperature", &[0, 1], 5, 800, 200), // float, dimids=[x,y]
                ("pressure", &[0, 1], 6, 1600, 1000),  // double, dimids=[x,y]
            ],
            0,
        );
        let header = parse_header(&data, NcFormat::Classic).unwrap();
        assert_eq!(header.variables.len(), 2);

        let temp = &header.variables[0];
        assert_eq!(temp.name, "temperature");
        assert_eq!(temp.dtype, NcType::Float);
        assert_eq!(temp.dimensions.len(), 2);
        assert_eq!(temp.dimensions[0].name, "x");
        assert_eq!(temp.dimensions[1].name, "y");
        assert_eq!(temp.data_offset, 200);
        assert_eq!(temp._data_size, 800);
        assert!(!temp.is_record_var);

        let pres = &header.variables[1];
        assert_eq!(pres.name, "pressure");
        assert_eq!(pres.dtype, NcType::Double);
        assert_eq!(pres.data_offset, 1000);
        assert_eq!(pres._data_size, 1600);
    }

    #[test]
    fn record_variable() {
        let data = build_cdf1_header(
            &[("time", 0), ("x", 5)], // time is unlimited
            &[],
            &[
                // record variable: first dim is unlimited
                ("values", &[0, 1], 5, 20, 100), // float, vsize=5*4=20 per record
            ],
            10, // 10 records
        );
        let header = parse_header(&data, NcFormat::Classic).unwrap();
        assert_eq!(header.numrecs, 10);
        assert_eq!(header.variables.len(), 1);

        let var = &header.variables[0];
        assert_eq!(var.name, "values");
        assert!(var.is_record_var);
        assert_eq!(var.record_size, 20);
        assert_eq!(var._data_size, 0); // data_size=0 for record vars (computed at read time)
        assert_eq!(var.shape(), vec![10, 5]);
    }

    #[test]
    fn cdf2_offset64() {
        // Build a CDF-2 header manually.
        // CDF-2 is mostly the same as CDF-1 but the data offset (begin) field is 8 bytes.
        let mut buf = Vec::new();
        buf.extend_from_slice(b"CDF\x02");
        // numrecs (4 bytes)
        buf.extend_from_slice(&0u32.to_be_bytes());
        // dim_list: one dimension "x" with size 100
        buf.extend_from_slice(&NC_DIMENSION.to_be_bytes());
        buf.extend_from_slice(&1u32.to_be_bytes());
        write_name_cdf1(&mut buf, "x");
        buf.extend_from_slice(&100u32.to_be_bytes());
        // att_list: absent
        buf.extend_from_slice(&ABSENT.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        // var_list: one variable
        buf.extend_from_slice(&NC_VARIABLE.to_be_bytes());
        buf.extend_from_slice(&1u32.to_be_bytes());
        write_name_cdf1(&mut buf, "data");
        buf.extend_from_slice(&1u32.to_be_bytes()); // ndims=1
        buf.extend_from_slice(&0u32.to_be_bytes()); // dimid=0
                                                    // att_list: absent
        buf.extend_from_slice(&ABSENT.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        // nc_type = NC_FLOAT = 5
        buf.extend_from_slice(&5u32.to_be_bytes());
        // vsize (4 bytes for CDF-2)
        buf.extend_from_slice(&400u32.to_be_bytes());
        // begin (8 bytes for CDF-2!)
        let offset: u64 = 0x1_0000_0000; // > 4 GB offset to test 64-bit
        buf.extend_from_slice(&offset.to_be_bytes());

        let header = parse_header(&buf, NcFormat::Offset64).unwrap();
        assert_eq!(header.variables.len(), 1);
        assert_eq!(header.variables[0].data_offset, 0x1_0000_0000);
        assert_eq!(header.variables[0]._data_size, 400);
    }

    #[test]
    fn cdf5_uses_64_bit_counts_for_var_metadata() {
        let data = build_cdf5_header(
            &[("n", 4)],
            &[
                ("ubyte_var", &[0], 7, 4, 128),
                ("int64_var", &[0], 10, 32, 256),
            ],
            0,
        );

        let header = parse_header(&data, NcFormat::Cdf5).unwrap();
        assert_eq!(header.variables.len(), 2);
        assert_eq!(header.variables[0].name, "ubyte_var");
        assert_eq!(header.variables[0].dtype, NcType::UByte);
        assert_eq!(header.variables[0].dimensions[0].name, "n");
        assert_eq!(header.variables[1].name, "int64_var");
        assert_eq!(header.variables[1].dtype, NcType::Int64);
        assert_eq!(header.variables[1].data_offset, 256);
    }

    #[test]
    fn unlimited_dimension_size_tracks_numrecs() {
        let data = build_cdf1_header(
            &[("time", 0), ("x", 5)],
            &[],
            &[("series", &[0, 1], 6, 40, 128)],
            3,
        );

        let header = parse_header(&data, NcFormat::Classic).unwrap();
        assert_eq!(header.dimensions[0].size, 3);
        assert_eq!(header.variables[0].shape(), vec![3, 5]);
    }

    #[test]
    fn double_attribute() {
        let pi = std::f64::consts::PI;
        let value_bytes = pi.to_be_bytes();
        let data = build_cdf1_header(
            &[],
            &[("pi", 6, &value_bytes)], // NC_DOUBLE = 6
            &[],
            0,
        );
        let header = parse_header(&data, NcFormat::Classic).unwrap();
        assert_eq!(header.global_attributes.len(), 1);
        if let NcAttrValue::Doubles(ref v) = header.global_attributes[0].value {
            assert_eq!(v.len(), 1);
            assert!((v[0] - pi).abs() < 1e-15);
        } else {
            panic!("expected Doubles attribute");
        }
    }

    #[test]
    fn short_attribute_with_padding() {
        // NC_SHORT (2 bytes) with 3 values = 6 bytes, padded to 8.
        let mut value_bytes = Vec::new();
        value_bytes.extend_from_slice(&1i16.to_be_bytes());
        value_bytes.extend_from_slice(&2i16.to_be_bytes());
        value_bytes.extend_from_slice(&3i16.to_be_bytes());
        // The build helper will add padding.

        let mut buf = Vec::new();
        buf.extend_from_slice(b"CDF\x01");
        buf.extend_from_slice(&0u32.to_be_bytes()); // numrecs
                                                    // dim_list: absent
        buf.extend_from_slice(&ABSENT.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        // att_list: one short attribute with 3 values
        buf.extend_from_slice(&NC_ATTRIBUTE.to_be_bytes());
        buf.extend_from_slice(&1u32.to_be_bytes());
        write_name_cdf1(&mut buf, "vals");
        buf.extend_from_slice(&3u32.to_be_bytes()); // NC_SHORT
        buf.extend_from_slice(&3u32.to_be_bytes()); // nvalues=3
        buf.extend_from_slice(&value_bytes);
        // Pad to 4-byte boundary: 6 bytes -> 2 bytes padding
        buf.extend_from_slice(&[0, 0]);
        // var_list: absent
        buf.extend_from_slice(&ABSENT.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());

        let header = parse_header(&buf, NcFormat::Classic).unwrap();
        if let NcAttrValue::Shorts(ref v) = header.global_attributes[0].value {
            assert_eq!(v, &[1, 2, 3]);
        } else {
            panic!("expected Shorts attribute");
        }
    }

    #[test]
    fn name_padding() {
        // Names with lengths 1, 2, 3, 4, 5 to test all padding cases.
        let data = build_cdf1_header(
            &[("a", 1), ("ab", 2), ("abc", 3), ("abcd", 4), ("abcde", 5)],
            &[],
            &[],
            0,
        );
        let header = parse_header(&data, NcFormat::Classic).unwrap();
        assert_eq!(header.dimensions.len(), 5);
        assert_eq!(header.dimensions[0].name, "a");
        assert_eq!(header.dimensions[1].name, "ab");
        assert_eq!(header.dimensions[2].name, "abc");
        assert_eq!(header.dimensions[3].name, "abcd");
        assert_eq!(header.dimensions[4].name, "abcde");
    }

    #[test]
    fn invalid_dimension_reference() {
        // Variable referencing a non-existent dimension.
        let data = build_cdf1_header(
            &[("x", 10)], // only dim 0 exists
            &[],
            &[("bad_var", &[5], 4, 40, 100)], // dimid=5 is out of range
            0,
        );
        let result = parse_header(&data, NcFormat::Classic);
        assert!(result.is_err());
    }

    #[test]
    fn byte_attribute() {
        let value_bytes: &[u8] = &[0xFF]; // -1 as i8
        let data = build_cdf1_header(
            &[],
            &[("flag", 1, value_bytes)], // NC_BYTE = 1
            &[],
            0,
        );
        let header = parse_header(&data, NcFormat::Classic).unwrap();
        if let NcAttrValue::Bytes(ref v) = header.global_attributes[0].value {
            assert_eq!(v, &[-1i8]);
        } else {
            panic!("expected Bytes attribute");
        }
    }

    #[test]
    fn float_attribute() {
        let val = std::f32::consts::PI;
        let value_bytes = val.to_be_bytes();
        let data = build_cdf1_header(
            &[],
            &[("pi_approx", 5, &value_bytes)], // NC_FLOAT = 5
            &[],
            0,
        );
        let header = parse_header(&data, NcFormat::Classic).unwrap();
        if let NcAttrValue::Floats(ref v) = header.global_attributes[0].value {
            assert_eq!(v.len(), 1);
            assert!((v[0] - std::f32::consts::PI).abs() < 1e-6);
        } else {
            panic!("expected Floats attribute");
        }
    }

    #[test]
    fn multiple_global_attributes() {
        let int_val = 100i32.to_be_bytes();
        let float_val = 2.5f32.to_be_bytes();
        let data = build_cdf1_header(
            &[],
            &[("count", 4, &int_val), ("scale", 5, &float_val)],
            &[],
            0,
        );
        let header = parse_header(&data, NcFormat::Classic).unwrap();
        assert_eq!(header.global_attributes.len(), 2);
        assert_eq!(header.global_attributes[0].name, "count");
        assert_eq!(header.global_attributes[1].name, "scale");
    }
}
