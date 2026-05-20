use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Provenance, Result, SignalType, SourceFile,
    SpectralArray, SpectralAxis, SpectralRecord,
};
use serde_json::json;

use crate::readers::util::normalize_key;
use crate::Reader;

pub struct EnviSliReader;

impl Reader for EnviSliReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::envi_sli"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        if ext == "hdr" {
            let text = String::from_utf8_lossy(head);
            return sniff_header(self.name(), &text);
        }
        if ext == "sli" {
            let header_path = path.with_extension("hdr");
            let text = std::fs::read_to_string(header_path).ok()?;
            return sniff_header(self.name(), &text);
        }
        None
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let (header_path, data_hint) = paired_paths(path)?;
        let header_bytes = std::fs::read(&header_path).map_err(|source| Error::Io {
            path: header_path.clone(),
            source,
        })?;
        let header_source = SourceFile::from_bytes(&header_path, &header_bytes, "header");
        let header_text = String::from_utf8_lossy(&header_bytes);
        let header = parse_envi_header(&header_text);

        let file_type = header_value(&header, "file type")
            .ok_or_else(|| Error::InvalidRecord("ENVI header missing file type".to_string()))?;
        if !file_type.eq_ignore_ascii_case("ENVI Spectral Library") {
            return Err(Error::InvalidRecord(format!(
                "unsupported ENVI file type '{file_type}'; only ENVI Spectral Library .sli is supported"
            )));
        }

        let samples = parse_usize(&header, "samples")?;
        let lines = parse_usize(&header, "lines")?;
        let bands = parse_usize(&header, "bands")?;
        if bands != 1 {
            return Err(Error::InvalidRecord(format!(
                "ENVI spectral library bands={bands}; only one-band SLI payloads are supported"
            )));
        }
        let interleave = header_value(&header, "interleave").unwrap_or("bsq");
        if !interleave.eq_ignore_ascii_case("bsq") {
            return Err(Error::InvalidRecord(format!(
                "ENVI SLI interleave '{interleave}' is not supported yet"
            )));
        }

        let data_type = parse_usize(&header, "data type")?;
        let byte_order = parse_usize(&header, "byte order")?;
        let header_offset = parse_optional_usize(&header, "header offset").unwrap_or(0);
        let data_path = resolve_data_path(&header_path, data_hint, &header)?;
        let data_bytes = std::fs::read(&data_path).map_err(|source| Error::Io {
            path: data_path.clone(),
            source,
        })?;
        let data_source = SourceFile::from_bytes(&data_path, &data_bytes, "binary");
        if data_bytes.len() < header_offset {
            return Err(Error::InvalidRecord(format!(
                "ENVI SLI header offset {header_offset} exceeds payload length {}",
                data_bytes.len()
            )));
        }

        let expected_values = samples
            .checked_mul(lines)
            .and_then(|value| value.checked_mul(bands))
            .ok_or_else(|| Error::InvalidRecord("ENVI SLI dimensions overflow".to_string()))?;
        let payload = &data_bytes[header_offset..];
        let values = decode_numeric_payload(payload, data_type, byte_order)?;
        if values.len() < expected_values {
            return Err(Error::InvalidRecord(format!(
                "ENVI SLI payload has {} values; expected {expected_values}",
                values.len()
            )));
        }

        let mut warnings = Vec::new();
        if values.len() > expected_values {
            warnings.push(format!(
                "envi_sli_payload_trailing_values:{}",
                values.len() - expected_values
            ));
        }

        let (axis_values, axis_unit, axis_kind) =
            axis_from_header(&header, samples, &mut warnings)?;
        let spectra_names = parse_list(header_value(&header, "spectra names").unwrap_or_default());
        let records = (0..lines)
            .map(|record_index| {
                let start = record_index * samples;
                let end = start + samples;
                let sample_id = spectra_names
                    .get(record_index)
                    .cloned()
                    .unwrap_or_else(|| format!("spectrum_{record_index}"));
                make_record(
                    self.name(),
                    header_source.clone(),
                    data_source.clone(),
                    record_index,
                    sample_id,
                    &header,
                    &axis_values,
                    &axis_unit,
                    axis_kind.clone(),
                    values[start..end].to_vec(),
                    warnings.clone(),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(records)
    }
}

fn sniff_header(reader: &'static str, text: &str) -> Option<FormatProbe> {
    if !text.trim_start().starts_with("ENVI") {
        return None;
    }
    let header = parse_envi_header(text);
    let file_type = header_value(&header, "file type")?;
    if file_type.eq_ignore_ascii_case("ENVI Spectral Library") {
        Some(FormatProbe::new(
            "envi-sli",
            reader,
            Confidence::Definite,
            "ENVI Spectral Library header detected",
        ))
    } else if file_type.eq_ignore_ascii_case("ENVI Standard") {
        Some(FormatProbe::new(
            "envi-standard-cube",
            reader,
            Confidence::Possible,
            "ENVI image cube header detected; cube loading is out of scope for v1",
        ))
    } else {
        None
    }
}

fn paired_paths(path: &Path) -> Result<(PathBuf, Option<PathBuf>)> {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "hdr" => Ok((path.to_path_buf(), None)),
        "sli" => Ok((path.with_extension("hdr"), Some(path.to_path_buf()))),
        _ => Err(Error::UnsupportedFormat {
            path: path.to_path_buf(),
        }),
    }
}

