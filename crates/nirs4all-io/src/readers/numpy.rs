use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Provenance, Result, SignalType, SourceFile,
    SpectralArray, SpectralAxis, SpectralRecord,
};
use serde_json::json;
use zip::ZipArchive;

use crate::Reader;

pub struct NumpyReader;

impl Reader for NumpyReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::numpy"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        if ext == "npy" && head.starts_with(b"\x93NUMPY") {
            return Some(FormatProbe::new(
                "numpy-npy",
                self.name(),
                Confidence::Definite,
                "NumPy NPY magic detected",
            ));
        }
        if ext == "npz" && head.starts_with(b"PK\x03\x04") {
            return Some(FormatProbe::new(
                "numpy-npz",
                self.name(),
                Confidence::Definite,
                "NumPy NPZ ZIP container detected",
            ));
        }
        None
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let source = SourceFile::from_bytes(path, &bytes, "primary");
        match path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "npy" => read_npy_file(self.name(), source, &bytes),
            "npz" => read_npz_file(self.name(), source, &bytes),
            _ => Err(Error::UnsupportedFormat {
                path: path.to_path_buf(),
            }),
        }
    }
}

fn read_npy_file(reader: &str, source: SourceFile, bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
    let array = parse_npy(bytes)?;
    let matrix = numeric_matrix(&array)?;
    let cols = matrix
        .first()
        .map(Vec::len)
        .ok_or_else(|| Error::InvalidRecord("NumPy NPY matrix is empty".to_string()))?;
    let axis_values = (0..cols).map(|value| value as f64).collect::<Vec<_>>();
    records_from_matrix(
        "numpy-npy",
        reader,
        source,
        matrix,
        AxisSpec {
            values: axis_values,
            unit: "index".to_string(),
            kind: AxisKind::Index,
        },
        None,
        None,
        NumpyMetadata {
            container: "npy",
            shape: array.shape,
        },
        vec!["numpy_npy_axis_generated_index".to_string()],
    )
}

fn read_npz_file(reader: &str, source: SourceFile, bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| Error::InvalidRecord(format!("NumPy NPZ ZIP container error: {error}")))?;
    let names = archive
        .file_names()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let x_name = find_member(&names, "X.npy").ok_or_else(|| {
        Error::InvalidRecord("NumPy NPZ missing canonical X.npy spectra array".to_string())
    })?;
    let x_array = parse_npy(&read_zip_member(&mut archive, &x_name)?)?;
    let matrix = numeric_matrix(&x_array)?;
    let row_count = matrix.len();
    let col_count = matrix
        .first()
        .map(Vec::len)
        .ok_or_else(|| Error::InvalidRecord("NumPy NPZ X matrix is empty".to_string()))?;

    let mut warnings = Vec::new();
    let axis = if let Some(name) = find_member(&names, "wavelengths.npy") {
        let wavelengths = numeric_vector(&parse_npy(&read_zip_member(&mut archive, &name)?)?)?;
        if wavelengths.len() != col_count {
            return Err(Error::InvalidRecord(format!(
                "NumPy NPZ wavelengths length {} does not match X columns {col_count}",
                wavelengths.len()
            )));
        }
        AxisSpec {
            values: wavelengths,
            unit: "nm".to_string(),
            kind: AxisKind::Wavelength,
        }
    } else {
        warnings.push("numpy_npz_axis_generated_index".to_string());
        AxisSpec {
            values: (0..col_count).map(|value| value as f64).collect(),
            unit: "index".to_string(),
            kind: AxisKind::Index,
        }
    };

    let targets = if let Some(name) = find_member(&names, "y.npy") {
        let values = numeric_vector(&parse_npy(&read_zip_member(&mut archive, &name)?)?)?;
        if values.len() != row_count {
            return Err(Error::InvalidRecord(format!(
                "NumPy NPZ y length {} does not match X rows {row_count}",
                values.len()
            )));
        }
        Some(values)
    } else {
        None
    };

    let sample_ids = if let Some(name) = find_member(&names, "sample_ids.npy") {
        let values = string_vector(&parse_npy(&read_zip_member(&mut archive, &name)?)?)?;
        if values.len() != row_count {
            return Err(Error::InvalidRecord(format!(
                "NumPy NPZ sample_ids length {} does not match X rows {row_count}",
                values.len()
            )));
        }
        Some(values)
    } else {
        None
    };

    records_from_matrix(
        "numpy-npz",
        reader,
        source,
        matrix,
        axis,
        sample_ids,
        targets,
        NumpyMetadata {
            container: "npz",
            shape: x_array.shape,
        },
        warnings,
    )
}

