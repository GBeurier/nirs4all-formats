use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, FormatProbe, Result, SignalType, SpectralArray, SpectralAxis,
};

use crate::readers::util::{
    metadata_from_pairs, normalize_key, parse_number, read_text_lossy, record_from_signals,
    safe_signal_name, signal_type_from_label,
};
use crate::Reader;

pub struct SedReader;

impl Reader for SedReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::sed"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        let text = String::from_utf8_lossy(head);
        (ext == "sed" && text.contains("Version:") && text.contains("Instrument:")).then(|| {
            FormatProbe::new(
                "spectral-evolution-sed",
                self.name(),
                Confidence::Definite,
                "Spectral Evolution SED header detected",
            )
        })
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let (text, source) = read_text_lossy(path)?;
        let lines: Vec<&str> = text.lines().collect();
        let data_idx = lines
            .iter()
            .position(|line| line.trim().eq_ignore_ascii_case("data:"))
            .ok_or_else(|| {
                nirs4all_io_core::Error::InvalidRecord("SED missing Data:".to_string())
            })?;
        let mut metadata_pairs = Vec::new();
        for line in &lines[..data_idx] {
            if let Some((key, value)) = line.split_once(':') {
                metadata_pairs.push((key.to_string(), value.trim().to_string()));
            }
        }
        let header_line = lines.get(data_idx + 1).ok_or_else(|| {
            nirs4all_io_core::Error::InvalidRecord("SED missing column header".to_string())
        })?;
        let headers = split_columns(header_line);
        let mut axis = Vec::new();
        let mut columns: Vec<Vec<f64>> = vec![Vec::new(); headers.len().saturating_sub(1)];
        for line in lines.iter().skip(data_idx + 2) {
            let numbers: Vec<f64> = line.split_whitespace().filter_map(parse_number).collect();
            if numbers.len() < headers.len() {
                continue;
            }
            axis.push(numbers[0]);
            for index in 1..headers.len() {
                columns[index - 1].push(numbers[index]);
            }
        }
        let mut signals = BTreeMap::new();
        let mut dominant = SignalType::Unknown;
        for (index, values) in columns.into_iter().enumerate() {
            let label = headers[index + 1].clone();
            let signal_type = signal_type_from_label(&label);
            if signal_type == SignalType::Reflectance {
                dominant = SignalType::Reflectance;
            } else if dominant == SignalType::Unknown {
                dominant = signal_type.clone();
            }
            let axis_obj = SpectralAxis::new(axis.clone(), "nm", AxisKind::Wavelength)?;
            let unit = label.contains('%').then(|| "%".to_string());
            let name = safe_signal_name(&label, "signal");
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
        let mut warnings = Vec::new();
        if !signals
            .values()
            .any(|signal| signal.signal_type == SignalType::Reflectance)
        {
            warnings.push("sed_missing_reflectance_signal".to_string());
        }
        let mut record = record_from_signals(
            "spectral-evolution-sed",
            self.name(),
            source,
            signals,
            dominant,
            sed_metadata(metadata_pairs),
            warnings,
        )?;
        if !record
            .signals
            .values()
            .any(|signal| signal.signal_type == SignalType::Reflectance)
        {
            record
                .quality_flags
                .push("missing_reflectance_signal".to_string());
        }
        Ok(vec![record])
    }
}

fn split_columns(line: &str) -> Vec<String> {
    if line.contains('\t') {
        line.split('\t')
            .map(|part| part.trim().to_string())
            .collect()
    } else {
        line.split_whitespace().map(ToString::to_string).collect()
    }
}

fn sed_metadata(pairs: Vec<(String, String)>) -> BTreeMap<String, serde_json::Value> {
    let normalized = pairs
        .iter()
        .map(|(key, value)| (normalize_key(key), value.trim().to_string()))
        .collect::<Vec<_>>();
    let mut metadata = metadata_from_pairs(pairs);

    promote_f64(&mut metadata, &normalized, "latitude", "gps_latitude");
    promote_f64(&mut metadata, &normalized, "longitude", "gps_longitude");
    promote_f64(&mut metadata, &normalized, "altitude", "gps_altitude_m");
    promote_time(&mut metadata, &normalized, "gps_time", "gps_time");
    promote_satellites(&mut metadata, &normalized);

    if let Some(value) = header_value(&normalized, "date") {
        let mut values = split_header_values(value)
            .into_iter()
            .filter_map(|value| normalize_sed_date(&value));
        if let Some(start) = values.next() {
            metadata.insert(
                "acquisition_start_date".to_string(),
                serde_json::json!(start),
            );
        }
        if let Some(end) = values.next() {
            metadata.insert("acquisition_end_date".to_string(), serde_json::json!(end));
        }
    }
    if let Some(value) = header_value(&normalized, "time") {
        let mut values = split_header_values(value)
            .into_iter()
            .filter_map(|value| normalize_sed_time(&value));
        if let Some(start) = values.next() {
            metadata.insert(
                "acquisition_start_time".to_string(),
                serde_json::json!(start),
            );
        }
        if let Some(end) = values.next() {
            metadata.insert("acquisition_end_time".to_string(), serde_json::json!(end));
        }
    }

    metadata
}

fn promote_f64(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
    source_key: &str,
    target_key: &str,
) {
    if let Some(value) = header_value(pairs, source_key).and_then(parse_number) {
        metadata.insert(target_key.to_string(), serde_json::json!(value));
    }
}

fn promote_time(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
    source_key: &str,
    target_key: &str,
) {
    if let Some(value) = header_value(pairs, source_key).and_then(normalize_sed_time) {
        metadata.insert(target_key.to_string(), serde_json::json!(value));
    }
}

fn promote_satellites(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
) {
    let Some(value) = header_value(pairs, "satellites") else {
        return;
    };
    let parts = value.split('/').map(str::trim).collect::<Vec<_>>();
    if let Some(used) = parts.first().and_then(|value| value.parse::<u64>().ok()) {
        metadata.insert("gps_satellites_used".to_string(), serde_json::json!(used));
    }
    if let Some(visible) = parts.get(1).and_then(|value| value.parse::<u64>().ok()) {
        metadata.insert(
            "gps_satellites_visible".to_string(),
            serde_json::json!(visible),
        );
    }
}

fn header_value<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(candidate, _)| candidate == key)
        .map(|(_, value)| value.as_str())
        .filter(|value| !value.trim().eq_ignore_ascii_case("n/a"))
}

fn split_header_values(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("n/a"))
        .map(ToString::to_string)
        .collect()
}

fn normalize_sed_date(value: &str) -> Option<String> {
    let parts = value.split('/').collect::<Vec<_>>();
    if parts.len() != 3 {
        return None;
    }
    let month = parts[0].parse::<u32>().ok()?;
    let day = parts[1].parse::<u32>().ok()?;
    let year = parts[2].parse::<u32>().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

fn normalize_sed_time(value: &str) -> Option<String> {
    let parts = value.split(':').collect::<Vec<_>>();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let hour = parts[0].parse::<u32>().ok()?;
    let minute = parts[1].parse::<u32>().ok()?;
    let second = parts
        .get(2)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    if hour >= 24 || minute >= 60 || second >= 60 {
        return None;
    }
    Some(format!("{hour:02}:{minute:02}:{second:02}"))
}
