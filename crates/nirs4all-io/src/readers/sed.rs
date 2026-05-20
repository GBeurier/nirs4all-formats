use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, FormatProbe, Result, SignalType, SpectralArray, SpectralAxis,
};

use crate::readers::util::{
    metadata_from_pairs, parse_number, read_text_lossy, record_from_signals, safe_signal_name,
    signal_type_from_label,
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
        let record = record_from_signals(
            "spectral-evolution-sed",
            self.name(),
            source,
            signals,
            dominant,
            metadata_from_pairs(metadata_pairs),
            Vec::new(),
        )?;
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
