use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SpectralArray, SpectralAxis,
};

use crate::readers::util::{
    metadata_from_pairs, parse_number, read_text_lossy, record_from_signals, safe_signal_name,
};
use crate::Reader;

pub struct OceanOpticsReader;

impl Reader for OceanOpticsReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::ocean_optics"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let text = String::from_utf8_lossy(head);
        let normalized = text.replace('\r', "\n");
        if normalized.contains("SpectraSuite Data File")
            || normalized.contains("OOIBase32 Version")
            || normalized.contains("Jaz Data File")
            || normalized.contains("Jaz Absolute Irradiance File")
            || (normalized.contains("Data from ") && normalized.contains("Begin Spectral Data"))
        {
            return Some(FormatProbe::new(
                "ocean-optics-text",
                self.name(),
                Confidence::Definite,
                "Ocean Optics/OceanView ASCII export detected",
            ));
        }
        if normalized.contains("SciMode:") && normalized.lines().any(is_numeric_pair_line) {
            return Some(FormatProbe::new(
                "ocean-optics-craic-text",
                self.name(),
                Confidence::Likely,
                "CRAIC/Ocean-style two-column text export detected",
            ));
        }
        if ext == "csv"
            && normalized
                .lines()
                .take(10)
                .filter(|line| is_numeric_pair_line(line))
                .count()
                >= 5
        {
            return Some(FormatProbe::new(
                "ocean-optics-two-column-csv",
                self.name(),
                Confidence::Likely,
                "two-column spectral CSV export detected",
            ));
        }
        None
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let (text, source) = read_text_lossy(path)?;
        let parsed = parse_ocean_text(&text, path)?;
        let signals = signals_from_columns(&parsed)?;
        let dominant = dominant_signal_type(&signals);
        let record = record_from_signals(
            parsed.format,
            self.name(),
            source,
            signals,
            dominant,
            metadata_from_pairs(parsed.metadata_pairs),
            parsed.warnings,
        )?;
        Ok(vec![record])
    }
}

struct ParsedOceanText {
    format: &'static str,
    metadata_pairs: Vec<(String, String)>,
    column_labels: Vec<String>,
    rows: Vec<Vec<f64>>,
    warnings: Vec<String>,
}

fn parse_ocean_text(text: &str, path: &Path) -> Result<ParsedOceanText> {
    let normalized = text.replace('\r', "\n");
    let mut metadata_pairs = Vec::new();
    if let Some(file_name) = path.file_name().and_then(|value| value.to_str()) {
        metadata_pairs.push(("file_name".to_string(), file_name.to_string()));
    }
    let mut rows = Vec::new();
    let mut column_labels = Vec::new();
    let mut in_data = false;
    let mut saw_begin_marker = false;

    for raw_line in normalized.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line == "++++++++++++++++++++++++++++++++++++" {
            continue;
        }
        if line.contains(">>>>>Begin") && line.contains("Data<<<<<") {
            in_data = true;
            saw_begin_marker = true;
            continue;
        }
        if in_data {
            if let Some(numbers) = parse_numeric_row(line) {
                rows.push(numbers);
            } else if rows.is_empty() && column_labels.is_empty() {
                column_labels = split_fields(line);
            }
            continue;
        }
        if let Some(numbers) = parse_numeric_row(line) {
            in_data = true;
            rows.push(numbers);
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            metadata_pairs.push((key.to_string(), value.trim().to_string()));
        } else {
            metadata_pairs.push(("header".to_string(), line.to_string()));
        }
    }

    if rows.is_empty() {
        return Err(Error::InvalidRecord(
            "Ocean Optics text export contains no numeric rows".to_string(),
        ));
    }
    let width = rows[0].len();
    rows.retain(|row| row.len() == width);
    if width < 2 {
        return Err(Error::InvalidRecord(
            "Ocean Optics text export needs at least x and y columns".to_string(),
        ));
    }
    if column_labels.len() != width {
        column_labels = default_column_labels(width);
    }
    let format = if metadata_pairs
        .iter()
        .any(|(_, value)| value.contains("CRAIC"))
        || metadata_pairs
            .iter()
            .any(|(_, value)| value.eq_ignore_ascii_case("Reflectance"))
    {
        "ocean-optics-craic-text"
    } else if saw_begin_marker {
        "ocean-optics-text"
    } else {
        "ocean-optics-two-column-csv"
    };

    Ok(ParsedOceanText {
        format,
        metadata_pairs,
        column_labels,
        rows,
        warnings: Vec::new(),
    })
}

