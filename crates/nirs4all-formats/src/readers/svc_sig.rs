use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_formats_core::{
    AxisKind, Confidence, FormatProbe, Result, SignalType, SpectralArray, SpectralAxis,
};

use crate::readers::util::{
    metadata_from_pairs, normalize_key, parse_number, read_bytes, record_from_signals,
    signal_type_from_label, text_lossy_from_bytes,
};
use crate::Reader;

pub struct SvcSigReader;

impl Reader for SvcSigReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::svc_sig"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        let text = String::from_utf8_lossy(head);
        (ext == "sig" && text.contains("Spectra Vista SIG Data")).then(|| {
            FormatProbe::new(
                "svc-ger-sig",
                self.name(),
                Confidence::Definite,
                "Spectra Vista SIG magic detected",
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
            .position(|line| line.trim().eq_ignore_ascii_case("data="))
            .ok_or_else(|| {
                nirs4all_formats_core::Error::InvalidRecord("SIG missing data=".to_string())
            })?;
        let mut metadata_pairs = Vec::new();
        let mut is_moc = false;
        let mut is_resampled = false;
        for line in &lines[..data_idx] {
            if let Some((key, value)) = line.split_once('=') {
                let normalized_key = normalize_key(key);
                let lower_value = value.to_ascii_lowercase();
                if normalized_key == "comm" {
                    if lower_value.contains("overlap") {
                        is_moc = true;
                    }
                    if lower_value.contains("resampled") {
                        is_resampled = true;
                    }
                }
                metadata_pairs.push((key.to_string(), value.trim().to_string()));
            }
        }
        let stem_lower = path
            .file_stem()
            .and_then(|value| value.to_str())
            .map(|stem| stem.to_ascii_lowercase());
        if stem_lower
            .as_deref()
            .is_some_and(|stem| stem.contains("_resamp"))
        {
            is_resampled = true;
        }
        let is_white_reference = stem_lower
            .as_deref()
            .is_some_and(|stem| stem.contains("_wr_") || stem.ends_with("_wr"));
        let metadata = svc_metadata(metadata_pairs);
        let overlap_policy = metadata
            .get("overlap_policy")
            .and_then(|value| value.as_str());
        let overlap_removed = overlap_policy == Some("remove");
        let overlap_preserved = overlap_policy == Some("preserve");
        if overlap_removed {
            is_moc = true;
        }
        let source_signal_units = source_signal_units(&metadata);
        let mut axis = Vec::new();
        let mut reference = Vec::new();
        let mut target = Vec::new();
        let mut reflectance = Vec::new();
        for line in lines.iter().skip(data_idx + 1) {
            let numbers: Vec<f64> = line.split_whitespace().filter_map(parse_number).collect();
            if numbers.len() >= 4 {
                axis.push(numbers[0]);
                reference.push(numbers[1]);
                target.push(numbers[2]);
                reflectance.push(numbers[3]);
            }
        }
        let mut signals = BTreeMap::new();
        for (name, values, signal_type, unit) in [
            (
                "reference",
                reference,
                source_signal_units
                    .first()
                    .map_or(SignalType::Radiance, |unit| signal_type_from_label(unit)),
                None,
            ),
            (
                "target",
                target,
                source_signal_units
                    .get(1)
                    .map_or(SignalType::Radiance, |unit| signal_type_from_label(unit)),
                None,
            ),
            (
                "reflectance",
                reflectance,
                SignalType::Reflectance,
                Some("%".to_string()),
            ),
        ] {
            let signal = SpectralArray::new(
                SpectralAxis::new(axis.clone(), "nm", AxisKind::Wavelength)?,
                values,
                vec!["x".to_string()],
                signal_type,
                unit,
                name,
                "file",
            )?;
            signals.insert(name.to_string(), signal);
        }
        let mut record = record_from_signals(
            "svc-ger-sig",
            self.name(),
            source,
            signals,
            SignalType::Reflectance,
            metadata,
            Vec::new(),
        )?;
        if is_moc {
            record
                .quality_flags
                .push("matched_overlap_corrected".to_string());
        }
        if overlap_removed {
            record.quality_flags.push("overlap_removed".to_string());
        }
        if overlap_preserved && !overlap_removed {
            record
                .quality_flags
                .push("detector_overlap_preserved".to_string());
        }
        if is_resampled {
            record.quality_flags.push("resampled_export".to_string());
        }
        if is_white_reference {
            record.quality_flags.push("white_reference".to_string());
        }
        if stem_lower
            .as_deref()
            .is_some_and(|stem| stem.contains("_bad"))
        {
            record
                .quality_flags
                .push("declared_bad_fixture".to_string());
            record
                .provenance
                .warnings
                .push("svc_sig_declared_bad_fixture".to_string());
        }
        Ok(vec![record])
    }
}

fn svc_metadata(pairs: Vec<(String, String)>) -> BTreeMap<String, serde_json::Value> {
    let normalized = pairs
        .iter()
        .map(|(key, value)| (normalize_key(key), value.trim().to_string()))
        .collect::<Vec<_>>();
    let mut metadata = metadata_from_pairs(pairs);

    if let Some(value) = header_value(&normalized, "time") {
        let times = split_header_values(value)
            .into_iter()
            .filter_map(|value| normalize_svc_datetime(&value))
            .collect::<Vec<_>>();
        if let Some((date, time)) = times.first() {
            metadata.insert(
                "acquisition_start_date".to_string(),
                serde_json::json!(date),
            );
            metadata.insert(
                "acquisition_start_time".to_string(),
                serde_json::json!(time),
            );
        }
        if let Some((date, time)) = times.get(1) {
            metadata.insert("acquisition_end_date".to_string(), serde_json::json!(date));
            metadata.insert("acquisition_end_time".to_string(), serde_json::json!(time));
        }
    }

    promote_coordinate_pair(&mut metadata, &normalized, "latitude", "latitude");
    promote_coordinate_pair(&mut metadata, &normalized, "longitude", "longitude");

    if let Some(value) = header_value(&normalized, "gpstime") {
        let times = split_header_values(value)
            .into_iter()
            .filter_map(|value| normalize_svc_gps_time(&value))
            .collect::<Vec<_>>();
        if let Some(time) = times.first() {
            metadata.insert("gps_time".to_string(), serde_json::json!(time));
            metadata.insert("gps_start_time".to_string(), serde_json::json!(time));
        }
        if let Some(time) = times.get(1) {
            metadata.insert("gps_end_time".to_string(), serde_json::json!(time));
        }
    }

    if let Some(value) = header_value(&normalized, "units") {
        let mut units = split_header_values(value);
        if !units.is_empty() {
            units.push("%".to_string());
            metadata.insert("source_signal_units".to_string(), serde_json::json!(units));
        }
    }

    promote_instrument(&mut metadata, &normalized);
    promote_string_pair(&mut metadata, &normalized, "optic", "foreoptic");
    promote_detector_triplets_pair(
        &mut metadata,
        &normalized,
        "integration",
        "integration_time_reference_ms",
        "integration_time_target_ms",
    );
    promote_detector_int_triplets_pair(
        &mut metadata,
        &normalized,
        "scan_coadds",
        "coadds_reference",
        "coadds_target",
    );
    promote_detector_triplets_pair(
        &mut metadata,
        &normalized,
        "temp",
        "detector_temperatures_reference_celsius",
        "detector_temperatures_target_celsius",
    );
    promote_pair_floats(
        &mut metadata,
        &normalized,
        "battery",
        "battery_voltages_volts",
    );
    promote_pair_ints(&mut metadata, &normalized, "error", "error_codes");
    promote_pair_ints(&mut metadata, &normalized, "memory_slot", "memory_slots");
    promote_factors(&mut metadata, &normalized);

    metadata
}

fn source_signal_units(metadata: &BTreeMap<String, serde_json::Value>) -> Vec<String> {
    metadata
        .get("source_signal_units")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn promote_coordinate_pair(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
    source_key: &str,
    target_suffix: &str,
) {
    let Some(value) = header_value(pairs, source_key) else {
        return;
    };
    let coordinates = split_header_values(value)
        .into_iter()
        .filter_map(|value| normalize_svc_coordinate(&value))
        .collect::<Vec<_>>();
    if let Some(coordinate) = coordinates.first() {
        metadata.insert(
            format!("gps_{target_suffix}"),
            serde_json::json!(coordinate),
        );
        metadata.insert(
            format!("gps_start_{target_suffix}"),
            serde_json::json!(coordinate),
        );
    }
    if let Some(coordinate) = coordinates.get(1) {
        metadata.insert(
            format!("gps_end_{target_suffix}"),
            serde_json::json!(coordinate),
        );
    }
}

fn header_value<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(candidate, _)| candidate == key)
        .map(|(_, value)| value.as_str())
        .filter(|value| !value.trim().is_empty())
}

