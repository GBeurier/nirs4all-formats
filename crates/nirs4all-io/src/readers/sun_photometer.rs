use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile};
use serde_json::{json, Value};

use crate::readers::util::{
    normalize_key, parse_number, read_text_lossy, single_signal_record, split_delimited,
    SingleSignalSpec,
};
use crate::Reader;

pub struct SunPhotometerReader;

impl Reader for SunPhotometerReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::sun_photometer"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        if !is_supported_extension(path) {
            return None;
        }
        let text = String::from_utf8_lossy(head);
        if text.contains("MFR-7 Sun Photometer") {
            Some(FormatProbe::new(
                "mfr-sun-photometer",
                self.name(),
                Confidence::Definite,
                "MFR sun photometer channel export detected",
            ))
        } else if text
            .lines()
            .any(|line| line.contains("AOT_1020") && line.contains("AOT_870"))
        {
            Some(FormatProbe::new(
                "microtops-sun-photometer",
                self.name(),
                Confidence::Definite,
                "Microtops sun photometer AOT export detected",
            ))
        } else {
            None
        }
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let (text, source) = read_text_lossy(path)?;
        if text.contains("MFR-7 Sun Photometer") {
            read_mfr_records(&text, source, self.name())
        } else {
            read_microtops_records(&text, source, self.name())
        }
    }
}

fn read_mfr_records(
    text: &str,
    source: SourceFile,
    reader: &str,
) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
    let lines: Vec<&str> = text.lines().collect();
    let header_index = lines
        .iter()
        .position(|line| line.trim_start().starts_with("Record"))
        .ok_or_else(|| Error::InvalidRecord("MFR export missing Record header".to_string()))?;
    let headers = split_whitespace(lines[header_index]);
    let channel_indices = headers
        .iter()
        .enumerate()
        .filter_map(|(column, header)| {
            header
                .strip_prefix("Channel_")
                .and_then(parse_number)
                .map(|wavelength| (column, wavelength))
        })
        .collect::<Vec<_>>();
    if channel_indices.is_empty() {
        return Err(Error::InvalidRecord(
            "MFR export contains no Channel_* columns".to_string(),
        ));
    }
    let axis = channel_indices
        .iter()
        .map(|(_, wavelength)| *wavelength)
        .collect::<Vec<_>>();
    let base_metadata = mfr_base_metadata(&lines[..header_index]);

    let mut records = Vec::new();
    for (row_index, raw) in lines.iter().skip(header_index + 1).enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let cells = split_whitespace(line);
        if cells.len() < headers.len() {
            continue;
        }
        let values = channel_indices
            .iter()
            .map(|(column, _)| parse_number(&cells[*column]))
            .collect::<Option<Vec<_>>>();
        let Some(values) = values else {
            continue;
        };
        let mut metadata = base_metadata.clone();
        metadata.insert("row_index".to_string(), json!(row_index));
        insert_cell_metadata(&mut metadata, &headers, &cells, "Record", "record");
        insert_cell_metadata(&mut metadata, &headers, &cells, "HH:MM:SS", "time");
        insert_cell_metadata(&mut metadata, &headers, &cells, "AirMass", "air_mass");

        records.push(single_signal_record(
            "mfr-sun-photometer",
            reader,
            source.clone(),
            SingleSignalSpec {
                axis_values: axis.clone(),
                axis_unit: "nm".to_string(),
                axis_kind: AxisKind::Wavelength,
                values,
                signal_name: "channels".to_string(),
                signal_type: SignalType::RawCounts,
                signal_unit: None,
                role: "channels".to_string(),
            },
            BTreeMap::new(),
            metadata,
            Vec::new(),
        )?);
    }
    if records.is_empty() {
        return Err(Error::InvalidRecord(
            "MFR export contains no complete data rows".to_string(),
        ));
    }
    Ok(records)
}

