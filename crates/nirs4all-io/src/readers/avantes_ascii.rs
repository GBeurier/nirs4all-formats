use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, FormatProbe, Result, SignalType, SpectralArray, SpectralAxis,
};

use crate::readers::util::{
    metadata_from_pairs, parse_number, read_text_lossy, record_from_signals, safe_signal_name,
    signal_type_from_label, single_signal_record, split_delimited, SingleSignalSpec,
};
use crate::Reader;

pub struct AvantesAsciiReader;

impl Reader for AvantesAsciiReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::avantes_ascii"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        if !matches!(ext.as_str(), "ttt" | "trt" | "tit" | "tat" | "irr" | "txt") {
            return None;
        }
        let text = String::from_utf8_lossy(head);
        if text.lines().any(|line| {
            line.trim_start().to_ascii_lowercase().starts_with("wave") && line.contains(';')
        }) {
            Some(FormatProbe::new(
                "avantes-ascii",
                self.name(),
                Confidence::Definite,
                "AvaSoft ASCII wave table detected",
            ))
        } else if ext == "irr"
            && text
                .lines()
                .filter(|line| parse_pair(line).is_some())
                .count()
                >= 5
        {
            Some(FormatProbe::new(
                "avantes-irradiance-ascii",
                self.name(),
                Confidence::Likely,
                "Avantes irradiance two-column ASCII export",
            ))
        } else {
            None
        }
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let (text, source) = read_text_lossy(path)?;
        if let Some(record) = read_wave_table(self.name(), source.clone(), &text)? {
            return Ok(vec![record]);
        }
        read_two_column_irradiance(self.name(), source, &text)
    }
}

fn read_wave_table(
    reader: &str,
    source: nirs4all_io_core::SourceFile,
    text: &str,
) -> Result<Option<nirs4all_io_core::SpectralRecord>> {
    let lines: Vec<&str> = text.lines().collect();
    let Some(header_idx) = lines
        .iter()
        .position(|line| line.trim_start().to_ascii_lowercase().starts_with("wave"))
    else {
        return Ok(None);
    };
    let headers = split_delimited(lines[header_idx], ';');
    let units = lines
        .get(header_idx + 1)
        .map(|line| split_delimited(line, ';'))
        .unwrap_or_default();
    let mut metadata_pairs = Vec::new();
    for line in &lines[..header_idx] {
        let clean = line.trim();
        if let Some((key, value)) = clean.split_once(':') {
            metadata_pairs.push((key.to_string(), value.trim().to_string()));
        } else if let Some((key, value)) = clean.split_once(']') {
            metadata_pairs.push((key.to_string(), value.trim().to_string()));
        } else if !clean.is_empty() {
            metadata_pairs.push(("header".to_string(), clean.to_string()));
        }
    }

    let mut axis_values = Vec::new();
    let mut columns: Vec<Vec<f64>> = vec![Vec::new(); headers.len().saturating_sub(1)];
    for line in lines.iter().skip(header_idx + 2) {
        if line.trim().is_empty() {
            continue;
        }
        let cells = split_delimited(line, ';');
        if cells.len() < headers.len() {
            continue;
        }
        let Some(x) = parse_number(&cells[0]) else {
            continue;
        };
        axis_values.push(x);
        for index in 1..headers.len() {
            columns[index - 1].push(parse_number(&cells[index]).unwrap_or(f64::NAN));
        }
    }
    if axis_values.is_empty() {
        return Ok(None);
    }

    let mut signals = BTreeMap::new();
    let mut dominant = SignalType::Unknown;
    for (index, values) in columns.into_iter().enumerate() {
        let label = headers
            .get(index + 1)
            .cloned()
            .unwrap_or_else(|| format!("signal_{index}"));
        let signal_type = avantes_signal_type(&label);
        if dominant == SignalType::Unknown
            || matches!(
                signal_type,
                SignalType::Reflectance | SignalType::Transmittance
            )
        {
            dominant = signal_type.clone();
        }
        let unit = units
            .get(index + 1)
            .map(|value| value.trim_matches(['[', ']']).to_string())
            .filter(|value| !value.is_empty());
        let axis = SpectralAxis::new(axis_values.clone(), "nm", AxisKind::Wavelength)?;
        let signal = SpectralArray::new(
            axis,
            values,
            vec!["x".to_string()],
            signal_type,
            unit,
            safe_signal_name(&label, "signal"),
            "file",
        )?;
        signals.insert(safe_signal_name(&label, "signal"), signal);
    }
    Ok(Some(record_from_signals(
        "avantes-ascii",
        reader,
        source,
        signals,
        dominant,
        metadata_from_pairs(metadata_pairs),
        Vec::new(),
    )?))
}

fn read_two_column_irradiance(
    reader: &str,
    source: nirs4all_io_core::SourceFile,
    text: &str,
) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
    let mut axis = Vec::new();
    let mut values = Vec::new();
    let mut metadata = Vec::new();
    for line in text.lines() {
        if let Some((x, y)) = parse_pair(line) {
            axis.push(x);
            values.push(y);
        } else if !line.trim().is_empty() {
            metadata.push((
                "header".to_string(),
                line.trim().trim_matches('"').to_string(),
            ));
        }
    }
    let record = single_signal_record(
        "avantes-irradiance-ascii",
        reader,
        source,
        SingleSignalSpec {
            axis_values: axis,
            axis_unit: "nm".to_string(),
            axis_kind: AxisKind::Wavelength,
            values,
            signal_name: "irradiance".to_string(),
            signal_type: SignalType::Irradiance,
            signal_unit: None,
            role: "irradiance".to_string(),
        },
        BTreeMap::new(),
        metadata_from_pairs(metadata),
        Vec::new(),
    )?;
    Ok(vec![record])
}

fn parse_pair(line: &str) -> Option<(f64, f64)> {
    let mut parts = line.split_whitespace();
    let x = parse_number(parts.next()?)?;
    let y = parse_number(parts.next()?)?;
    Some((x, y))
}

fn avantes_signal_type(label: &str) -> SignalType {
    match safe_signal_name(label, "signal").as_str() {
        "dark" | "ref" | "reference" | "sample" => SignalType::RawCounts,
        _ => signal_type_from_label(label),
    }
}
