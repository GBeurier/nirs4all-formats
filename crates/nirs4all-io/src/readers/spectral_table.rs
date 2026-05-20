use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SpectralArray, SpectralAxis,
};
use serde_json::{json, Value};

use crate::readers::util::{
    detect_delimiter, normalize_key, parse_number, read_text_lossy, record_from_signals,
    safe_signal_name, signal_type_from_label, split_delimited,
};
use crate::Reader;

pub struct SpectralTableReader;

impl Reader for SpectralTableReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::spectral_table"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        if !is_supported_extension(path) {
            return None;
        }
        let text = String::from_utf8_lossy(head);
        parse_spectral_table_text(&text, path).ok().map(|_| {
            FormatProbe::new(
                "row-spectral-table",
                self.name(),
                Confidence::Likely,
                "row-oriented spectral table with first-column axis detected",
            )
        })
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let (text, source) = read_text_lossy(path)?;
        let parsed = parse_spectral_table_text(&text, path)?;
        let mut signals = BTreeMap::new();
        let mut dominant = SignalType::Unknown;

        for column in parsed.columns {
            let signal_type = column.signal_type;
            dominant = choose_dominant(&dominant, &signal_type);
            let axis = SpectralAxis::new(
                parsed.axis.clone(),
                parsed.axis_unit.clone(),
                parsed.axis_kind.clone(),
            )?;
            let signal = SpectralArray::new(
                axis,
                column.values,
                vec!["x".to_string()],
                signal_type,
                column.unit,
                column.role.clone(),
                "file",
            )?;
            let name = unique_signal_name(&signals, &column.name);
            signals.insert(name, signal);
        }

        let record = record_from_signals(
            "row-spectral-table",
            self.name(),
            source,
            signals,
            dominant,
            parsed.metadata,
            parsed.warnings,
        )?;
        Ok(vec![record])
    }
}

struct ParsedTable {
    axis: Vec<f64>,
    axis_unit: String,
    axis_kind: AxisKind,
    columns: Vec<ParsedColumn>,
    metadata: BTreeMap<String, Value>,
    warnings: Vec<String>,
}

struct ParsedColumn {
    name: String,
    role: String,
    values: Vec<f64>,
    signal_type: SignalType,
    unit: Option<String>,
}

struct TableLayout {
    headers: Vec<String>,
    units: Option<Vec<String>>,
    data_start: usize,
    delimiter: Option<char>,
    metadata_pairs: BTreeMap<String, String>,
    notes: Vec<String>,
}

fn parse_spectral_table_text(text: &str, path: &Path) -> Result<ParsedTable> {
    let lines: Vec<&str> = text.lines().collect();
    let layout = find_layout(&lines)?;
    if layout.headers.len() < 2 {
        return Err(Error::InvalidRecord(
            "spectral table needs at least axis and one signal column".to_string(),
        ));
    }

    let expected_columns = layout.headers.len();
    let mut axis = Vec::new();
    let mut columns = vec![Vec::<f64>::new(); expected_columns - 1];
    let mut skipped_rows = 0usize;

    for raw in lines.iter().skip(layout.data_start) {
        let line = trim_bom(raw).trim();
        if line.is_empty() || is_section_marker(line) {
            continue;
        }
        if is_comment(line) {
            continue;
        }
        let fields = split_fields(line, layout.delimiter);
        let numbers: Vec<f64> = fields
            .iter()
            .filter_map(|field| parse_number(field))
            .collect();
        if numbers.len() < expected_columns {
            if !axis.is_empty() {
                break;
            }
            skipped_rows += 1;
            continue;
        }
        axis.push(numbers[0]);
        for (index, values) in columns.iter_mut().enumerate() {
            values.push(numbers[index + 1]);
        }
    }

    if axis.len() < 2 || columns.iter().any(|values| values.len() != axis.len()) {
        return Err(Error::InvalidRecord(
            "spectral table contains fewer than two complete numeric rows".to_string(),
        ));
    }

    let axis_header = layout.headers[0].as_str();
    let axis_unit_hint = layout
        .units
        .as_ref()
        .and_then(|units| units.first())
        .map(String::as_str)
        .or_else(|| metadata_value(&layout.metadata_pairs, "x_units"))
        .or_else(|| metadata_value(&layout.metadata_pairs, "xaxisunit"));
    let axis_unit = infer_axis_unit(axis_header, axis_unit_hint, path, &axis);
    let axis_kind = infer_axis_kind(axis_header, &axis_unit);

    let parsed_columns = columns
        .into_iter()
        .enumerate()
        .map(|(index, values)| {
            let header = layout.headers[index + 1].as_str();
            let unit_hint = layout
                .units
                .as_ref()
                .and_then(|units| units.get(index + 1))
                .map(String::as_str)
                .or_else(|| metadata_value(&layout.metadata_pairs, "y_units"))
                .or_else(|| metadata_value(&layout.metadata_pairs, "dataunit"));
            let signal_type = infer_signal_type(header, unit_hint);
            let name = safe_signal_name(header, &format!("signal_{}", index + 1));
            ParsedColumn {
                role: name.clone(),
                name,
                values,
                signal_type,
                unit: infer_signal_unit(header, unit_hint),
            }
        })
        .collect::<Vec<_>>();

    let mut warnings = Vec::new();
    if skipped_rows > 0 {
        warnings.push(format!("row_spectral_table_skipped_rows:{skipped_rows}"));
    }

    Ok(ParsedTable {
        axis,
        axis_unit,
        axis_kind,
        columns: parsed_columns,
        metadata: build_metadata(layout.metadata_pairs, layout.notes),
        warnings,
    })
}

