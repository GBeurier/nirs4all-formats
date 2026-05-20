use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{AxisKind, Confidence, Error, FormatProbe, Result, SignalType};
use serde_json::json;

use crate::readers::util::{parse_number, read_text_lossy, single_signal_record, SingleSignalSpec};
use crate::Reader;

pub struct UsgsArefReader;

impl Reader for UsgsArefReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::usgs_aref"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "txt" | "asc") {
            return None;
        }
        let text = String::from_utf8_lossy(head);
        let first = text.lines().next()?.trim();
        (first.contains("Record=") && first.contains(" AREF")).then(|| {
            FormatProbe::new(
                "usgs-aref-single-column",
                self.name(),
                Confidence::Likely,
                "USGS spectral library AREF single-column reflectance dump",
            )
        })
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let (text, source) = read_text_lossy(path)?;
        let parsed = parse_aref_text(&text)?;
        let axis = (0..parsed.values.len())
            .map(|value| value as f64)
            .collect::<Vec<_>>();
        let mut metadata = BTreeMap::new();
        metadata.insert("title".to_string(), json!(parsed.title));
        if let Some(record) = parsed.record_number {
            metadata.insert("record_number".to_string(), json!(record));
        }
        metadata.insert("axis_note".to_string(), json!("no wavelength axis in file"));

        Ok(vec![single_signal_record(
            "usgs-aref-single-column",
            self.name(),
            source,
            SingleSignalSpec {
                axis_values: axis,
                axis_unit: "index".to_string(),
                axis_kind: AxisKind::Index,
                values: parsed.values,
                signal_name: "reflectance".to_string(),
                signal_type: SignalType::Reflectance,
                signal_unit: None,
                role: "reflectance".to_string(),
            },
            BTreeMap::new(),
            metadata,
            vec!["usgs_aref_axis_generated_index".to_string()],
        )?])
    }
}

struct ParsedAref {
    title: String,
    record_number: Option<u64>,
    values: Vec<f64>,
}

fn parse_aref_text(text: &str) -> Result<ParsedAref> {
    let mut lines = text.lines();
    let title = lines
        .next()
        .map(str::trim)
        .filter(|line| line.contains("Record=") && line.contains(" AREF"))
        .ok_or_else(|| Error::InvalidRecord("USGS AREF title line missing".to_string()))?
        .to_string();
    let record_number = title
        .split("Record=")
        .nth(1)
        .and_then(|tail| tail.split(':').next())
        .and_then(|value| value.trim().parse::<u64>().ok());
    let values = lines
        .filter_map(|line| {
            let trimmed = line.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .map(|line| {
            parse_number(line).ok_or_else(|| {
                Error::InvalidRecord(format!("USGS AREF non-numeric reflectance row: {line}"))
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if values.len() < 2 {
        return Err(Error::InvalidRecord(
            "USGS AREF dump contains fewer than two reflectance values".to_string(),
        ));
    }
    Ok(ParsedAref {
        title,
        record_number,
        values,
    })
}
