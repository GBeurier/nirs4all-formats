use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_formats_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SpectralArray, SpectralAxis,
    SpectralRecord,
};
use serde_json::{json, Value};

use crate::readers::util::{
    detect_delimiter, normalize_key, parse_number, provenance, read_bytes, split_delimited,
    text_lossy_from_bytes,
};
use crate::Reader;

pub struct ScioCsvReader;

impl Reader for ScioCsvReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::scio_csv"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        if ext != "csv" {
            return None;
        }
        let text = String::from_utf8_lossy(head);
        let first = text.lines().find(|line| !line.trim().is_empty())?;
        let first_fields = split_delimited(first, detect_delimiter(first));
        if band_columns(&first_fields, "band").len() >= 32 {
            return Some(FormatProbe::new(
                "scio-csv",
                self.name(),
                Confidence::Definite,
                "Consumer Physics SCiO app CSV with band-prefixed wavelength columns",
            ));
        }
        if text.contains("num_wavelengths") && text.contains("wavelengths_start") {
            return Some(FormatProbe::new(
                "scio-csv",
                self.name(),
                Confidence::Definite,
                "Consumer Physics SCiO developer export with spectrum/raw channel groups",
            ));
        }
        None
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let bytes = read_bytes(path)?;
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(
        &self,
        path: &Path,
        bytes: &[u8],
    ) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
        let (text, source) = text_lossy_from_bytes(path, bytes);
        if let Some(records) = read_band_export(&text, source.clone(), self.name())? {
            return Ok(records);
        }
        read_developer_export(&text, source, self.name())
    }
}

fn read_band_export(
    text: &str,
    source: nirs4all_formats_core::SourceFile,
    reader: &str,
) -> Result<Option<Vec<SpectralRecord>>> {
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let Some(header_line) = lines.next() else {
        return Ok(None);
    };
    let delimiter = detect_delimiter(header_line);
    let headers = split_delimited(header_line, delimiter);
    let band_indices = band_columns(&headers, "band");
    if band_indices.len() < 32 {
        return Ok(None);
    }

    let mut records = Vec::new();
    for (row_index, line) in lines.enumerate() {
        let cells = split_delimited(line, delimiter);
        if cells.len() < headers.len() {
            continue;
        }
        let values = values_for_indices(&cells, &band_indices)?;
        let metadata = BTreeMap::from([
            ("row_index".to_string(), json!(row_index)),
            ("layout".to_string(), json!("band_columns")),
        ]);
        records.push(build_record(
            source.clone(),
            reader,
            vec![SignalSpec {
                name: "spectrum".to_string(),
                role: "spectrum".to_string(),
                axis: axis_for_indices(&band_indices),
                values,
                signal_type: SignalType::Unknown,
                unit: None,
            }],
            metadata,
            BTreeMap::new(),
            Vec::new(),
        )?);
    }
    if records.is_empty() {
        return Err(Error::InvalidRecord(
            "SCiO band CSV contains no complete data rows".to_string(),
        ));
    }
    Ok(Some(records))
}

fn read_developer_export(
    text: &str,
    source: nirs4all_formats_core::SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    let lines = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let header_index = lines
        .iter()
        .position(|line| {
            let fields = split_delimited(line, detect_delimiter(line));
            fields
                .iter()
                .any(|field| normalize_key(field) == "sample_id")
                && !band_columns(&fields, "spectrum_").is_empty()
        })
        .ok_or_else(|| {
            Error::InvalidRecord("SCiO developer CSV missing spectrum header".to_string())
        })?;
    let delimiter = detect_delimiter(lines[header_index]);
    let headers = split_delimited(lines[header_index], delimiter);
    let type_row_index = header_index + 1;
    let data_start = if lines
        .get(type_row_index)
        .map(|line| line.trim_start().starts_with("int,"))
        .unwrap_or(false)
    {
        type_row_index + 1
    } else {
        header_index + 1
    };

    let groups = [
        (
            "spectrum",
            band_columns(&headers, "spectrum_"),
            SignalType::Reflectance,
            None,
        ),
        (
            "wr_raw",
            band_columns(&headers, "wr_raw_"),
            SignalType::RawCounts,
            Some("counts".to_string()),
        ),
        (
            "sample_raw",
            band_columns(&headers, "sample_raw_"),
            SignalType::RawCounts,
            Some("counts".to_string()),
        ),
    ];
    if groups.iter().any(|(_, indices, _, _)| indices.is_empty()) {
        return Err(Error::InvalidRecord(
            "SCiO developer CSV missing spectrum/raw channel groups".to_string(),
        ));
    }

    let preamble = preamble_metadata(&lines[..header_index]);
    let spectral_columns = groups
        .iter()
        .flat_map(|(_, indices, _, _)| indices.iter().map(|(index, _)| *index))
        .collect::<Vec<_>>();
    let mut records = Vec::new();
    for (row_offset, line) in lines.iter().skip(data_start).enumerate() {
        let cells = split_delimited(line, delimiter);
        if cells.len() < headers.len() {
            continue;
        }
        let mut signals = Vec::new();
        for (name, indices, signal_type, unit) in &groups {
            signals.push(SignalSpec {
                name: (*name).to_string(),
                role: (*name).to_string(),
                axis: axis_for_indices(indices),
                values: values_for_indices(&cells, indices)?,
                signal_type: signal_type.clone(),
                unit: unit.clone(),
            });
        }
        let (metadata, targets) =
            row_metadata_and_targets(&headers, &cells, &spectral_columns, &preamble, row_offset);
        records.push(build_record(
            source.clone(),
            reader,
            signals,
            metadata,
            targets,
            Vec::new(),
        )?);
    }
    if records.is_empty() {
        return Err(Error::InvalidRecord(
            "SCiO developer CSV contains no complete data rows".to_string(),
        ));
    }
    Ok(records)
}