fn find_layout(lines: &[&str]) -> Result<TableLayout> {
    let mut metadata_pairs = BTreeMap::<String, String>::new();
    let mut notes = Vec::<String>::new();
    let mut descriptive_header: Option<Vec<String>> = None;

    for (index, raw) in lines.iter().enumerate() {
        let line = trim_bom(raw).trim();
        if line.is_empty() {
            continue;
        }
        if is_section_marker(line) {
            continue;
        }
        if is_metadata_assignment(line) {
            collect_metadata_line(line, &mut metadata_pairs, &mut notes);
            continue;
        }
        if let Some(headers) = descriptive_header_from_line(strip_comment_marker(line)) {
            descriptive_header = Some(headers);
            continue;
        }

        let delimiter = delimiter_for_line(line);
        let fields = split_fields(line, delimiter);
        if is_header_fields(&fields) {
            let units = find_units_row(lines, index + 1, fields.len(), delimiter);
            let data_start = units
                .as_ref()
                .map(|(_, row_index)| row_index + 1)
                .unwrap_or(index + 1);
            return Ok(TableLayout {
                headers: fields,
                units: units.map(|(fields, _)| fields),
                data_start,
                delimiter,
                metadata_pairs,
                notes,
            });
        }

        if looks_like_numeric_data_line(line, delimiter) {
            let headers = descriptive_header
                .clone()
                .or_else(|| synthetic_headers_from_metadata(&metadata_pairs));
            if let Some(headers) = headers {
                if consecutive_numeric_rows(lines, index, headers.len(), delimiter) >= 2 {
                    return Ok(TableLayout {
                        headers,
                        units: None,
                        data_start: index,
                        delimiter,
                        metadata_pairs,
                        notes,
                    });
                }
            }
        }

        collect_metadata_line(line, &mut metadata_pairs, &mut notes);
    }

    Err(Error::InvalidRecord(
        "no spectral table header or metadata-described numeric block found".to_string(),
    ))
}

fn find_units_row(
    lines: &[&str],
    start: usize,
    width: usize,
    delimiter: Option<char>,
) -> Option<(Vec<String>, usize)> {
    for (offset, raw) in lines.iter().enumerate().skip(start) {
        let line = trim_bom(raw).trim();
        if line.is_empty() || is_comment(line) || is_section_marker(line) {
            continue;
        }
        let fields = split_fields(line, delimiter);
        if fields.len() == width && is_unit_row(&fields) {
            return Some((fields, offset));
        }
        return None;
    }
    None
}