fn split_header_values(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("n/a"))
        .map(ToString::to_string)
        .collect()
}

fn normalize_svc_datetime(value: &str) -> Option<(String, String)> {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 3 {
        return None;
    }
    let date = normalize_svc_date(parts[0])?;
    let time = normalize_svc_time(parts[1], parts[2])?;
    Some((date, time))
}

fn normalize_svc_date(value: &str) -> Option<String> {
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

fn normalize_svc_time(value: &str, meridiem: &str) -> Option<String> {
    let parts = value.split(':').collect::<Vec<_>>();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let mut hour = parts[0].parse::<u32>().ok()?;
    let minute = parts[1].parse::<u32>().ok()?;
    let second = parts
        .get(2)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    match meridiem.to_ascii_uppercase().as_str() {
        "AM" if hour == 12 => hour = 0,
        "AM" => {}
        "PM" if hour < 12 => hour += 12,
        "PM" => {}
        _ => return None,
    }
    if hour >= 24 || minute >= 60 || second >= 60 {
        return None;
    }
    Some(format!("{hour:02}:{minute:02}:{second:02}"))
}

fn normalize_svc_gps_time(value: &str) -> Option<String> {
    let digits = value
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    if digits.len() < 6 {
        return None;
    }
    let hour = digits[0..2].parse::<u32>().ok()?;
    let minute = digits[2..4].parse::<u32>().ok()?;
    let second = digits[4..6].parse::<u32>().ok()?;
    if hour >= 24 || minute >= 60 || second >= 60 {
        return None;
    }
    Some(format!("{hour:02}:{minute:02}:{second:02}"))
}

fn promote_instrument(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
) {
    let Some(value) = header_value(pairs, "instrument") else {
        return;
    };
    let Some((serial, model)) = parse_svc_instrument(value) else {
        return;
    };
    metadata.insert("instrument_serial".to_string(), serde_json::json!(serial));
    metadata.insert("instrument_model".to_string(), serde_json::json!(model));
}

/// SVC firmware writes `HI: <serial> (<model>)`.
fn parse_svc_instrument(value: &str) -> Option<(String, String)> {
    let trimmed = value.trim();
    let body = trimmed.strip_prefix("HI:").unwrap_or(trimmed).trim();
    let (serial_part, rest) = body.split_once('(')?;
    let serial = serial_part.trim().to_string();
    let model = rest.trim_end_matches(')').trim().to_string();
    if serial.is_empty() || model.is_empty() {
        return None;
    }
    Some((serial, model))
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
    let entries = split_header_values(value);
    if entries.len() == 2 {
        metadata.insert(target_key.to_string(), serde_json::json!(entries));
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

fn promote_detector_int_triplets_pair(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
    source_key: &str,
    reference_key: &str,
    target_key: &str,
) {
    let Some(value) = header_value(pairs, source_key) else {
        return;
    };
    let Some(values) = parse_int_values(value) else {
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

fn promote_pair_ints(
    metadata: &mut BTreeMap<String, serde_json::Value>,
    pairs: &[(String, String)],
    source_key: &str,
    target_key: &str,
) {
    let Some(value) = header_value(pairs, source_key) else {
        return;
    };
    let Some(values) = parse_int_values(value) else {
        return;
    };
    if values.len() == 2 {
        metadata.insert(target_key.to_string(), serde_json::json!(values));
    }
}

fn promote_factors(metadata: &mut BTreeMap<String, serde_json::Value>, pairs: &[(String, String)]) {
    let Some(value) = header_value(pairs, "factors") else {
        return;
    };
    let (numeric_part, bracket) = split_first_factor_block(value);
    let factors: Vec<f64> = numeric_part
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(parse_number)
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default();
    if factors.len() == 3 {
        metadata.insert(
            "radiometric_factors".to_string(),
            serde_json::json!(factors),
        );
    }
    let Some(bracket) = bracket else {
        return;
    };
    if let Some(policy) = parse_factor_field(bracket, "Overlap:") {
        let policy_value = policy.trim();
        let lower = policy_value.to_ascii_lowercase();
        let canonical = if lower.starts_with("remove") {
            "remove"
        } else if lower.starts_with("preserve") {
            "preserve"
        } else {
            policy_value
        };
        metadata.insert("overlap_policy".to_string(), serde_json::json!(canonical));
        if canonical == "remove" {
            // Parse "Remove @ 997,1901" into overlap breakpoints in nm.
            if let Some(at_part) = policy_value.split_once('@').map(|(_, tail)| tail) {
                let breakpoints: Vec<f64> = at_part
                    .split(',')
                    .map(str::trim)
                    .map(parse_number)
                    .collect::<Option<Vec<_>>>()
                    .unwrap_or_default();
                if !breakpoints.is_empty() {
                    metadata.insert(
                        "overlap_break_wavelengths_nm".to_string(),
                        serde_json::json!(breakpoints),
                    );
                }
            }
        }
    }
    if let Some(matching) = parse_factor_field(bracket, "Matching Type:") {
        metadata.insert(
            "matching_type".to_string(),
            serde_json::json!(matching.trim()),
        );
    }
}

fn parse_float_values(value: &str) -> Option<Vec<f64>> {
    split_header_values(value)
        .iter()
        .map(|item| parse_number(item))
        .collect()
}

fn parse_int_values(value: &str) -> Option<Vec<i64>> {
    split_header_values(value)
        .iter()
        .map(|item| item.parse::<i64>().ok())
        .collect()
}

fn split_first_factor_block(value: &str) -> (&str, Option<&str>) {
    let Some((head, rest)) = value.split_once('[') else {
        return (value, None);
    };
    let bracket = rest
        .split_once(']')
        .map(|(inside, _tail)| inside)
        .unwrap_or(rest);
    (head, Some(bracket))
}

/// Read a value associated with `key` (e.g. `Overlap:`) inside a `factors=` bracket.
/// Values can themselves contain commas (e.g. `Remove @ 997,1901`), so we slice up
/// to the next known field marker instead of splitting on commas.
fn parse_factor_field(bracket: &str, key: &str) -> Option<String> {
    let start = bracket.find(key)? + key.len();
    let tail = &bracket[start..];
    const NEXT_FIELDS: &[&str] = &[", Matching Type:", ", Overlap:"];
    let end = NEXT_FIELDS
        .iter()
        .filter_map(|marker| tail.find(marker))
        .min()
        .unwrap_or(tail.len());
    Some(tail[..end].to_string())
}

fn normalize_svc_coordinate(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    let hemisphere = trimmed.chars().last()?.to_ascii_uppercase();
    let degree_digits = match hemisphere {
        'N' | 'S' => 2,
        'E' | 'W' => 3,
        _ => return None,
    };
    let number = &trimmed[..trimmed.len().saturating_sub(1)];
    if number.len() <= degree_digits {
        return None;
    }
    let degrees = number[..degree_digits].parse::<f64>().ok()?;
    let minutes = number[degree_digits..].parse::<f64>().ok()?;
    if minutes >= 60.0 {
        return None;
    }
    let mut decimal = degrees + minutes / 60.0;
    if matches!(hemisphere, 'S' | 'W') {
        decimal = -decimal;
    }
    Some(decimal)
}
