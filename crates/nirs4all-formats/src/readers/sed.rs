use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_formats_core::{
    AxisKind, Confidence, FormatProbe, Result, SignalType, SpectralArray, SpectralAxis,
};

use crate::readers::util::{
    metadata_from_pairs, normalize_key, parse_number, read_bytes, record_from_signals,
    safe_signal_name, signal_type_from_label, text_lossy_from_bytes,
};
use crate::Reader;

pub struct SedReader;

impl Reader for SedReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::sed"
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

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
        let bytes = read_bytes(path)?;
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(
        &self,
        path: &Path,
        bytes: &[u8],
    ) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
        let (text, source) = text_lossy_from_bytes(path, bytes);
        let lines: Vec<&str> = text.lines().collect();
        let data_idx = lines
            .iter()
            .position(|line| line.trim().eq_ignore_ascii_case("data:"))
            .ok_or_else(|| {
                nirs4all_formats_core::Error::InvalidRecord("SED missing Data:".to_string())
            })?;
        let mut metadata_pairs = Vec::new();
        for line in &lines[..data_idx] {
            if let Some((key, value)) = line.split_once(':') {
                metadata_pairs.push((key.to_string(), value.trim().to_string()));
            }
        }
        let header_line = lines.get(data_idx + 1).ok_or_else(|| {
            nirs4all_formats_core::Error::InvalidRecord("SED missing column header".to_string())
        })?;
        let headers = split_columns(header_line);
        let mut axis = Vec::new();
        let signal_specs = headers
            .iter()
            .skip(1)
            .map(|label| SedSignalSpec::from_label(label))
            .collect::<Vec<_>>();
        let mut columns: Vec<Vec<f64>> = vec![Vec::new(); signal_specs.len()];
        for line in lines.iter().skip(data_idx + 2) {
            let numbers: Vec<f64> = line.split_whitespace().filter_map(parse_number).collect();
            if numbers.len() < signal_specs.len() + 1 {
                continue;
            }
            axis.push(numbers[0]);
            for index in 1..=signal_specs.len() {
                columns[index - 1].push(numbers[index]);
            }
        }
        let mut signals = BTreeMap::new();
        let mut dominant = SignalType::Unknown;
        for (spec, values) in signal_specs.iter().zip(columns) {
            let signal_type = spec.signal_type.clone();
            if signal_type == SignalType::Reflectance {
                dominant = SignalType::Reflectance;
            } else if dominant == SignalType::Unknown {
                dominant = signal_type.clone();
            }
            let axis_obj = SpectralAxis::new(axis.clone(), "nm", AxisKind::Wavelength)?;
            let signal = SpectralArray::new(
                axis_obj,
                values,
                vec!["x".to_string()],
                signal_type,
                spec.unit.clone(),
                &spec.name,
                "file",
            )?;
            signals.insert(spec.name.clone(), signal);
        }
        let metadata = sed_metadata(metadata_pairs, &signal_specs);
        let mut warnings = Vec::new();
        if let Some(expected) = metadata.get("point_count").and_then(|value| value.as_u64()) {
            if expected != axis.len() as u64 {
                warnings.push(format!(
                    "sed_point_count_mismatch:declared={expected}:parsed={}",
                    axis.len()
                ));
            }
        }
        if let Some(expected) = metadata
            .get("declared_column_count")
            .and_then(|value| value.as_u64())
        {
            let parsed = signal_specs.len() as u64 + 1;
            if expected != parsed {
                warnings.push(format!(
                    "sed_column_count_mismatch:declared={expected}:parsed={parsed}"
                ));
            }
        }
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
            metadata,
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

#[derive(Clone, Debug)]
struct SedSignalSpec {
    label: String,
    name: String,
    signal_type: SignalType,
    unit: Option<String>,
}

