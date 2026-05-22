use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{AxisKind, Confidence, Error, FormatProbe, Result, SignalType};
use serde_json::{json, Value};

use crate::readers::util::{
    normalize_key, parse_number, read_bytes, safe_signal_name, signal_type_from_label,
    single_signal_record, text_lossy_from_bytes, SingleSignalSpec,
};
use crate::Reader;

pub struct MsaReader;

impl Reader for MsaReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::msa"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let text = String::from_utf8_lossy(head);
        let upper = text.to_ascii_uppercase();
        ((ext == "msa" || upper.contains("EMSA/MAS")) && upper.contains("#FORMAT")).then(|| {
            FormatProbe::new(
                "emsa-mas-msa",
                self.name(),
                Confidence::Definite,
                "EMSA/MAS spectral data header detected",
            )
        })
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let bytes = read_bytes(path)?;
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(
        &self,
        path: &Path,
        bytes: &[u8],
    ) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let (text, source) = text_lossy_from_bytes(path, bytes);
        let parsed = parse_msa_text(&text)?;
        let metadata = parsed.metadata;
        let record = single_signal_record(
            "emsa-mas-msa",
            self.name(),
            source,
            SingleSignalSpec {
                axis_values: parsed.axis,
                axis_unit: parsed.axis_unit,
                axis_kind: parsed.axis_kind,
                values: parsed.values,
                signal_name: parsed.signal_name,
                signal_type: parsed.signal_type,
                signal_unit: parsed.signal_unit,
                role: parsed.role,
            },
            BTreeMap::new(),
            metadata,
            parsed.warnings,
        )?;
        Ok(vec![record])
    }
}

struct ParsedMsa {
    axis: Vec<f64>,
    axis_unit: String,
    axis_kind: AxisKind,
    values: Vec<f64>,
    signal_name: String,
    signal_type: SignalType,
    signal_unit: Option<String>,
    role: String,
    metadata: BTreeMap<String, Value>,
    warnings: Vec<String>,
}

fn parse_msa_text(text: &str) -> Result<ParsedMsa> {
    let mut header = BTreeMap::<String, Vec<String>>::new();
    let mut data_numbers = Vec::<f64>::new();
    let mut in_spectrum = false;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("#ENDOFDATA") {
            break;
        }
        if line.starts_with('#') && !in_spectrum {
            if let Some((key, value)) = line[1..].split_once(':') {
                let key = normalize_msa_key(key);
                header
                    .entry(key)
                    .or_default()
                    .push(value.trim().trim_matches('"').to_string());
            }
            if line
                .get(1..)
                .map(|value| normalize_msa_key(value.split_once(':').map_or(value, |(key, _)| key)))
                .as_deref()
                == Some("spectrum")
            {
                in_spectrum = true;
            }
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if in_spectrum {
            data_numbers.extend(parse_numeric_fields(line));
        }
    }

    if data_numbers.is_empty() {
        return Err(Error::InvalidRecord(
            "EMSA/MAS MSA contains no spectral data".to_string(),
        ));
    }

    let datatype = header_value(&header, "datatype")
        .unwrap_or("Y")
        .trim()
        .to_ascii_uppercase();
    let npoints = header_number(&header, "npoints").map(|value| value.round() as usize);
    let mut warnings = Vec::new();
    let (mut axis, mut values) =
        if datatype == "XY" || npoints.is_some_and(|expected| data_numbers.len() == expected * 2) {
            parse_xy_data(&data_numbers)?
        } else {
            let values = data_numbers;
            let axis = reconstruct_y_axis(&header, values.len());
            (axis, values)
        };

    if let Some(expected) = npoints {
        if values.len() > expected {
            warnings.push(format!(
                "msa_npoints_truncated: declared {expected}, parsed {}",
                values.len()
            ));
            values.truncate(expected);
            axis.truncate(expected);
        } else if values.len() < expected {
            warnings.push(format!(
                "msa_npoints_mismatch: declared {expected}, parsed {}",
                values.len()
            ));
        }
    }

    let xunits = header_value(&header, "xunits").unwrap_or("index");
    let (axis_kind, axis_unit) = msa_axis_kind_unit(xunits);
    let y_label = header_value(&header, "ylabel").unwrap_or("signal");
    let y_units = header_value(&header, "yunits").unwrap_or("");
    let signal_type = msa_signal_type(y_label, y_units);
    let signal_name = safe_signal_name(y_label, "signal");
    let mut metadata = BTreeMap::new();
    metadata.insert("emsa_mas".to_string(), json!(header));

    Ok(ParsedMsa {
        axis,
        axis_unit,
        axis_kind,
        values,
        signal_name: signal_name.clone(),
        signal_type,
        signal_unit: (!y_units.trim().is_empty()).then(|| y_units.to_string()),
        role: signal_name,
        metadata,
        warnings,
    })
}

fn normalize_msa_key(key: &str) -> String {
    normalize_key(key.split_whitespace().next().unwrap_or(key))
}

fn header_value<'a>(header: &'a BTreeMap<String, Vec<String>>, key: &str) -> Option<&'a str> {
    header
        .get(key)
        .and_then(|values| values.last())
        .map(String::as_str)
}

fn header_number(header: &BTreeMap<String, Vec<String>>, key: &str) -> Option<f64> {
    header_value(header, key).and_then(parse_number)
}

fn parse_numeric_fields(line: &str) -> Vec<f64> {
    line.split([',', ';', '\t', ' '])
        .filter_map(|field| {
            let field = field.trim();
            (!field.is_empty()).then(|| parse_number(field)).flatten()
        })
        .collect()
}

fn parse_xy_data(numbers: &[f64]) -> Result<(Vec<f64>, Vec<f64>)> {
    if numbers.len() < 2 {
        return Err(Error::InvalidRecord(
            "EMSA/MAS XY data contains fewer than two numbers".to_string(),
        ));
    }
    let mut axis = Vec::with_capacity(numbers.len() / 2);
    let mut values = Vec::with_capacity(numbers.len() / 2);
    for pair in numbers.chunks(2) {
        if pair.len() == 2 {
            axis.push(pair[0]);
            values.push(pair[1]);
        }
    }
    Ok((axis, values))
}

fn reconstruct_y_axis(header: &BTreeMap<String, Vec<String>>, len: usize) -> Vec<f64> {
    let offset = header_number(header, "offset").unwrap_or(0.0);
    let xperchan = header_number(header, "xperchan").unwrap_or(1.0);
    let choffset = header_number(header, "choffset").unwrap_or(0.0);
    (0..len)
        .map(|index| offset + (index as f64 + choffset) * xperchan)
        .collect()
}

fn msa_axis_kind_unit(raw: &str) -> (AxisKind, String) {
    let upper = raw.trim().to_ascii_uppercase();
    if upper.contains("1/CM") || upper.contains("CM-1") || upper.contains("WAVENUMBER") {
        (AxisKind::Wavenumber, "cm-1".to_string())
    } else if upper.contains("NM") || upper.contains("NANOM") {
        (AxisKind::Wavelength, "nm".to_string())
    } else if upper.contains("EV") {
        (AxisKind::Energy, "eV".to_string())
    } else if upper.is_empty() {
        (AxisKind::Index, "index".to_string())
    } else {
        (AxisKind::Index, raw.trim().to_string())
    }
}

fn msa_signal_type(label: &str, units: &str) -> SignalType {
    let combined = format!("{label} {units}");
    let inferred = signal_type_from_label(&combined);
    if inferred == SignalType::Unknown && combined.to_ascii_lowercase().contains("intensity") {
        SignalType::RawCounts
    } else {
        inferred
    }
}