fn collect_metadata_line(
    line: &str,
    metadata_pairs: &mut BTreeMap<String, String>,
    notes: &mut Vec<String>,
) {
    let cleaned = line
        .trim_start_matches('#')
        .trim_start_matches('/')
        .trim_start_matches(';')
        .trim()
        .trim_matches('"')
        .trim();
    if cleaned.is_empty() {
        return;
    }

    if let Some((key, value)) = cleaned.split_once(':').or_else(|| cleaned.split_once('=')) {
        let key = normalize_key(key);
        let value = value.trim().trim_matches('"').to_string();
        if !key.is_empty() && !value.is_empty() {
            metadata_pairs.insert(key, value);
            return;
        }
    }

    let fields = split_fields(cleaned, delimiter_for_line(cleaned));
    if cleaned.contains('\t')
        && fields.len() >= 2
        && fields[0]
            .chars()
            .any(|character| character.is_ascii_alphabetic())
        && fields[1]
            .chars()
            .any(|character| !character.is_whitespace())
        && parse_number(&fields[0]).is_none()
    {
        metadata_pairs.insert(normalize_key(&fields[0]), fields[1].to_string());
        return;
    }

    if notes.len() < 12 {
        notes.push(cleaned.to_string());
    }
}

fn is_metadata_assignment(line: &str) -> bool {
    let cleaned = line
        .trim_start_matches('#')
        .trim_start_matches('/')
        .trim_start_matches(';')
        .trim()
        .trim_matches('"')
        .trim();
    let Some(separator_index) = cleaned.find([':', '=']) else {
        return false;
    };
    let delimiter_index = [',', ';', '\t']
        .into_iter()
        .filter_map(|delimiter| cleaned.find(delimiter))
        .min();
    if delimiter_index.is_some_and(|index| index < separator_index) {
        return false;
    }
    let key = cleaned[..separator_index].trim();
    !key.is_empty()
        && key.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, ' ' | '_' | '-' | '.' | '(' | ')')
        })
}

fn synthetic_headers_from_metadata(
    metadata_pairs: &BTreeMap<String, String>,
) -> Option<Vec<String>> {
    if let Some(first) = metadata_value(metadata_pairs, "first_column") {
        if !first.eq_ignore_ascii_case("x") && !first.to_ascii_lowercase().contains("wavelength") {
            return None;
        }
        let y_label = metadata_value(metadata_pairs, "y_units")
            .map(signal_label_from_unit)
            .or_else(|| metadata_value(metadata_pairs, "second_column").map(ToString::to_string))
            .unwrap_or_else(|| "signal".to_string());
        return Some(vec!["x".to_string(), y_label]);
    }

    let x_units = metadata_value(metadata_pairs, "xunits")?;
    let axis = axis_label_from_units(x_units)?;
    let y_label = metadata_value(metadata_pairs, "yunits")
        .map(signal_label_from_unit)
        .or_else(|| metadata_value(metadata_pairs, "data_type").map(ToString::to_string))
        .unwrap_or_else(|| "signal".to_string());
    Some(vec![axis, y_label])
}

fn descriptive_header_from_line(line: &str) -> Option<Vec<String>> {
    let lower = line.to_ascii_lowercase();
    if lower.contains("wavelength")
        && lower.contains("reflectance")
        && lower.contains("standard deviation")
    {
        return Some(vec![
            "wavelength".to_string(),
            "reflectance".to_string(),
            "standard deviation".to_string(),
        ]);
    }

    let delimiter = delimiter_for_line(line);
    let fields = split_fields(line, delimiter);
    if is_header_fields(&fields) {
        Some(fields)
    } else {
        None
    }
}

fn is_header_fields(fields: &[String]) -> bool {
    if fields.len() < 2 {
        return false;
    }
    if fields.iter().all(|field| parse_number(field).is_some()) {
        return false;
    }
    axis_label_score(&fields[0]) > 0
        && fields
            .iter()
            .skip(1)
            .any(|field| parse_number(field).is_none())
}

fn is_unit_row(fields: &[String]) -> bool {
    fields.len() >= 2
        && unit_score(&fields[0]) > 0
        && fields
            .iter()
            .all(|field| parse_number(field).is_none() && field.len() <= 32)
}