fn signals_from_columns(parsed: &ParsedOceanText) -> Result<BTreeMap<String, SpectralArray>> {
    let axis: Vec<f64> = parsed.rows.iter().map(|row| row[0]).collect();
    let mut signals = BTreeMap::new();
    for column_index in 1..parsed.column_labels.len() {
        let label = &parsed.column_labels[column_index];
        let values = parsed
            .rows
            .iter()
            .map(|row| row[column_index])
            .collect::<Vec<_>>();
        let (name, signal_type, unit) = signal_mapping(label, parsed);
        let axis_obj = SpectralAxis::new(axis.clone(), "nm", AxisKind::Wavelength)?;
        let signal = SpectralArray::new(
            axis_obj,
            values,
            vec!["x".to_string()],
            signal_type,
            unit,
            &name,
            "file",
        )?;
        signals.insert(name, signal);
    }
    Ok(signals)
}

fn signal_mapping(label: &str, parsed: &ParsedOceanText) -> (String, SignalType, Option<String>) {
    let lower = label.to_ascii_lowercase();
    if lower == "d" || lower.contains("dark") {
        return ("dark_reference".to_string(), SignalType::RawCounts, None);
    }
    if lower == "r" || lower.contains("reference") {
        return ("white_reference".to_string(), SignalType::RawCounts, None);
    }
    if lower == "s" || lower.contains("sample") {
        return ("sample".to_string(), SignalType::RawCounts, None);
    }

    let header_text = parsed
        .metadata_pairs
        .iter()
        .map(|(_, value)| value.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    if lower == "p" || lower.contains("processed") || lower == "y" {
        if header_text.contains("absolute irradiance") {
            return ("irradiance".to_string(), SignalType::Irradiance, None);
        }
        if header_text.contains("transmission") || header_text.contains("transmittance") {
            return (
                "transmittance".to_string(),
                SignalType::Transmittance,
                Some("%".to_string()),
            );
        }
        if header_text.contains("reflectance") {
            return (
                "reflectance".to_string(),
                SignalType::Reflectance,
                Some("%".to_string()),
            );
        }
        return ("processed".to_string(), SignalType::Unknown, None);
    }

    (safe_signal_name(label, "signal"), SignalType::Unknown, None)
}

fn dominant_signal_type(signals: &BTreeMap<String, SpectralArray>) -> SignalType {
    for preferred in [
        SignalType::Absorbance,
        SignalType::Reflectance,
        SignalType::Transmittance,
        SignalType::Irradiance,
    ] {
        if signals
            .values()
            .any(|signal| signal.signal_type == preferred)
        {
            return preferred;
        }
    }
    signals
        .values()
        .next()
        .map(|signal| signal.signal_type.clone())
        .unwrap_or(SignalType::Unknown)
}

fn default_column_labels(width: usize) -> Vec<String> {
    if width == 2 {
        vec!["W".to_string(), "P".to_string()]
    } else {
        (0..width)
            .map(|index| {
                if index == 0 {
                    "W".to_string()
                } else {
                    format!("signal_{index}")
                }
            })
            .collect()
    }
}

fn is_numeric_pair_line(line: &str) -> bool {
    parse_numeric_row(line)
        .map(|values| values.len() >= 2)
        .unwrap_or(false)
}

fn parse_numeric_row(line: &str) -> Option<Vec<f64>> {
    let values = split_fields(line)
        .iter()
        .map(|field| parse_number(field))
        .collect::<Option<Vec<_>>>()?;
    (values.len() >= 2).then_some(values)
}

fn split_fields(line: &str) -> Vec<String> {
    if line.contains('\t') {
        line.split('\t')
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect()
    } else if line.contains(',') {
        line.split(',')
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect()
    } else {
        line.split_whitespace().map(ToString::to_string).collect()
    }
}
