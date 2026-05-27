use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_formats_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile,
};
use serde_json::{json, Value};

use crate::readers::util::{
    detect_delimiter, normalize_key, parse_number, read_bytes, single_signal_record,
    split_delimited, text_lossy_from_bytes, SingleSignalSpec,
};
use crate::Reader;

pub struct SpectralMatrixReader;

impl Reader for SpectralMatrixReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::spectral_matrix"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        if !is_supported_extension(path) {
            return None;
        }
        let text = String::from_utf8_lossy(head);
        find_matrix_layout(&text).ok().map(|_| {
            FormatProbe::new(
                "spectral-matrix",
                self.name(),
                Confidence::Likely,
                "one-spectrum-per-row matrix export detected",
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
        let layout = find_matrix_layout(&text)?;
        read_matrix_records(&text, source, self.name(), layout)
    }
}

struct MatrixLayout {
    axis: Vec<f64>,
    headers: Vec<String>,
    spectral_indices: Vec<usize>,
    data_start: usize,
    delimiter: Option<char>,
    metadata_pairs: BTreeMap<String, String>,
}

fn find_matrix_layout(text: &str) -> Result<MatrixLayout> {
    let lines: Vec<&str> = text.lines().collect();
    let mut metadata_pairs = BTreeMap::<String, String>::new();
    let mut axis_block: Option<Vec<f64>> = None;
    let mut saw_preamble = false;

    for (index, raw) in lines.iter().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.eq_ignore_ascii_case("wavelengths:") {
            axis_block = next_numeric_line(&lines, index + 1);
            saw_preamble = true;
            continue;
        }

        let delimiter = delimiter_for_line(line);
        let headers = split_fields(line, delimiter);
        if axis_block.is_some() && headers.iter().all(|header| parse_number(header).is_some()) {
            continue;
        }
        if let Some(axis) = axis_block.clone() {
            let spectral_indices = headers
                .iter()
                .enumerate()
                .filter_map(|(column, header)| {
                    let normalized = header.to_ascii_lowercase();
                    normalized
                        .strip_prefix('p')
                        .and_then(|suffix| suffix.parse::<usize>().ok())
                        .map(|_| column)
                })
                .collect::<Vec<_>>();
            if spectral_indices.len() == axis.len() && axis.len() >= 2 {
                return Ok(MatrixLayout {
                    axis,
                    headers,
                    spectral_indices,
                    data_start: index + 1,
                    delimiter,
                    metadata_pairs,
                });
            }
        }

        let numeric_indices = headers
            .iter()
            .enumerate()
            .filter_map(|(column, header)| parse_number(header).map(|_| column))
            .collect::<Vec<_>>();
        if saw_preamble && numeric_indices.len() >= 10 {
            let axis = numeric_indices
                .iter()
                .filter_map(|column| parse_number(&headers[*column]))
                .collect::<Vec<_>>();
            if looks_like_spectral_axis(&axis) {
                return Ok(MatrixLayout {
                    axis,
                    headers,
                    spectral_indices: numeric_indices,
                    data_start: index + 1,
                    delimiter,
                    metadata_pairs,
                });
            }
        }

        saw_preamble = true;
        collect_metadata_line(line, &mut metadata_pairs);
    }

    Err(Error::InvalidRecord(
        "no spectral matrix header found".to_string(),
    ))
}