fn parse_envi_header(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut current_key: Option<String> = None;
    let mut current_value = String::new();

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.eq_ignore_ascii_case("ENVI") {
            continue;
        }

        if let Some(key) = &current_key {
            if !current_value.is_empty() {
                current_value.push('\n');
            }
            current_value.push_str(line);
            if line.contains('}') {
                out.insert(key.clone(), strip_braces(&current_value));
                current_key = None;
                current_value.clear();
            }
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = normalize_key(key);
        let value = value.trim();
        if value.starts_with('{') && !value.contains('}') {
            current_key = Some(key);
            current_value = value.to_string();
        } else {
            out.insert(key, strip_braces(value));
        }
    }

    if let Some(key) = current_key {
        out.insert(key, strip_braces(&current_value));
    }

    out
}

fn strip_braces(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        trimmed[1..trimmed.len() - 1].trim().to_string()
    } else {
        trimmed.trim_matches('"').trim().to_string()
    }
}

fn header_value<'a>(header: &'a BTreeMap<String, String>, key: &str) -> Option<&'a str> {
    header.get(&normalize_key(key)).map(String::as_str)
}

fn parse_usize(header: &BTreeMap<String, String>, key: &str) -> Result<usize> {
    let value = header_value(header, key)
        .ok_or_else(|| Error::InvalidRecord(format!("ENVI header missing {key}")))?;
    value
        .trim()
        .parse::<usize>()
        .map_err(|_| Error::InvalidRecord(format!("invalid ENVI {key}: {value}")))
}

fn parse_optional_usize(header: &BTreeMap<String, String>, key: &str) -> Option<usize> {
    header_value(header, key)?.trim().parse::<usize>().ok()
}

fn resolve_data_path(
    header_path: &Path,
    data_hint: Option<PathBuf>,
    header: &BTreeMap<String, String>,
) -> Result<PathBuf> {
    if let Some(path) = data_hint {
        return Ok(path);
    }
    if let Some(value) = header_value(header, "data file") {
        let path = Path::new(value);
        return Ok(if path.is_absolute() {
            path.to_path_buf()
        } else {
            header_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(path)
        });
    }

    for extension in ["sli", "SLI"] {
        let candidate = header_path.with_extension(extension);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(Error::InvalidRecord(format!(
        "missing ENVI SLI binary next to {}",
        header_path.display()
    )))
}

fn decode_numeric_payload(payload: &[u8], data_type: usize, byte_order: usize) -> Result<Vec<f64>> {
    let width = match data_type {
        4 => 4,
        5 => 8,
        _ => {
            return Err(Error::InvalidRecord(format!(
                "ENVI data type {data_type} is not supported for SLI yet"
            )))
        }
    };
    if !payload.len().is_multiple_of(width) {
        return Err(Error::InvalidRecord(format!(
            "ENVI payload length {} is not aligned to {width}-byte values",
            payload.len()
        )));
    }
    let big_endian = match byte_order {
        0 => false,
        1 => true,
        _ => {
            return Err(Error::InvalidRecord(format!(
                "invalid ENVI byte order: {byte_order}"
            )))
        }
    };

    let mut values = Vec::with_capacity(payload.len() / width);
    match (data_type, big_endian) {
        (4, false) => {
            for chunk in payload.chunks_exact(4) {
                values.push(f32::from_le_bytes(chunk.try_into().expect("chunk width")) as f64);
            }
        }
        (4, true) => {
            for chunk in payload.chunks_exact(4) {
                values.push(f32::from_be_bytes(chunk.try_into().expect("chunk width")) as f64);
            }
        }
        (5, false) => {
            for chunk in payload.chunks_exact(8) {
                values.push(f64::from_le_bytes(chunk.try_into().expect("chunk width")));
            }
        }
        (5, true) => {
            for chunk in payload.chunks_exact(8) {
                values.push(f64::from_be_bytes(chunk.try_into().expect("chunk width")));
            }
        }
        _ => unreachable!("data type checked above"),
    }
    Ok(values)
}

fn axis_from_header(
    header: &BTreeMap<String, String>,
    samples: usize,
    warnings: &mut Vec<String>,
) -> Result<(Vec<f64>, String, AxisKind)> {
    let (axis_unit, axis_kind) = axis_unit_kind(header_value(header, "wavelength units"));
    let Some(wavelengths) = header_value(header, "wavelength") else {
        warnings.push("envi_sli_missing_wavelength_axis_generated_index".to_string());
        return Ok((
            (0..samples).map(|value| value as f64).collect(),
            "index".to_string(),
            AxisKind::Index,
        ));
    };
    let axis_values = parse_list(wavelengths)
        .into_iter()
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| Error::InvalidRecord(format!("invalid ENVI wavelength: {value}")))
        })
        .collect::<Result<Vec<_>>>()?;
    if axis_values.len() != samples {
        return Err(Error::InvalidRecord(format!(
            "ENVI wavelength count {} does not match samples {samples}",
            axis_values.len()
        )));
    }
    Ok((axis_values, axis_unit, axis_kind))
}