fn find_member(names: &[String], basename: &str) -> Option<String> {
    names
        .iter()
        .find(|name| name.rsplit('/').next() == Some(basename))
        .cloned()
}

fn read_zip_member(archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>> {
    let mut file = archive
        .by_name(name)
        .map_err(|error| Error::InvalidRecord(format!("NumPy NPZ member {name} error: {error}")))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|source| Error::Io {
        path: name.into(),
        source,
    })?;
    Ok(bytes)
}

struct AxisSpec {
    values: Vec<f64>,
    unit: String,
    kind: AxisKind,
}

struct NumpyMetadata {
    container: &'static str,
    shape: Vec<usize>,
}

#[allow(clippy::too_many_arguments)]
fn records_from_matrix(
    format: &str,
    reader: &str,
    source: SourceFile,
    matrix: Vec<Vec<f64>>,
    axis: AxisSpec,
    sample_ids: Option<Vec<String>>,
    targets: Option<Vec<f64>>,
    numpy: NumpyMetadata,
    warnings: Vec<String>,
) -> Result<Vec<SpectralRecord>> {
    let row_count = matrix.len();
    let col_count = matrix
        .first()
        .map(Vec::len)
        .ok_or_else(|| Error::InvalidRecord("NumPy spectra matrix is empty".to_string()))?;
    if axis.values.len() != col_count {
        return Err(Error::InvalidRecord(format!(
            "NumPy axis length {} does not match matrix columns {col_count}",
            axis.values.len()
        )));
    }

    matrix
        .into_iter()
        .enumerate()
        .map(|(row_index, row)| {
            if row.len() != col_count {
                return Err(Error::InvalidRecord(
                    "NumPy matrix rows have inconsistent lengths".to_string(),
                ));
            }
            let sample_id = sample_ids
                .as_ref()
                .and_then(|values| values.get(row_index))
                .cloned()
                .unwrap_or_else(|| format!("row_{row_index}"));
            let axis =
                SpectralAxis::new(axis.values.clone(), axis.unit.clone(), axis.kind.clone())?;
            let signal = SpectralArray::new(
                axis,
                row,
                vec!["x".to_string()],
                SignalType::Unknown,
                None,
                "spectrum",
                "file",
            )?;
            let mut signals = BTreeMap::new();
            signals.insert("spectrum".to_string(), signal);

            let mut target_map = BTreeMap::new();
            if let Some(values) = &targets {
                target_map.insert("y".to_string(), json!(values[row_index]));
            }

            let mut metadata = BTreeMap::new();
            metadata.insert("sample_id".to_string(), json!(sample_id));
            metadata.insert(
                "numpy".to_string(),
                json!({
                    "container": numpy.container,
                    "shape": &numpy.shape,
                    "row_index": row_index,
                    "row_count": row_count,
                    "column_count": col_count,
                }),
            );
            let record = SpectralRecord {
                signals,
                signal_type: SignalType::Unknown,
                targets: target_map,
                metadata,
                provenance: Provenance {
                    format: format.to_string(),
                    reader: reader.to_string(),
                    reader_version: env!("CARGO_PKG_VERSION").to_string(),
                    sources: vec![source.clone()],
                    parsed_at_utc: None,
                    record_schema_version: "0.1.0".to_string(),
                    warnings: warnings.clone(),
                },
                quality_flags: Vec::new(),
            };
            record.validate()?;
            Ok(record)
        })
        .collect()
}