struct SignalSpec {
    name: String,
    role: String,
    axis: Vec<f64>,
    values: Vec<f64>,
    signal_type: SignalType,
    unit: Option<String>,
}

fn build_record(
    source: nirs4all_formats_core::SourceFile,
    reader: &str,
    signal_specs: Vec<SignalSpec>,
    metadata: BTreeMap<String, Value>,
    targets: BTreeMap<String, Value>,
    warnings: Vec<String>,
) -> Result<SpectralRecord> {
    let mut signals = BTreeMap::new();
    let mut dominant = SignalType::Unknown;
    for spec in signal_specs {
        if dominant == SignalType::Unknown {
            dominant = spec.signal_type.clone();
        }
        let axis = SpectralAxis::new(spec.axis, "nm", AxisKind::Wavelength)?;
        let signal = SpectralArray::new(
            axis,
            spec.values,
            vec!["x".to_string()],
            spec.signal_type,
            spec.unit,
            spec.role,
            "file",
        )?;
        signals.insert(spec.name, signal);
    }
    let record = SpectralRecord {
        signals,
        signal_type: dominant,
        targets,
        metadata,
        provenance: provenance("scio-csv", reader, source, warnings),
        quality_flags: Vec::new(),
    };
    record.validate()?;
    Ok(record)
}

fn band_columns(headers: &[String], prefix: &str) -> Vec<(usize, f64)> {
    headers
        .iter()
        .enumerate()
        .filter_map(|(index, header)| {
            let normalized = header.trim().trim_matches('"').trim().to_ascii_lowercase();
            normalized
                .strip_prefix(prefix)
                .and_then(parse_number)
                .map(|wavelength| (index, wavelength))
        })
        .collect()
}

fn axis_for_indices(indices: &[(usize, f64)]) -> Vec<f64> {
    indices.iter().map(|(_, wavelength)| *wavelength).collect()
}

fn values_for_indices(cells: &[String], indices: &[(usize, f64)]) -> Result<Vec<f64>> {
    indices
        .iter()
        .map(|(index, _)| {
            cells
                .get(*index)
                .and_then(|cell| parse_number(cell))
                .ok_or_else(|| {
                    Error::InvalidRecord("SCiO CSV contains non-numeric signal value".to_string())
                })
        })
        .collect()
}

fn preamble_metadata(lines: &[&str]) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    for line in lines {
        let cells = split_delimited(line, detect_delimiter(line));
        let key = cells
            .first()
            .map(|value| normalize_key(value))
            .unwrap_or_default();
        if key.is_empty() {
            continue;
        }
        let Some(value) = cells
            .get(1)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if let Some(number) = parse_number(value) {
            metadata.insert(key, json!(number));
        } else {
            metadata.insert(key, json!(value));
        }
    }
    metadata
}

fn row_metadata_and_targets(
    headers: &[String],
    cells: &[String],
    spectral_columns: &[usize],
    preamble: &BTreeMap<String, Value>,
    row_index: usize,
) -> (BTreeMap<String, Value>, BTreeMap<String, Value>) {
    let mut metadata = preamble.clone();
    let mut targets = BTreeMap::new();
    metadata.insert("row_index".to_string(), json!(row_index));
    for (index, header) in headers.iter().enumerate() {
        if spectral_columns.contains(&index) {
            continue;
        }
        let key = normalize_key(header);
        if key.is_empty() {
            continue;
        }
        let Some(value) = cells
            .get(index)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if matches!(key.as_str(), "protein" | "fat") {
            if let Some(number) = parse_number(value) {
                targets.insert(key, json!(number));
            }
            continue;
        }
        if let Some(number) = parse_number(value) {
            metadata.insert(key, json!(number));
        } else {
            metadata.insert(key, json!(value));
        }
    }
    (metadata, targets)
}
