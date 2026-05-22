use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{AxisKind, Confidence, FormatProbe, Result, SignalType};
use serde_json::{json, Value};

use crate::readers::util::{
    detect_delimiter, normalize_key, parse_number, read_bytes, single_signal_record,
    split_delimited, text_lossy_from_bytes, SingleSignalSpec,
};
use crate::Reader;

pub struct CsvLikeReader;

impl Reader for CsvLikeReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::csv_like"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        if !matches!(ext.as_str(), "csv" | "tsv" | "txt") {
            return None;
        }
        let text = String::from_utf8_lossy(head);
        let first = text.lines().find(|line| !line.trim().is_empty())?;
        let detected_delimiter = detect_delimiter(first);
        let delimiters: &[char] = match ext.as_str() {
            "csv" => &[',', ';', '\t'],
            "tsv" => &['\t'],
            _ => std::slice::from_ref(&detected_delimiter),
        };
        let minimum_delimiters = if matches!(ext.as_str(), "csv" | "tsv") {
            1
        } else {
            2
        };
        let direct_header = first.matches(detected_delimiter).count() >= 2;
        let fallback_delimited = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .any(|line| {
                delimiters
                    .iter()
                    .any(|delimiter| line.matches(*delimiter).count() >= minimum_delimiters)
            });
        if direct_header || fallback_delimited {
            let confidence = if direct_header {
                Confidence::Likely
            } else {
                Confidence::Possible
            };
            Some(FormatProbe::new(
                "delimited-text",
                self.name(),
                confidence,
                "text file with delimited header",
            ))
        } else {
            None
        }
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
        let mut lines = text.lines().filter(|line| !line.trim().is_empty());
        let header_line = lines.next().ok_or_else(|| {
            nirs4all_io_core::Error::InvalidRecord("empty delimited file".to_string())
        })?;
        let delimiter = detect_delimiter(header_line);
        let headers = split_delimited(header_line, delimiter);
        let spectral_indices: Vec<usize> = headers
            .iter()
            .enumerate()
            .filter_map(|(index, header)| parse_number(header).map(|_| index))
            .collect();
        if spectral_indices.is_empty() {
            return Err(nirs4all_io_core::Error::InvalidRecord(
                "no numeric spectral headers found".to_string(),
            ));
        }
        let axis: Vec<f64> = spectral_indices
            .iter()
            .filter_map(|index| parse_number(&headers[*index]))
            .collect();

        let mut records = Vec::new();
        for (row_index, line) in lines.enumerate() {
            let cells = split_delimited(line, delimiter);
            if cells.len() < headers.len() {
                continue;
            }
            let values: Vec<f64> = spectral_indices
                .iter()
                .filter_map(|index| parse_number(&cells[*index]))
                .collect();
            if values.len() != axis.len() {
                continue;
            }

            let mut targets = BTreeMap::<String, Value>::new();
            let mut metadata = BTreeMap::<String, Value>::new();
            for (index, header) in headers.iter().enumerate() {
                if spectral_indices.contains(&index) {
                    continue;
                }
                let cell = cells.get(index).cloned().unwrap_or_default();
                if is_sample_id_header(header) {
                    metadata.insert("sample_id".to_string(), json!(cell));
                } else if let Some(number) = parse_number(&cell) {
                    targets.insert(header.to_string(), json!(number));
                } else if !cell.is_empty() {
                    metadata.insert(header.to_string(), json!(cell));
                }
            }
            metadata.insert("row_index".to_string(), json!(row_index));

            records.push(single_signal_record(
                "delimited-text",
                self.name(),
                source.clone(),
                SingleSignalSpec {
                    axis_values: axis.clone(),
                    axis_unit: "nm".to_string(),
                    axis_kind: AxisKind::Wavelength,
                    values,
                    signal_name: "signal".to_string(),
                    signal_type: SignalType::Absorbance,
                    signal_unit: None,
                    role: "signal".to_string(),
                },
                targets,
                metadata,
                Vec::new(),
            )?);
        }
        Ok(records)
    }
}

fn is_sample_id_header(header: &str) -> bool {
    let normalized = normalize_key(header);
    matches!(
        normalized.as_str(),
        "sample" | "sample_id" | "sampleid" | "id" | "id_layer_uuid_txt"
    )
}