struct NpyArray {
    shape: Vec<usize>,
    data: NpyData,
}

enum NpyData {
    Numeric(Vec<f64>),
    Strings(Vec<String>),
}

fn parse_npy(bytes: &[u8]) -> Result<NpyArray> {
    if bytes.len() < 10 || !bytes.starts_with(b"\x93NUMPY") {
        return Err(Error::InvalidRecord("NumPy NPY magic missing".to_string()));
    }
    let major = bytes[6];
    let header_len_offset = 8;
    let (header_len, data_offset) = match major {
        1 => {
            let len = u16::from_le_bytes(
                bytes[header_len_offset..header_len_offset + 2]
                    .try_into()
                    .expect("header length"),
            ) as usize;
            (len, 10)
        }
        2 | 3 => {
            if bytes.len() < 12 {
                return Err(Error::InvalidRecord(
                    "NumPy NPY v2/v3 header is truncated".to_string(),
                ));
            }
            let len = u32::from_le_bytes(
                bytes[header_len_offset..header_len_offset + 4]
                    .try_into()
                    .expect("header length"),
            ) as usize;
            (len, 12)
        }
        _ => {
            return Err(Error::InvalidRecord(format!(
                "NumPy NPY version {major} is not supported"
            )))
        }
    };
    if bytes.len() < data_offset + header_len {
        return Err(Error::InvalidRecord(
            "NumPy NPY header is truncated".to_string(),
        ));
    }
    let header = String::from_utf8_lossy(&bytes[data_offset..data_offset + header_len]);
    let descr = parse_header_string(&header, "descr")?;
    let fortran_order = parse_header_bool(&header, "fortran_order")?;
    if fortran_order {
        return Err(Error::InvalidRecord(
            "NumPy Fortran-order arrays are not supported yet".to_string(),
        ));
    }
    let shape = parse_header_shape(&header)?;
    let data = decode_npy_data(&descr, &shape, &bytes[data_offset + header_len..])?;
    Ok(NpyArray { shape, data })
}

fn parse_header_string(header: &str, key: &str) -> Result<String> {
    let raw = parse_header_value(header, key)?;
    Ok(raw.trim().trim_matches('\'').trim_matches('"').to_string())
}

fn parse_header_bool(header: &str, key: &str) -> Result<bool> {
    match parse_header_value(header, key)?.trim() {
        "True" => Ok(true),
        "False" => Ok(false),
        value => Err(Error::InvalidRecord(format!(
            "invalid NumPy header bool for {key}: {value}"
        ))),
    }
}

fn parse_header_shape(header: &str) -> Result<Vec<usize>> {
    let value = parse_header_value(header, "shape")?;
    let trimmed = value.trim().trim_start_matches('(').trim_end_matches(')');
    let shape = trimmed
        .split(',')
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                None
            } else {
                Some(part.parse::<usize>())
            }
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| Error::InvalidRecord(format!("invalid NumPy shape: {value}")))?;
    if shape.is_empty() {
        return Err(Error::InvalidRecord("NumPy shape is empty".to_string()));
    }
    Ok(shape)
}

fn parse_header_value(header: &str, key: &str) -> Result<String> {
    let needle = format!("'{key}':");
    let start = header
        .find(&needle)
        .ok_or_else(|| Error::InvalidRecord(format!("NumPy header missing {key}")))?
        + needle.len();
    let mut value = String::new();
    let mut quote: Option<char> = None;
    let mut depth = 0usize;
    for ch in header[start..].chars() {
        if let Some(active) = quote {
            value.push(ch);
            if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => {
                quote = Some(ch);
                value.push(ch);
            }
            '(' | '[' | '{' => {
                depth += 1;
                value.push(ch);
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                value.push(ch);
            }
            ',' if depth == 0 => break,
            _ => value.push(ch),
        }
    }
    Ok(value.trim().to_string())
}