impl SedSignalSpec {
    fn from_label(label: &str) -> Self {
        let signal_type = sed_signal_type(label);
        Self {
            label: label.to_string(),
            name: safe_signal_name(label, "signal"),
            unit: sed_signal_unit(label),
            signal_type,
        }
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

fn sed_metadata(
    pairs: Vec<(String, String)>,
    signal_specs: &[SedSignalSpec],
) -> BTreeMap<String, serde_json::Value> {
    let normalized = pairs
        .iter()
        .map(|(key, value)| (normalize_key(key), value.trim().to_string()))
        .collect::<Vec<_>>();
    let mut metadata = metadata_from_pairs(pairs);

    promote_string(&mut metadata, &normalized, "instrument", "instrument");
    promote_instrument(&mut metadata, &normalized);
    promote_measurement_mode(&mut metadata, &normalized);
    promote_string(
        &mut metadata,
        &normalized,
        "radiometric_calibration",
        "radiometric_calibration",
    );
    promote_u64(&mut metadata, &normalized, "channels", "point_count");
    promote_declared_column_count(&mut metadata, &normalized);
    promote_wavelength_range(&mut metadata, &normalized);
    promote_signal_metadata(&mut metadata, signal_specs);
    promote_detector_channels(&mut metadata, &normalized);
    promote_detector_triplets_pair(
        &mut metadata,
        &normalized,
        "temperature_c",
        "detector_temperatures_reference_celsius",
        "detector_temperatures_target_celsius",
    );
    promote_detector_triplets_pair(
        &mut metadata,
        &normalized,
        "integration",
        "integration_time_reference_ms",
        "integration_time_target_ms",
    );
    promote_pair_floats(
        &mut metadata,
        &normalized,
        "battery_voltage",
        "battery_voltages_volts",
    );
    promote_pair_u64s(&mut metadata, &normalized, "averages", "scan_averages");
    promote_string_pair(&mut metadata, &normalized, "dark_mode", "dark_mode");
    promote_foreoptic(&mut metadata, &normalized);

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

fn sed_signal_type(label: &str) -> SignalType {
    let lower = label.to_ascii_lowercase();
    if lower.contains("dn") {
        SignalType::RawCounts
    } else {
        signal_type_from_label(label)
    }
}

fn sed_signal_unit(label: &str) -> Option<String> {
    let lower = label.to_ascii_lowercase();
    if lower.contains('%') {
        Some("%".to_string())
    } else if lower.contains("[1.0]") {
        Some("1".to_string())
    } else if lower.contains("dn") {
        Some("DN".to_string())
    } else {
        None
    }
}

fn promote_string(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
    source_key: &str,
    target_key: &str,
) {
    if let Some(value) = header_value(pairs, source_key) {
        metadata.insert(target_key.to_string(), serde_json::json!(value));
    }
}

fn promote_u64(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
    source_key: &str,
    target_key: &str,
) {
    if let Some(value) = header_value(pairs, source_key).and_then(|value| value.parse::<u64>().ok())
    {
        metadata.insert(target_key.to_string(), serde_json::json!(value));
    }
}

fn promote_declared_column_count(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
) {
    let Some((key, _)) = pairs
        .iter()
        .find(|(candidate, _)| candidate.starts_with("columns_"))
    else {
        return;
    };
    let Some(value) = key.strip_prefix("columns_") else {
        return;
    };
    if let Ok(count) = value.parse::<u64>() {
        metadata.insert(
            "declared_column_count".to_string(),
            serde_json::json!(count),
        );
    }
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

fn promote_detector_channels(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
) {
    let Some(value) = header_value(pairs, "detectors") else {
        return;
    };
    let Some(values) = parse_u64_values(value) else {
        return;
    };
    if values.len() == 3 {
        metadata.insert("detector_channels".to_string(), serde_json::json!(values));
    }
}

fn promote_detector_triplets_pair(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
    source_key: &str,
    reference_key: &str,
    target_key: &str,
) {
    let Some(value) = header_value(pairs, source_key) else {
        return;
    };
    let Some(values) = parse_float_values(value) else {
        return;
    };
    if values.len() != 6 {
        return;
    }
    metadata.insert(
        reference_key.to_string(),
        serde_json::json!(values[..3].to_vec()),
    );
    metadata.insert(
        target_key.to_string(),
        serde_json::json!(values[3..].to_vec()),
    );
}

fn promote_pair_floats(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
    source_key: &str,
    target_key: &str,
) {
    let Some(value) = header_value(pairs, source_key) else {
        return;
    };
    let Some(values) = parse_float_values(value) else {
        return;
    };
    if values.len() == 2 {
        metadata.insert(target_key.to_string(), serde_json::json!(values));
    }
}

fn promote_pair_u64s(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
    source_key: &str,
    target_key: &str,
) {
    let Some(value) = header_value(pairs, source_key) else {
        return;
    };
    let Some(values) = parse_u64_values(value) else {
        return;
    };
    if values.len() == 2 {
        metadata.insert(target_key.to_string(), serde_json::json!(values));
    }
}

fn promote_string_pair(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
    source_key: &str,
    target_key: &str,
) {
    let Some(value) = header_value(pairs, source_key) else {
        return;
    };
    let values = split_header_values(value);
    if values.len() == 2 {
        metadata.insert(target_key.to_string(), serde_json::json!(values));
    }
}

fn promote_foreoptic(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
) {
    let Some(value) = header_value(pairs, "foreoptic") else {
        return;
    };
    let parsed = split_header_values(value)
        .into_iter()
        .map(|value| parse_foreoptic_entry(&value))
        .collect::<Vec<_>>();
    if parsed.len() != 2 {
        return;
    }
    let optics = parsed
        .iter()
        .map(|(optic, _unit)| optic.as_str())
        .collect::<Vec<_>>();
    metadata.insert("foreoptic".to_string(), serde_json::json!(optics));

    let units = parsed
        .iter()
        .filter_map(|(_optic, unit)| unit.as_deref())
        .collect::<Vec<_>>();
    if units.len() == parsed.len() {
        metadata.insert(
            "foreoptic_signal_units".to_string(),
            serde_json::json!(units),
        );
    }
}

fn parse_foreoptic_entry(value: &str) -> (String, Option<String>) {
    let trimmed = value.trim();
    let unit = trimmed
        .split_once('{')
        .and_then(|(_, tail)| {
            tail.split_once('}')
                .map(|(unit, _)| unit.trim().to_string())
        })
        .filter(|unit| !unit.is_empty());
    let optic = trimmed
        .split_once('{')
        .map(|(head, _)| head)
        .unwrap_or(trimmed)
        .trim()
        .trim_end_matches(':')
        .trim()
        .to_string();
    (optic, unit)
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

fn promote_measurement_mode(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
) {
    if let Some(value) = header_value(pairs, "measurement") {
        metadata.insert(
            "measurement_mode".to_string(),
            serde_json::json!(normalize_key(value)),
        );
    }
}

fn promote_instrument(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
) {
    let Some(value) = header_value(pairs, "instrument") else {
        return;
    };
    let Some((model, serial)) = parse_sed_instrument(value) else {
        return;
    };
    metadata.insert("instrument_model".to_string(), serde_json::json!(model));
    metadata.insert("instrument_serial".to_string(), serde_json::json!(serial));
}

fn parse_sed_instrument(value: &str) -> Option<(String, String)> {
    let primary = value.split_whitespace().next().unwrap_or(value);
    let (model, serial_part) = primary.split_once("_SN")?;
    let serial = serial_part
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    if model.is_empty() || serial.is_empty() {
        return None;
    }
    Some((model.to_string(), serial))
}

fn promote_wavelength_range(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
) {
    let Some(value) = header_value(pairs, "wavelength_range") else {
        return;
    };
    let values = split_header_values(value)
        .into_iter()
        .filter_map(|value| parse_number(&value))
        .collect::<Vec<_>>();
    if values.len() == 2 {
        metadata.insert("wavelength_range_nm".to_string(), serde_json::json!(values));
    }
}

fn promote_signal_metadata(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    signal_specs: &[SedSignalSpec],
) {
    if signal_specs.is_empty() {
        return;
    }
    let labels = signal_specs
        .iter()
        .map(|spec| spec.label.as_str())
        .collect::<Vec<_>>();
    metadata.insert(
        "source_signal_labels".to_string(),
        serde_json::json!(labels),
    );

    let units = signal_specs
        .iter()
        .filter_map(|spec| spec.unit.as_deref())
        .collect::<Vec<_>>();
    if units.len() == signal_specs.len() {
        metadata.insert("source_signal_units".to_string(), serde_json::json!(units));
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

fn parse_float_values(value: &str) -> Option<Vec<f64>> {
    split_header_values(value)
        .iter()
        .map(|item| parse_number(item))
        .collect()
}

fn parse_u64_values(value: &str) -> Option<Vec<u64>> {
    split_header_values(value)
        .iter()
        .map(|item| item.parse::<u64>().ok())
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