fn looks_like_numeric_data_line(line: &str, delimiter: Option<char>) -> bool {
    split_fields(line, delimiter)
        .iter()
        .filter(|field| parse_number(field).is_some())
        .count()
        >= 2
}

fn consecutive_numeric_rows(
    lines: &[&str],
    start: usize,
    width: usize,
    delimiter: Option<char>,
) -> usize {
    let mut count = 0usize;
    for raw in lines.iter().skip(start) {
        let line = trim_bom(raw).trim();
        if line.is_empty() || is_comment(line) {
            continue;
        }
        let numeric_count = split_fields(line, delimiter)
            .iter()
            .filter(|field| parse_number(field).is_some())
            .count();
        if numeric_count >= width {
            count += 1;
        } else if count > 0 {
            break;
        }
        if count >= 3 {
            break;
        }
    }
    count
}

fn split_fields(line: &str, delimiter: Option<char>) -> Vec<String> {
    let cleaned = line.trim().trim_matches('"');
    if let Some(delimiter) = delimiter {
        split_delimited(cleaned, delimiter)
    } else if let Some(delimiter) = delimiter_for_line(cleaned) {
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

fn trim_bom(line: &str) -> &str {
    line.trim_start_matches('\u{feff}')
}

fn is_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('#') || trimmed.starts_with("//") || trimmed.starts_with(';')
}

fn is_section_marker(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('[') && trimmed.ends_with(']')
}

fn is_supported_extension(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "csv" | "tsv" | "txt" | "dat" | "asc" | "spt" | "spu"
    )
}

fn metadata_value<'a>(metadata: &'a BTreeMap<String, String>, key: &str) -> Option<&'a str> {
    metadata.get(&normalize_key(key)).map(String::as_str)
}

fn build_metadata(
    metadata_pairs: BTreeMap<String, String>,
    notes: Vec<String>,
) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    if !metadata_pairs.is_empty() {
        metadata.insert("vendor".to_string(), json!(metadata_pairs));
    }
    if !notes.is_empty() {
        metadata.insert("notes".to_string(), json!(notes));
    }
    metadata
}

fn infer_axis_unit(header: &str, unit_hint: Option<&str>, path: &Path, axis: &[f64]) -> String {
    let combined = format!("{} {}", header, unit_hint.unwrap_or_default()).to_ascii_lowercase();
    if combined.contains("cm-1")
        || combined.contains("cm^-1")
        || combined.contains("1/cm")
        || combined.contains("wavenumber")
    {
        "cm-1".to_string()
    } else if combined.contains("nanometer")
        || combined.contains("_nm")
        || word_contains(&combined, "nm")
    {
        "nm".to_string()
    } else if combined.contains("micrometer")
        || combined.contains("micrometre")
        || combined.contains("_um")
        || combined.contains(" um")
        || combined.contains('\u{00b5}')
    {
        "um".to_string()
    } else if combined.contains("hz") {
        "hz".to_string()
    } else if combined.contains(" ev") || combined.ends_with("ev") {
        "eV".to_string()
    } else if header.to_ascii_lowercase().contains("wavelength") {
        infer_wavelength_unit_from_values(path, axis)
    } else {
        "index".to_string()
    }
}

fn infer_wavelength_unit_from_values(path: &Path, axis: &[f64]) -> String {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let max_axis = axis.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if ext == "asc" || max_axis <= 50.0 {
        "um".to_string()
    } else {
        "nm".to_string()
    }
}

fn infer_axis_kind(header: &str, unit: &str) -> AxisKind {
    let lower = format!("{} {}", header, unit).to_ascii_lowercase();
    if lower.contains("cm-1") || lower.contains("wavenumber") {
        AxisKind::Wavenumber
    } else if lower.contains("hz") {
        AxisKind::Frequency
    } else if lower.contains("wavelength") || matches!(unit, "nm" | "um") {
        AxisKind::Wavelength
    } else {
        AxisKind::Index
    }
}