fn axis_unit_kind(value: Option<&str>) -> (String, AxisKind) {
    let normalized = value.unwrap_or_default().trim().to_ascii_lowercase();
    if normalized.contains("nanometer") || normalized == "nm" {
        ("nm".to_string(), AxisKind::Wavelength)
    } else if normalized.contains("micrometer") || normalized.contains("um") {
        ("um".to_string(), AxisKind::Wavelength)
    } else if normalized.contains("wavenumber")
        || normalized.contains("cm-1")
        || normalized.contains("1/cm")
    {
        ("cm-1".to_string(), AxisKind::Wavenumber)
    } else {
        ("index".to_string(), AxisKind::Index)
    }
}

fn parse_list(value: &str) -> Vec<String> {
    value
        .lines()
        .flat_map(|line| line.split(','))
        .map(|item| item.trim().trim_matches('"').to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn make_record(
    reader: &str,
    header_source: SourceFile,
    data_source: SourceFile,
    record_index: usize,
    sample_id: String,
    header: &BTreeMap<String, String>,
    axis_values: &[f64],
    axis_unit: &str,
    axis_kind: AxisKind,
    values: Vec<f64>,
    warnings: Vec<String>,
) -> Result<SpectralRecord> {
    let axis = SpectralAxis::new(axis_values.to_vec(), axis_unit, axis_kind)?;
    let signal = SpectralArray::new(
        axis,
        values,
        vec!["x".to_string()],
        SignalType::Unknown,
        None,
        "spectrum",
        "file",
    )?;
    let mut signals = BTreeMap::new();
    signals.insert("spectrum".to_string(), signal);

    let mut metadata = BTreeMap::new();
    metadata.insert("sample_id".to_string(), json!(sample_id));
    metadata.insert(
        "envi".to_string(),
        json!({
            "record_index": record_index,
            "description": header_value(header, "description"),
            "file_type": header_value(header, "file type"),
            "samples": header_value(header, "samples").and_then(|value| value.parse::<usize>().ok()),
            "lines": header_value(header, "lines").and_then(|value| value.parse::<usize>().ok()),
            "bands": header_value(header, "bands").and_then(|value| value.parse::<usize>().ok()),
            "interleave": header_value(header, "interleave"),
            "data_type": header_value(header, "data type").and_then(|value| value.parse::<usize>().ok()),
            "byte_order": header_value(header, "byte order").and_then(|value| value.parse::<usize>().ok()),
            "wavelength_units": header_value(header, "wavelength units"),
            "sensor_type": header_value(header, "sensor type"),
        }),
    );

    let record = SpectralRecord {
        signals,
        signal_type: SignalType::Unknown,
        targets: BTreeMap::new(),
        metadata,
        provenance: Provenance {
            format: "envi-sli".to_string(),
            reader: reader.to_string(),
            reader_version: env!("CARGO_PKG_VERSION").to_string(),
            sources: vec![header_source, data_source],
            parsed_at_utc: None,
            record_schema_version: "0.1.0".to_string(),
            warnings,
        },
        quality_flags: Vec::new(),
    };
    record.validate()?;
    Ok(record)
}