fn read_matrix_records(
    text: &str,
    source: SourceFile,
    reader: &str,
    layout: MatrixLayout,
) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut records = Vec::new();
    for (row_index, raw) in lines.iter().skip(layout.data_start).enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let cells = split_fields(line, layout.delimiter);
        if cells.len() < layout.headers.len() {
            continue;
        }
        let values = layout
            .spectral_indices
            .iter()
            .map(|column| parse_number(&cells[*column]))
            .collect::<Option<Vec<_>>>();
        let Some(values) = values else {
            continue;
        };
        if values.len() != layout.axis.len() {
            continue;
        }

        let mut targets = BTreeMap::<String, Value>::new();
        let mut metadata = metadata_from_layout(&layout.metadata_pairs);
        metadata.insert("row_index".to_string(), json!(row_index));

        for (column, header) in layout.headers.iter().enumerate() {
            if layout.spectral_indices.contains(&column) {
                continue;
            }
            let value = cells.get(column).cloned().unwrap_or_default();
            if value.is_empty() {
                continue;
            }
            let key = normalize_key(header);
            if is_sample_id_header(header) || (header.is_empty() && column == 0) {
                metadata.insert("sample_id".to_string(), json!(value));
            } else if let Some(number) = parse_number(&value) {
                targets.insert(key, json!(number));
            } else {
                metadata.insert(key, json!(value));
            }
        }

        records.push(single_signal_record(
            "spectral-matrix",
            reader,
            source.clone(),
            SingleSignalSpec {
                axis_values: layout.axis.clone(),
                axis_unit: "nm".to_string(),
                axis_kind: AxisKind::Wavelength,
                values,
                signal_name: "absorbance".to_string(),
                signal_type: SignalType::Absorbance,
                signal_unit: None,
                role: "absorbance".to_string(),
            },
            targets,
            metadata,
            Vec::new(),
        )?);
    }

    if records.is_empty() {
        return Err(Error::InvalidRecord(
            "spectral matrix contains no complete sample rows".to_string(),
        ));
    }
    Ok(records)
}

fn next_numeric_line(lines: &[&str], start: usize) -> Option<Vec<f64>> {
    for raw in lines.iter().skip(start) {
        let numbers = raw
            .split_whitespace()
            .filter_map(parse_number)
            .collect::<Vec<_>>();
        if numbers.len() >= 2 {
            return Some(numbers);
        }
        if !raw.trim().is_empty() {
            return None;
        }
    }
    None
}

fn looks_like_spectral_axis(axis: &[f64]) -> bool {
    axis.len() >= 10
        && axis.first().is_some_and(|value| *value >= 100.0)
        && axis.windows(2).all(|pair| pair[0] < pair[1])
}

fn collect_metadata_line(line: &str, metadata_pairs: &mut BTreeMap<String, String>) {
    if let Some((key, value)) = line.split_once(':').or_else(|| line.split_once(',')) {
        let key = normalize_key(key);
        let value = value.trim().trim_matches('"').to_string();
        if !key.is_empty() && !value.is_empty() {
            metadata_pairs.insert(key, value);
            return;
        }
    }
    metadata_pairs
        .entry("title".to_string())
        .or_insert_with(|| line.trim_matches('"').to_string());
}

fn metadata_from_layout(pairs: &BTreeMap<String, String>) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    if !pairs.is_empty() {
        metadata.insert("vendor".to_string(), json!(pairs));
    }
    metadata
}

fn is_sample_id_header(header: &str) -> bool {
    let normalized = normalize_key(header);
    matches!(
        normalized.as_str(),
        "sample" | "sample_id" | "sampleid" | "id"
    )
}

fn split_fields(line: &str, delimiter: Option<char>) -> Vec<String> {
    let cleaned = line.trim().trim_matches('"');
    if let Some(delimiter) = delimiter {
        split_delimited(cleaned, delimiter)
    } else {
        cleaned
            .split_whitespace()
            .map(|field| field.trim().trim_matches('"').to_string())
            .collect()
    }
}

fn delimiter_for_line(line: &str) -> Option<char> {
    let counts = [',', ';', '\t']
        .into_iter()
        .map(|delimiter| (delimiter, line.matches(delimiter).count()))
        .collect::<Vec<_>>();
    if counts.iter().any(|(_, count)| *count > 0) {
        Some(detect_delimiter(line))
    } else {
        None
    }
}

fn is_supported_extension(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(ext.as_str(), "csv" | "txt")
}