fn infer_signal_type(header: &str, unit_hint: Option<&str>) -> SignalType {
    let combined = format!("{} {}", header, unit_hint.unwrap_or_default());
    let lower = combined.to_ascii_lowercase();
    if lower.contains("albedo") {
        SignalType::Reflectance
    } else if lower.contains("standard deviation") || lower == "stddev" || lower.contains("std") {
        SignalType::Unknown
    } else if lower.contains("ccd") || lower.contains("cts") {
        SignalType::RawCounts
    } else if lower.starts_with("sample") {
        SignalType::Unknown
    } else {
        signal_type_from_label(&combined)
    }
}

fn infer_signal_unit(header: &str, unit_hint: Option<&str>) -> Option<String> {
    if let Some(unit) = unit_hint.map(str::trim).filter(|unit| !unit.is_empty()) {
        return Some(unit.to_string());
    }
    let lower = header.to_ascii_lowercase();
    if lower.contains('%') || lower.contains("percentage") {
        Some("%".to_string())
    } else {
        None
    }
}

fn signal_label_from_unit(unit: &str) -> String {
    let lower = unit.to_ascii_lowercase();
    if lower.contains("reflect") {
        "reflectance".to_string()
    } else if lower.contains("abs") {
        "absorbance".to_string()
    } else if lower.contains("trans") {
        "transmittance".to_string()
    } else {
        "signal".to_string()
    }
}

fn axis_label_from_units(unit: &str) -> Option<String> {
    let lower = unit.to_ascii_lowercase();
    if lower.contains("wavenumber") || lower.contains("cm-1") || lower.contains("1/cm") {
        Some("wavenumber".to_string())
    } else if lower.contains("nanometer")
        || lower.contains("micrometer")
        || lower.contains("micrometre")
        || lower.contains("wavelength")
        || word_contains(&lower, "nm")
    {
        Some("wavelength".to_string())
    } else {
        None
    }
}

fn strip_comment_marker(line: &str) -> &str {
    let trimmed = line.trim_start();
    trimmed
        .strip_prefix("//")
        .or_else(|| trimmed.strip_prefix('#'))
        .or_else(|| trimmed.strip_prefix(';'))
        .unwrap_or(line)
        .trim_start()
}

fn word_contains(text: &str, needle: &str) -> bool {
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .any(|word| word == needle)
}

fn axis_label_score(label: &str) -> u8 {
    let lower = label.to_ascii_lowercase();
    if lower.contains("wavelength")
        || lower.contains("wavenumber")
        || lower.contains("x-axis")
        || lower == "x"
    {
        2
    } else if lower.starts_with("x_") || lower.starts_with("x ") {
        1
    } else {
        0
    }
}

fn unit_score(value: &str) -> u8 {
    let lower = value.to_ascii_lowercase();
    if lower.contains("cm-1")
        || lower.contains("nm")
        || lower.contains(" um")
        || lower == "um"
        || lower.contains('\u{00b5}')
        || lower.contains("ev")
        || lower.contains("hz")
    {
        1
    } else {
        0
    }
}

fn choose_dominant(current: &SignalType, candidate: &SignalType) -> SignalType {
    if signal_priority(candidate) > signal_priority(current) {
        candidate.clone()
    } else {
        current.clone()
    }
}

fn signal_priority(signal_type: &SignalType) -> u8 {
    match signal_type {
        SignalType::Absorbance
        | SignalType::Reflectance
        | SignalType::Transmittance
        | SignalType::Irradiance
        | SignalType::Radiance
        | SignalType::AerosolOpticalThickness => 4,
        SignalType::KubelkaMunk | SignalType::Derivative | SignalType::Preprocessed => 3,
        SignalType::RawCounts | SignalType::SingleBeam | SignalType::Interferogram => 2,
        SignalType::Unknown => 0,
    }
}

fn unique_signal_name(signals: &BTreeMap<String, SpectralArray>, base: &str) -> String {
    if !signals.contains_key(base) {
        return base.to_string();
    }
    let mut index = 2usize;
    loop {
        let candidate = format!("{base}_{index}");
        if !signals.contains_key(&candidate) {
            return candidate;
        }
        index += 1;
    }
}