fn decode_npy_data(descr: &str, shape: &[usize], payload: &[u8]) -> Result<NpyData> {
    let count = shape
        .iter()
        .try_fold(1usize, |acc, value| acc.checked_mul(*value))
        .ok_or_else(|| Error::InvalidRecord("NumPy shape overflows".to_string()))?;
    let dtype = DType::parse(descr)?;
    match dtype {
        DType::String {
            width,
            unicode,
            endian,
        } => {
            let bytes_per_value = if unicode { width * 4 } else { width };
            let expected = count * bytes_per_value;
            if payload.len() < expected {
                return Err(Error::InvalidRecord(format!(
                    "NumPy string payload has {} bytes; expected {expected}",
                    payload.len()
                )));
            }
            let values = payload[..expected]
                .chunks_exact(bytes_per_value)
                .map(|chunk| {
                    if unicode {
                        decode_unicode_string(chunk, endian)
                    } else {
                        decode_byte_string(chunk)
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(NpyData::Strings(values))
        }
        DType::Numeric {
            kind,
            width,
            endian,
        } => {
            let expected = count * width;
            if payload.len() < expected {
                return Err(Error::InvalidRecord(format!(
                    "NumPy numeric payload has {} bytes; expected {expected}",
                    payload.len()
                )));
            }
            let mut values = Vec::with_capacity(count);
            for chunk in payload[..expected].chunks_exact(width) {
                values.push(decode_numeric_value(kind, width, endian, chunk)?);
            }
            Ok(NpyData::Numeric(values))
        }
    }
}

#[derive(Clone, Copy)]
enum Endian {
    Little,
    Big,
    NotApplicable,
}

enum DType {
    Numeric {
        kind: char,
        width: usize,
        endian: Endian,
    },
    String {
        width: usize,
        unicode: bool,
        endian: Endian,
    },
}

impl DType {
    fn parse(descr: &str) -> Result<Self> {
        let mut chars = descr.chars();
        let first = chars
            .next()
            .ok_or_else(|| Error::InvalidRecord("NumPy dtype is empty".to_string()))?;
        let (endian, body) = match first {
            '<' | '=' => (Endian::Little, chars.as_str()),
            '>' => (Endian::Big, chars.as_str()),
            '|' => (Endian::NotApplicable, chars.as_str()),
            _ => (Endian::NotApplicable, descr),
        };
        let mut body_chars = body.chars();
        let kind = body_chars
            .next()
            .ok_or_else(|| Error::InvalidRecord(format!("invalid NumPy dtype: {descr}")))?;
        let width = body_chars
            .as_str()
            .parse::<usize>()
            .map_err(|_| Error::InvalidRecord(format!("invalid NumPy dtype width: {descr}")))?;
        match kind {
            'f' | 'i' | 'u' => Ok(DType::Numeric {
                kind,
                width,
                endian,
            }),
            'S' => Ok(DType::String {
                width,
                unicode: false,
                endian,
            }),
            'U' => Ok(DType::String {
                width,
                unicode: true,
                endian,
            }),
            _ => Err(Error::InvalidRecord(format!(
                "NumPy dtype {descr} is not supported"
            ))),
        }
    }
}

fn decode_numeric_value(kind: char, width: usize, endian: Endian, chunk: &[u8]) -> Result<f64> {
    let little = !matches!(endian, Endian::Big);
    match (kind, width, little) {
        ('f', 4, true) => Ok(f32::from_le_bytes(chunk.try_into().expect("width")) as f64),
        ('f', 4, false) => Ok(f32::from_be_bytes(chunk.try_into().expect("width")) as f64),
        ('f', 8, true) => Ok(f64::from_le_bytes(chunk.try_into().expect("width"))),
        ('f', 8, false) => Ok(f64::from_be_bytes(chunk.try_into().expect("width"))),
        ('i', 1, _) => Ok(i8::from_ne_bytes(chunk.try_into().expect("width")) as f64),
        ('i', 2, true) => Ok(i16::from_le_bytes(chunk.try_into().expect("width")) as f64),
        ('i', 2, false) => Ok(i16::from_be_bytes(chunk.try_into().expect("width")) as f64),
        ('i', 4, true) => Ok(i32::from_le_bytes(chunk.try_into().expect("width")) as f64),
        ('i', 4, false) => Ok(i32::from_be_bytes(chunk.try_into().expect("width")) as f64),
        ('i', 8, true) => Ok(i64::from_le_bytes(chunk.try_into().expect("width")) as f64),
        ('i', 8, false) => Ok(i64::from_be_bytes(chunk.try_into().expect("width")) as f64),
        ('u', 1, _) => Ok(u8::from_ne_bytes(chunk.try_into().expect("width")) as f64),
        ('u', 2, true) => Ok(u16::from_le_bytes(chunk.try_into().expect("width")) as f64),
        ('u', 2, false) => Ok(u16::from_be_bytes(chunk.try_into().expect("width")) as f64),
        ('u', 4, true) => Ok(u32::from_le_bytes(chunk.try_into().expect("width")) as f64),
        ('u', 4, false) => Ok(u32::from_be_bytes(chunk.try_into().expect("width")) as f64),
        ('u', 8, true) => Ok(u64::from_le_bytes(chunk.try_into().expect("width")) as f64),
        ('u', 8, false) => Ok(u64::from_be_bytes(chunk.try_into().expect("width")) as f64),
        _ => Err(Error::InvalidRecord(format!(
            "NumPy numeric dtype {kind}{width} is not supported"
        ))),
    }
}

fn decode_unicode_string(chunk: &[u8], endian: Endian) -> Result<String> {
    let mut out = String::new();
    for item in chunk.chunks_exact(4) {
        let codepoint = match endian {
            Endian::Big => u32::from_be_bytes(item.try_into().expect("unicode width")),
            Endian::Little | Endian::NotApplicable => {
                u32::from_le_bytes(item.try_into().expect("unicode width"))
            }
        };
        if codepoint == 0 {
            continue;
        }
        let ch = char::from_u32(codepoint).ok_or_else(|| {
            Error::InvalidRecord(format!("invalid NumPy unicode codepoint: {codepoint}"))
        })?;
        out.push(ch);
    }
    Ok(out.trim().to_string())
}

fn decode_byte_string(chunk: &[u8]) -> Result<String> {
    Ok(String::from_utf8_lossy(chunk)
        .trim_matches(char::from(0))
        .trim()
        .to_string())
}

fn numeric_matrix(array: &NpyArray) -> Result<Vec<Vec<f64>>> {
    let NpyData::Numeric(values) = &array.data else {
        return Err(Error::InvalidRecord(
            "NumPy spectra array is not numeric".to_string(),
        ));
    };
    match array.shape.as_slice() {
        [cols] => Ok(vec![values[..*cols].to_vec()]),
        [rows, cols] => Ok((0..*rows)
            .map(|row| {
                let start = row * cols;
                values[start..start + cols].to_vec()
            })
            .collect()),
        shape => Err(Error::InvalidRecord(format!(
            "NumPy spectra array shape {shape:?} is not 1D or 2D"
        ))),
    }
}

fn numeric_vector(array: &NpyArray) -> Result<Vec<f64>> {
    let NpyData::Numeric(values) = &array.data else {
        return Err(Error::InvalidRecord(
            "NumPy vector array is not numeric".to_string(),
        ));
    };
    match array.shape.as_slice() {
        [_] => Ok(values.clone()),
        shape => Err(Error::InvalidRecord(format!(
            "NumPy vector shape {shape:?} is not 1D"
        ))),
    }
}

fn string_vector(array: &NpyArray) -> Result<Vec<String>> {
    let NpyData::Strings(values) = &array.data else {
        return Err(Error::InvalidRecord(
            "NumPy string array is not textual".to_string(),
        ));
    };
    match array.shape.as_slice() {
        [_] => Ok(values.clone()),
        shape => Err(Error::InvalidRecord(format!(
            "NumPy string vector shape {shape:?} is not 1D"
        ))),
    }
}
