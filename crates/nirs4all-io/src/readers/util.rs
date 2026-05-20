use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Provenance, Result, SignalType, SourceFile, SpectralArray, SpectralAxis,
    SpectralRecord,
};
use serde_json::{json, Value};

pub fn read_text_lossy(path: &Path) -> Result<(String, SourceFile)> {
    let bytes = std::fs::read(path).map_err(|source| nirs4all_io_core::Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let source = SourceFile::from_bytes(path, &bytes, "primary");
    Ok((String::from_utf8_lossy(&bytes).replace('\0', " "), source))
}

pub fn metadata_from_pairs(pairs: Vec<(String, String)>) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    let mut vendor = BTreeMap::new();
    for (key, value) in pairs {
        vendor.insert(normalize_key(&key), json!(value.trim()));
    }
    metadata.insert("vendor".to_string(), json!(vendor));
    metadata
}

pub fn provenance(
    format: &str,
    reader: &str,
    source: SourceFile,
    warnings: Vec<String>,
) -> Provenance {
    Provenance {
        format: format.to_string(),
        reader: reader.to_string(),
        reader_version: env!("CARGO_PKG_VERSION").to_string(),
        sources: vec![source],
        parsed_at_utc: None,
        record_schema_version: "0.1.0".to_string(),
        warnings,
    }
}

pub struct SingleSignalSpec {
    pub axis_values: Vec<f64>,
    pub axis_unit: String,
    pub axis_kind: AxisKind,
    pub values: Vec<f64>,
    pub signal_name: String,
    pub signal_type: SignalType,
    pub signal_unit: Option<String>,
    pub role: String,
}

pub fn single_signal_record(
    format: &str,
    reader: &str,
    source: SourceFile,
    signal_spec: SingleSignalSpec,
    targets: BTreeMap<String, Value>,
    metadata: BTreeMap<String, Value>,
    warnings: Vec<String>,
) -> Result<SpectralRecord> {
    let axis = SpectralAxis::new(
        signal_spec.axis_values,
        signal_spec.axis_unit,
        signal_spec.axis_kind,
    )?;
    let signal = SpectralArray::new(
        axis,
        signal_spec.values,
        vec!["x".to_string()],
        signal_spec.signal_type.clone(),
        signal_spec.signal_unit,
        signal_spec.role,
        "file",
    )?;
    let mut signals = BTreeMap::new();
    signals.insert(signal_spec.signal_name, signal);
    let record = SpectralRecord {
        signals,
        signal_type: signal_spec.signal_type,
        targets,
        metadata,
        provenance: provenance(format, reader, source, warnings),
        quality_flags: Vec::new(),
    };
    record.validate()?;
    Ok(record)
}

pub fn record_from_signals(
    format: &str,
    reader: &str,
    source: SourceFile,
    signals: BTreeMap<String, SpectralArray>,
    dominant: SignalType,
    metadata: BTreeMap<String, Value>,
    warnings: Vec<String>,
) -> Result<SpectralRecord> {
    let record = SpectralRecord {
        signals,
        signal_type: dominant,
        targets: BTreeMap::new(),
        metadata,
        provenance: provenance(format, reader, source, warnings),
        quality_flags: Vec::new(),
    };
    record.validate()?;
    Ok(record)
}

pub fn parse_number(value: &str) -> Option<f64> {
    let normalized = value.trim().trim_matches('"').replace(',', ".");
    if normalized.is_empty() {
        return None;
    }
    normalized.parse::<f64>().ok()
}

pub fn split_delimited(line: &str, delimiter: char) -> Vec<String> {
    line.split(delimiter)
        .map(|part| part.trim().trim_matches('"').to_string())
        .collect()
}

pub fn detect_delimiter(line: &str) -> char {
    let candidates = [',', ';', '\t'];
    candidates
        .into_iter()
        .max_by_key(|candidate| line.matches(*candidate).count())
        .unwrap_or(',')
}

pub fn normalize_key(key: &str) -> String {
    key.trim()
        .trim_matches('#')
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '.', '/', '-'], "_")
        .replace(['[', ']', '(', ')'], "")
}

pub fn safe_signal_name(name: &str, fallback: &str) -> String {
    let normalized = normalize_key(name);
    if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized
    }
}

pub fn signal_type_from_label(label: &str) -> SignalType {
    let lower = label.to_ascii_lowercase();
    if lower.contains("abs") {
        SignalType::Absorbance
    } else if lower.contains("reflect") || lower.contains("%r") {
        SignalType::Reflectance
    } else if lower.contains("trans") {
        SignalType::Transmittance
    } else if lower.contains("radiance") {
        SignalType::Radiance
    } else if lower.contains("irradiance") || lower.contains("irr") {
        SignalType::Irradiance
    } else if lower.contains("raw")
        || lower.contains("count")
        || lower.contains("sample")
        || lower.contains("dn")
    {
        SignalType::RawCounts
    } else {
        SignalType::Unknown
    }
}
