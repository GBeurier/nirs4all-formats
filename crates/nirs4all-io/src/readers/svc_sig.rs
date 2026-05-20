use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, FormatProbe, Result, SignalType, SpectralArray, SpectralAxis,
};

use crate::readers::util::{
    metadata_from_pairs, normalize_key, parse_number, read_text_lossy, record_from_signals,
    signal_type_from_label,
};
use crate::Reader;

pub struct SvcSigReader;

impl Reader for SvcSigReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::svc_sig"
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

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let (text, source) = read_text_lossy(path)?;
        let lines: Vec<&str> = text.lines().collect();
        let data_idx = lines
            .iter()
            .position(|line| line.trim().eq_ignore_ascii_case("data="))
            .ok_or_else(|| {
                nirs4all_io_core::Error::InvalidRecord("SIG missing data=".to_string())
            })?;
        let mut metadata_pairs = Vec::new();
        let mut is_moc = false;
        let mut overlap_removed = false;
        let mut is_resampled = false;
        for line in &lines[..data_idx] {
            if let Some((key, value)) = line.split_once('=') {
                let normalized_key = normalize_key(key);
                let lower_value = value.to_ascii_lowercase();
                match normalized_key.as_str() {
                    "comm" => {
                        if lower_value.contains("overlap") {
                            is_moc = true;
                        }
                        if lower_value.contains("resampled") {
                            is_resampled = true;
                        }
                    }
                    "factors" if lower_value.contains("overlap: remove") => {
                        is_moc = true;
                        overlap_removed = true;
                    }
                    _ => {}
                }
                metadata_pairs.push((key.to_string(), value.trim().to_string()));
            }
        }
        if path
            .file_stem()
            .and_then(|value| value.to_str())
            .is_some_and(|stem| stem.to_ascii_lowercase().contains("_resamp"))
        {
            is_resampled = true;
        }
        let metadata = svc_metadata(metadata_pairs);
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
        if is_resampled {
            record.quality_flags.push("resampled_export".to_string());
        }
        if path
            .file_stem()
            .and_then(|value| value.to_str())
            .is_some_and(|stem| stem.to_ascii_lowercase().contains("_bad"))
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