fn read_microtops_records(
    text: &str,
    source: SourceFile,
    reader: &str,
) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
    let lines: Vec<&str> = text.lines().collect();
    let header_line = lines
        .iter()
        .find(|line| line.contains("AOT_1020") && line.contains("AOT_870"))
        .ok_or_else(|| Error::InvalidRecord("Microtops export missing AOT header".to_string()))?;
    let headers = split_delimited(header_line, ',');
    let header_index = lines
        .iter()
        .position(|line| *line == *header_line)
        .unwrap_or(0);
    let aot_indices = headers
        .iter()
        .enumerate()
        .filter_map(|(column, header)| {
            header
                .strip_prefix("AOT_")
                .and_then(parse_number)
                .map(|wavelength| (column, wavelength))
        })
        .collect::<Vec<_>>();
    if aot_indices.is_empty() {
        return Err(Error::InvalidRecord(
            "Microtops export contains no AOT_* columns".to_string(),
        ));
    }
    let axis = aot_indices
        .iter()
        .map(|(_, wavelength)| *wavelength)
        .collect::<Vec<_>>();

    let mut records = Vec::new();
    for (row_index, raw) in lines.iter().skip(header_index + 1).enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let cells = split_delimited(line, ',');
        if cells.len() < headers.len() {
            continue;
        }
        let values = aot_indices
            .iter()
            .map(|(column, _)| parse_number(&cells[*column]))
            .collect::<Option<Vec<_>>>();
        let Some(values) = values else {
            continue;
        };
        let mut metadata = BTreeMap::new();
        metadata.insert("row_index".to_string(), json!(row_index));
        for (column, header) in headers.iter().enumerate() {
            if aot_indices
                .iter()
                .any(|(aot_column, _)| *aot_column == column)
            {
                continue;
            }
            let value = cells.get(column).cloned().unwrap_or_default();
            if value.is_empty() {
                continue;
            }
            let key = normalize_key(header);
            if let Some(number) = parse_number(&value) {
                metadata.insert(key, json!(number));
            } else {
                metadata.insert(key, json!(value));
            }
        }

        records.push(single_signal_record(
            "microtops-sun-photometer",
            reader,
            source.clone(),
            SingleSignalSpec {
                axis_values: axis.clone(),
                axis_unit: "nm".to_string(),
                axis_kind: AxisKind::Wavelength,
                values,
                signal_name: "aot".to_string(),
                signal_type: SignalType::Unknown,
                signal_unit: None,
                role: "aot".to_string(),
            },
            BTreeMap::new(),
            metadata,
            Vec::new(),
        )?);
    }
    if records.is_empty() {
        return Err(Error::InvalidRecord(
            "Microtops export contains no complete data rows".to_string(),
        ));
    }
    Ok(records)
}

fn mfr_base_metadata(lines: &[&str]) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    if let Some(title) = lines
        .first()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
    {
        metadata.insert("instrument".to_string(), json!(title));
    }
    if let Some(line) = lines.get(1) {
        for key in ["Date", "Site", "Lat", "Lon", "Alt"] {
            if let Some(value) = value_after_label(line, key) {
                metadata.insert(normalize_key(key), json!(value));
            }
        }
    }
    metadata
}

fn value_after_label(line: &str, label: &str) -> Option<String> {
    let start = line.find(label)? + label.len();
    let after_label = line[start..].trim_start_matches(':').trim();
    let next_label = ["Date:", "Site:", "Lat:", "Lon:", "Alt:"]
        .into_iter()
        .filter_map(|marker| after_label.find(marker))
        .filter(|position| *position > 0)
        .min()
        .unwrap_or(after_label.len());
    Some(after_label[..next_label].trim().to_string())
}

fn insert_cell_metadata(
    metadata: &mut BTreeMap<String, Value>,
    headers: &[String],
    cells: &[String],
    header: &str,
    key: &str,
) {
    if let Some(index) = headers.iter().position(|value| value == header) {
        if let Some(value) = cells.get(index) {
            if let Some(number) = parse_number(value) {
                metadata.insert(key.to_string(), json!(number));
            } else {
                metadata.insert(key.to_string(), json!(value));
            }
        }
    }
}

fn split_whitespace(line: &str) -> Vec<String> {
    line.split_whitespace().map(ToString::to_string).collect()
}

fn is_supported_extension(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(ext.as_str(), "out" | "txt" | "csv")
}
