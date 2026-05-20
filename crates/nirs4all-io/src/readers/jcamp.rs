use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, FormatProbe, Result, SignalType, SpectralArray, SpectralAxis,
};
use serde_json::{json, Value};

use crate::readers::util::{
    normalize_key, parse_number, read_text_lossy, record_from_signals, safe_signal_name,
};
use crate::Reader;

pub struct JcampReader;

impl Reader for JcampReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::jcamp"
    }

    fn sniff(&self, head: &[u8], _path: &Path) -> Option<FormatProbe> {
        let text = String::from_utf8_lossy(head);
        (text.contains("##JCAMP-DX=") || text.contains("##JCAMPDX=")).then(|| {
            FormatProbe::new(
                "jcamp-dx",
                self.name(),
                Confidence::Definite,
                "JCAMP-DX labeled-data records detected",
            )
        })
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let (text, source) = read_text_lossy(path)?;
        let parsed = parse_jcamp_text(&text)?;
        let mut signals = BTreeMap::new();
        for signal in parsed.signals {
            let axis = SpectralAxis::new(
                parsed.axis.clone(),
                parsed.axis_unit.clone(),
                parsed.axis_kind.clone(),
            )?;
            let array = SpectralArray::new(
                axis,
                signal.values,
                vec!["x".to_string()],
                signal.signal_type,
                signal.signal_unit,
                signal.role,
                "file",
            )?;
            signals.insert(signal.name, array);
        }
        let dominant = dominant_signal_type(&signals);
        let metadata = parsed.metadata;
        let record = record_from_signals(
            "jcamp-dx",
            self.name(),
            source,
            signals,
            dominant,
            metadata,
            parsed.warnings,
        )?;
        Ok(vec![record])
    }
}

struct ParsedJcamp {
    axis: Vec<f64>,
    axis_unit: String,
    axis_kind: AxisKind,
    signals: Vec<ParsedJcampSignal>,
    metadata: BTreeMap<String, Value>,
    warnings: Vec<String>,
}

struct ParsedJcampSignal {
    name: String,
    values: Vec<f64>,
    signal_type: SignalType,
    signal_unit: Option<String>,
    role: String,
}

fn parse_jcamp_text(text: &str) -> Result<ParsedJcamp> {
    if is_link_jcamp(text) {
        return parse_link_jcamp_text(text);
    }
    if text.lines().any(|line| {
        line.trim().strip_prefix("##").is_some_and(|body| {
            normalize_key(body.split_once('=').map_or(body, |(key, _)| key)) == "ntuples"
        })
    }) {
        return parse_ntuples_jcamp_text(text);
    }

    parse_xy_jcamp_text(text)
}

fn parse_xy_jcamp_text(text: &str) -> Result<ParsedJcamp> {
    let mut ldr = BTreeMap::<String, String>::new();
    let mut xy_lines = Vec::new();
    let mut in_xy = false;
    let mut has_xypoints = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("$$") || line.is_empty() {
            continue;
        }
        if let Some(body) = line.strip_prefix("##") {
            if body.to_ascii_uppercase().starts_with("END") {
                break;
            }
            in_xy = false;
            if let Some((key, value)) = body.split_once('=') {
                let normalized = normalize_key(key);
                if normalized == "xydata" || normalized == "xypoints" {
                    in_xy = true;
                    has_xypoints = normalized == "xypoints";
                }
                ldr.insert(normalized, value.trim().to_string());
            }
        } else if in_xy {
            xy_lines.push(line.to_string());
        }
    }

    let yfactor = get_f64(&ldr, "yfactor").unwrap_or(1.0);

    let mut warnings = Vec::new();
    let (mut axis, mut values) = if has_xypoints {
        parse_xypoints_lines(&xy_lines, yfactor)?
    } else {
        let firstx = get_f64(&ldr, "firstx").ok_or_else(|| {
            nirs4all_io_core::Error::InvalidRecord("JCAMP missing FIRSTX".to_string())
        })?;
        let deltax = get_f64(&ldr, "deltax").unwrap_or_else(|| {
            let lastx = get_f64(&ldr, "lastx").unwrap_or(firstx);
            let npoints = get_f64(&ldr, "npoints").unwrap_or(1.0).max(1.0);
            if npoints <= 1.0 {
                1.0
            } else {
                (lastx - firstx) / (npoints - 1.0)
            }
        });
        let values = decode_xy_lines(&xy_lines, yfactor, "signal", &mut warnings);
        let axis: Vec<f64> = (0..values.len())
            .map(|idx| firstx + deltax * idx as f64)
            .collect();
        (axis, values)
    };
    if values.is_empty() {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "JCAMP XYDATA contains no plain AFFN values; packed DIF/DUP support is pending"
                .to_string(),
        ));
    }

    if let Some(expected) = get_f64(&ldr, "npoints").map(|value| value as usize) {
        if values.len() > expected {
            warnings.push(format!(
                "npoints_truncated: declared {expected}, decoded {}",
                values.len()
            ));
            values.truncate(expected);
            axis.truncate(expected);
        } else if values.len() < expected {
            warnings.push(format!(
                "npoints_mismatch: declared {expected}, parsed {}",
                values.len()
            ));
        }
    }

    let xunits = ldr.get("xunits").map(String::as_str).unwrap_or("index");
    let (axis_kind, axis_unit) = axis_kind_unit(xunits);
    let yunits = ldr.get("yunits").map(String::as_str).unwrap_or("");
    let signal_type = signal_type_from_yunits(yunits);
    let mut metadata = BTreeMap::new();
    metadata.insert("jcamp".to_string(), json!(ldr));
    Ok(ParsedJcamp {
        axis,
        axis_unit,
        axis_kind,
        signals: vec![ParsedJcampSignal {
            name: "signal".to_string(),
            values,
            signal_type,
            signal_unit: (!yunits.is_empty()).then(|| yunits.to_string()),
            role: "signal".to_string(),
        }],
        metadata,
        warnings,
    })
}

fn is_link_jcamp(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        let Some(body) = line.strip_prefix("##") else {
            return false;
        };
        let Some((key, value)) = body.split_once('=') else {
            return false;
        };
        normalize_key(key) == "data_type" && value.trim().eq_ignore_ascii_case("LINK")
    })
}

fn parse_link_jcamp_text(text: &str) -> Result<ParsedJcamp> {
    let chunks = link_block_chunks(text);
    if chunks.is_empty() {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "JCAMP LINK contains no child blocks".to_string(),
        ));
    }

    let mut axis = Vec::new();
    let mut axis_unit = String::new();
    let mut axis_kind = AxisKind::Index;
    let mut signals = Vec::<ParsedJcampSignal>::new();
    let mut block_metadata = Vec::<BTreeMap<String, String>>::new();
    let mut warnings = Vec::<String>::new();

    for (index, chunk) in chunks.iter().enumerate() {
        let ldr = collect_ldr(chunk);
        let parsed = parse_xy_jcamp_text(chunk)?;
        if parsed.signals.len() != 1 {
            return Err(nirs4all_io_core::Error::InvalidRecord(
                "JCAMP LINK child block did not decode to one signal".to_string(),
            ));
        }
        if axis.is_empty() {
            axis = parsed.axis;
            axis_unit = parsed.axis_unit;
            axis_kind = parsed.axis_kind;
        } else if !same_axis(&axis, &parsed.axis) {
            return Err(nirs4all_io_core::Error::InvalidRecord(
                "JCAMP LINK child blocks have incompatible axes".to_string(),
            ));
        }
        warnings.extend(parsed.warnings);

        let mut signal = parsed.signals.into_iter().next().expect("checked");
        let name = link_signal_name(&ldr, index);
        let (signal_type, unit) = link_signal_type_and_unit(&name, &ldr);
        signal.name = name.clone();
        signal.role = name;
        signal.signal_type = signal_type;
        signal.signal_unit = unit;
        signals.push(signal);
        block_metadata.push(ldr);
    }

    if let Some((processed, undefined_count)) = compute_ocean_link_transmittance(&signals) {
        if undefined_count > 0 {
            warnings.push(format!(
                "jcamp_link_processed_zero_denominator: {undefined_count} points set to 0"
            ));
        }
        signals.push(ParsedJcampSignal {
            name: "processed".to_string(),
            values: processed,
            signal_type: SignalType::Transmittance,
            signal_unit: Some("%".to_string()),
            role: "processed".to_string(),
        });
    }

    let mut metadata = BTreeMap::new();
    metadata.insert(
        "jcamp_link".to_string(),
        json!({
            "block_count": block_metadata.len(),
            "blocks": block_metadata,
        }),
    );
    Ok(ParsedJcamp {
        axis,
        axis_unit,
        axis_kind,
        signals,
        metadata,
        warnings,
    })
}

fn link_block_chunks(text: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    for raw in text.lines() {
        current.push(raw.to_string());
        if raw.trim().to_ascii_uppercase().starts_with("##END=") {
            if current
                .iter()
                .any(|line| line.trim().to_ascii_uppercase().starts_with("##JCAMP-DX="))
            {
                chunks.push(current.join("\n"));
            }
            current.clear();
        }
    }
    chunks
}

fn collect_ldr(text: &str) -> BTreeMap<String, String> {
    let mut ldr = BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        let Some(body) = line.strip_prefix("##") else {
            continue;
        };
        let Some((key, value)) = body.split_once('=') else {
            continue;
        };
        ldr.insert(
            normalize_key(key),
            strip_inline_comment(value).trim().to_string(),
        );
    }
    ldr
}

fn same_axis(left: &[f64], right: &[f64]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(a, b)| (a - b).abs() <= a.abs().max(b.abs()).max(1.0) * 1e-6)
}

fn link_signal_name(ldr: &BTreeMap<String, String>, index: usize) -> String {
    let title = ldr.get("title").map(String::as_str).unwrap_or("");
    let sample_description = ldr
        .get("sample_description")
        .map(String::as_str)
        .unwrap_or("");
    let origin = ldr.get("origin").map(String::as_str).unwrap_or("");
    let combined = format!("{title}\n{sample_description}").to_ascii_uppercase();
    if combined.contains("DARK") {
        "dark_reference".to_string()
    } else if combined.contains("REFERENCE") {
        "white_reference".to_string()
    } else if combined.contains("PROCESSED") && origin.eq_ignore_ascii_case("OCEANOPTICS EXPORT") {
        "sample".to_string()
    } else if combined.contains("PROCESSED") {
        "processed".to_string()
    } else {
        format!("signal_{}", index + 1)
    }
}

fn link_signal_type_and_unit(
    name: &str,
    ldr: &BTreeMap<String, String>,
) -> (SignalType, Option<String>) {
    if matches!(name, "sample" | "dark_reference" | "white_reference") {
        return (SignalType::RawCounts, None);
    }
    let yunits = ldr.get("yunits").map(String::as_str).unwrap_or("");
    let signal_type = signal_type_from_yunits(yunits);
    let unit = if yunits.trim().is_empty() {
        None
    } else {
        Some(yunits.to_string())
    };
    (signal_type, unit)
}

fn compute_ocean_link_transmittance(signals: &[ParsedJcampSignal]) -> Option<(Vec<f64>, usize)> {
    let sample = signals.iter().find(|signal| signal.name == "sample")?;
    let dark = signals
        .iter()
        .find(|signal| signal.name == "dark_reference")?;
    let white = signals
        .iter()
        .find(|signal| signal.name == "white_reference")?;
    if sample.values.len() != dark.values.len() || sample.values.len() != white.values.len() {
        return None;
    }

    let mut undefined_count = 0usize;
    let processed = sample
        .values
        .iter()
        .zip(&dark.values)
        .zip(&white.values)
        .map(|((sample, dark), white)| {
            let denominator = white - dark;
            if denominator.abs() <= f64::EPSILON {
                undefined_count += 1;
                0.0
            } else {
                (sample - dark) / denominator * 100.0
            }
        })
        .collect();
    Some((processed, undefined_count))
}

struct NtuplePage {
    descriptor: String,
    data_symbol: Option<String>,
    lines: Vec<String>,
}

fn parse_ntuples_jcamp_text(text: &str) -> Result<ParsedJcamp> {
    let mut ldr = BTreeMap::<String, String>::new();
    let mut pages = Vec::<NtuplePage>::new();
    let mut current_page: Option<NtuplePage> = None;
    let mut in_data = false;

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("$$") || line.is_empty() {
            continue;
        }
        if let Some(body) = line.strip_prefix("##") {
            let upper = body.to_ascii_uppercase();
            if upper.starts_with("END") {
                if let Some(page) = current_page.take() {
                    if !page.lines.is_empty() {
                        pages.push(page);
                    }
                }
                if upper.starts_with("END=") {
                    break;
                }
                in_data = false;
                continue;
            }

            in_data = false;
            if let Some((key, value)) = body.split_once('=') {
                let normalized = normalize_key(key);
                let value = strip_inline_comment(value).trim().to_string();
                if normalized == "page" {
                    if let Some(page) = current_page.take() {
                        if !page.lines.is_empty() {
                            pages.push(page);
                        }
                    }
                    current_page = Some(NtuplePage {
                        descriptor: value.clone(),
                        data_symbol: None,
                        lines: Vec::new(),
                    });
                } else if normalized == "data_table" {
                    if current_page.is_none() {
                        current_page = Some(NtuplePage {
                            descriptor: String::new(),
                            data_symbol: None,
                            lines: Vec::new(),
                        });
                    }
                    if let Some(page) = current_page.as_mut() {
                        page.data_symbol = data_symbol_from_table(&value);
                    }
                    in_data = true;
                }
                ldr.insert(normalized, value);
            }
        } else if in_data {
            if let Some(page) = current_page.as_mut() {
                page.lines.push(line.to_string());
            }
        }
    }
    if let Some(page) = current_page.take() {
        if !page.lines.is_empty() {
            pages.push(page);
        }
    }

    if pages.is_empty() {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "JCAMP NTUPLES contains no DATA TABLE pages".to_string(),
        ));
    }

    let var_names = ldr_list(&ldr, "var_name");
    let symbols = ldr_list(&ldr, "symbol");
    let var_types = ldr_list(&ldr, "var_type");
    let var_dims = ldr_list(&ldr, "var_dim");
    let units = ldr_list(&ldr, "units");
    let first = ldr_list(&ldr, "first");
    let last = ldr_list(&ldr, "last");
    let factors = ldr_list(&ldr, "factor");

    let x_index = var_types
        .iter()
        .position(|value| value.to_ascii_uppercase().contains("INDEPENDENT"))
        .or_else(|| {
            symbols
                .iter()
                .position(|symbol| symbol.eq_ignore_ascii_case("X"))
        })
        .unwrap_or(0);

    let npoints = parse_list_number(&var_dims, x_index)
        .map(|value| value as usize)
        .unwrap_or_else(|| {
            pages
                .iter()
                .map(|page| page.lines.len())
                .max()
                .unwrap_or(1)
                .max(1)
        });
    let firstx = parse_list_number(&first, x_index).ok_or_else(|| {
        nirs4all_io_core::Error::InvalidRecord("JCAMP NTUPLES missing FIRST axis".to_string())
    })?;
    let lastx = parse_list_number(&last, x_index).unwrap_or(firstx + npoints as f64 - 1.0);
    let step = if npoints <= 1 {
        1.0
    } else {
        (lastx - firstx) / (npoints - 1) as f64
    };
    let axis: Vec<f64> = (0..npoints).map(|idx| firstx + step * idx as f64).collect();

    let xunit_raw = units.get(x_index).map(String::as_str).unwrap_or("index");
    let (axis_kind, axis_unit) = axis_kind_unit(xunit_raw);

    let mut warnings = Vec::new();
    let mut parsed_signals = Vec::new();
    for (page_index, page) in pages.iter().enumerate() {
        let Some(signal_index) = ntuple_page_signal_index(page, &symbols, &var_types, page_index)
        else {
            warnings.push(format!("jcamp_ntuples_unmapped_page: {}", page.descriptor));
            continue;
        };
        let yfactor = parse_list_number(&factors, signal_index).unwrap_or(1.0);
        let mut values = decode_xy_lines(
            &page.lines,
            yfactor,
            page.data_symbol.as_deref().unwrap_or("page"),
            &mut warnings,
        );
        if values.len() > npoints {
            warnings.push(format!(
                "jcamp_ntuples_npoints_truncated: page {} declared {npoints}, decoded {}",
                page.data_symbol.as_deref().unwrap_or("?"),
                values.len()
            ));
            values.truncate(npoints);
        } else if values.len() < npoints {
            warnings.push(format!(
                "jcamp_ntuples_npoints_mismatch: page {} declared {npoints}, parsed {}",
                page.data_symbol.as_deref().unwrap_or("?"),
                values.len()
            ));
        }
        if values.is_empty() {
            continue;
        }

        let signal_name = var_names
            .get(signal_index)
            .map(|name| ntuple_signal_name(name, symbols.get(signal_index).map(String::as_str)))
            .unwrap_or_else(|| format!("signal_{}", page_index + 1));
        let signal_unit = units
            .get(signal_index)
            .map(String::as_str)
            .filter(|unit| !unit.trim().is_empty())
            .map(|unit| unit.to_string());
        parsed_signals.push(ParsedJcampSignal {
            name: signal_name.clone(),
            values,
            signal_type: signal_type_from_yunits(
                units
                    .get(signal_index)
                    .map(String::as_str)
                    .unwrap_or(signal_name.as_str()),
            ),
            signal_unit,
            role: signal_name,
        });
    }

    if parsed_signals.is_empty() {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "JCAMP NTUPLES contains no decoded signal pages".to_string(),
        ));
    }

    let mut metadata = BTreeMap::new();
    metadata.insert("jcamp".to_string(), json!(ldr));
    Ok(ParsedJcamp {
        axis,
        axis_unit,
        axis_kind,
        signals: parsed_signals,
        metadata,
        warnings,
    })
}

fn decode_xy_lines(
    xy_lines: &[String],
    yfactor: f64,
    context: &str,
    warnings: &mut Vec<String>,
) -> Vec<f64> {
    let mut values = Vec::new();
    let mut last_raw_y: Option<f64> = None;
    for line in xy_lines {
        let has_dif = xy_line_has_dif(line);
        let mut line_values = values_from_xy_line(line);
        if has_dif && last_raw_y.is_some() && !line_values.is_empty() {
            let checkpoint = line_values.remove(0);
            if let Some(previous) = last_raw_y {
                let tolerance = previous.abs().max(1.0) * 1e-6;
                if (checkpoint - previous).abs() > tolerance {
                    warnings.push(format!(
                        "jcamp_dif_checkpoint_mismatch: {context} previous {previous}, checkpoint {checkpoint}"
                    ));
                }
            }
        }
        for raw_y in line_values {
            last_raw_y = Some(raw_y);
            values.push(raw_y * yfactor);
        }
    }
    values
}

fn get_f64(map: &BTreeMap<String, String>, key: &str) -> Option<f64> {
    map.get(key).and_then(|value| parse_number(value))
}

fn ldr_list(map: &BTreeMap<String, String>, key: &str) -> Vec<String> {
    map.get(key)
        .map(|value| {
            value
                .split(',')
                .map(|part| part.trim().trim_matches('"').to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn parse_list_number(values: &[String], index: usize) -> Option<f64> {
    values.get(index).and_then(|value| parse_number(value))
}

fn strip_inline_comment(value: &str) -> &str {
    value.split_once("$$").map_or(value, |(head, _)| head)
}

fn data_symbol_from_table(value: &str) -> Option<String> {
    let start = value.find("++(")? + 3;
    let mut symbol = String::new();
    for ch in value[start..].chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            symbol.push(ch);
        } else if !symbol.is_empty() {
            break;
        }
    }
    (!symbol.is_empty()).then_some(symbol)
}

fn ntuple_page_signal_index(
    page: &NtuplePage,
    symbols: &[String],
    var_types: &[String],
    page_index: usize,
) -> Option<usize> {
    if let Some(symbol) = page.data_symbol.as_deref() {
        if let Some(index) = symbols
            .iter()
            .position(|candidate| candidate.eq_ignore_ascii_case(symbol))
        {
            return Some(index);
        }
    }
    var_types
        .iter()
        .enumerate()
        .filter(|(_, var_type)| {
            let upper = var_type.to_ascii_uppercase();
            !upper.contains("INDEPENDENT") && !upper.contains("PAGE")
        })
        .map(|(index, _)| index)
        .nth(page_index)
}

fn ntuple_signal_name(var_name: &str, symbol: Option<&str>) -> String {
    let lower = var_name.to_ascii_lowercase();
    if lower.contains("real") {
        "real".to_string()
    } else if lower.contains("imag") {
        "imaginary".to_string()
    } else {
        safe_signal_name(var_name, symbol.unwrap_or("signal"))
    }
}

fn values_from_xy_line(line: &str) -> Vec<f64> {
    let Some((_, rest)) = split_xy_line(line) else {
        return Vec::new();
    };
    decode_asdf_values(rest)
}

fn parse_xypoints_lines(lines: &[String], yfactor: f64) -> Result<(Vec<f64>, Vec<f64>)> {
    let mut axis = Vec::new();
    let mut values = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with("$$") {
            continue;
        }
        let fields = if line.contains(',') {
            line.split(',').collect::<Vec<_>>()
        } else {
            line.split_whitespace().collect::<Vec<_>>()
        };
        if fields.len() < 2 {
            continue;
        }
        let Some(x) = parse_number(fields[0]) else {
            continue;
        };
        let Some(y) = parse_number(fields[1]) else {
            continue;
        };
        axis.push(x);
        values.push(y * yfactor);
    }
    if axis.is_empty() {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "JCAMP XYPOINTS contains no numeric XY pairs".to_string(),
        ));
    }
    Ok((axis, values))
}

fn xy_line_has_dif(line: &str) -> bool {
    let Some((_, rest)) = split_xy_line(line) else {
        return false;
    };
    rest.chars().any(|ch| dif_digit(ch).is_some())
}

fn split_xy_line(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    if let Some(first) = line.split_whitespace().next() {
        if parse_number(first).is_some() {
            return Some((first, line[first.len()..].trim_start()));
        }
    }

    let mut end = 0usize;
    for (index, ch) in line.char_indices() {
        if index == 0 && matches!(ch, '+' | '-') {
            end = ch.len_utf8();
            continue;
        }
        if ch.is_ascii_digit() || matches!(ch, '.') {
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }
    (end > 0).then(|| line.split_at(end))
}

fn decode_asdf_values(input: &str) -> Vec<f64> {
    let mut values = Vec::new();
    let mut current: Option<i64> = None;
    let mut last_difference: i64 = 0;
    let mut index = 0usize;
    let bytes = input.as_bytes();

    while index < bytes.len() {
        let ch = input[index..].chars().next().expect("valid char boundary");
        if ch.is_whitespace() || ch == ',' {
            index += ch.len_utf8();
            continue;
        }

        if let Some(first_digit) = sqz_digit(ch) {
            let (number, next) = asdf_number(input, index, first_digit);
            current = Some(number);
            values.push(number as f64);
            index = next;
        } else if let Some(first_digit) = dif_digit(ch) {
            let (difference, next) = asdf_number(input, index, first_digit);
            let next_value = current.unwrap_or(0) + difference;
            current = Some(next_value);
            last_difference = difference;
            values.push(next_value as f64);
            index = next;
        } else if let Some(first_digit) = dup_digit(ch) {
            let (count, next) = asdf_unsigned_number(input, index, first_digit);
            if let Some(mut value) = current {
                for _ in 0..count.saturating_sub(1) {
                    value += last_difference;
                    values.push(value as f64);
                }
                current = Some(value);
            }
            index = next;
        } else if ch.is_ascii_digit() || matches!(ch, '+' | '-' | '.') {
            let (token, next) = ordinary_number_token(input, index);
            if let Some(value) = parse_number(token) {
                let int_value = value as i64;
                current = Some(int_value);
                values.push(value);
            }
            index = next;
        } else {
            index += ch.len_utf8();
        }
    }

    values
}

#[cfg(test)]
fn numbers_from_token(token: &str) -> Vec<f64> {
    if parse_number(token).is_some() {
        return vec![parse_number(token).expect("checked")];
    }
    let mut out = Vec::new();
    let mut start = 0usize;
    let bytes = token.as_bytes();
    for index in 1..bytes.len() {
        if bytes[index] == b'+' || bytes[index] == b'-' {
            if let Some(value) = parse_number(&token[start..index]) {
                out.push(value);
            }
            start = index;
        }
    }
    if start < token.len() {
        if let Some(value) = parse_number(&token[start..]) {
            out.push(value);
        }
    }
    out
}

fn asdf_number(input: &str, offset: usize, first_digit: i64) -> (i64, usize) {
    let (digits, next) = following_digits(input, offset);
    let sign = if first_digit < 0 { -1 } else { 1 };
    let magnitude =
        first_digit.abs() * 10_i64.pow(digits.len() as u32) + digits.parse::<i64>().unwrap_or(0);
    (sign * magnitude, next)
}

fn asdf_unsigned_number(input: &str, offset: usize, first_digit: i64) -> (usize, usize) {
    let (digits, next) = following_digits(input, offset);
    let value = first_digit * 10_i64.pow(digits.len() as u32) + digits.parse::<i64>().unwrap_or(0);
    (value.max(0) as usize, next)
}

fn following_digits(input: &str, offset: usize) -> (&str, usize) {
    let start = offset + input[offset..].chars().next().expect("char").len_utf8();
    let mut end = start;
    for (relative, ch) in input[start..].char_indices() {
        if ch.is_ascii_digit() {
            end = start + relative + ch.len_utf8();
        } else {
            break;
        }
    }
    (&input[start..end], end)
}

fn ordinary_number_token(input: &str, offset: usize) -> (&str, usize) {
    let mut end = offset;
    let mut seen_exponent = false;
    let mut previous_was_exponent = false;
    for (relative, ch) in input[offset..].char_indices() {
        if relative == 0 && matches!(ch, '+' | '-') {
            end = offset + ch.len_utf8();
            continue;
        }
        if ch.is_ascii_digit() || matches!(ch, '.') {
            end = offset + relative + ch.len_utf8();
            previous_was_exponent = false;
        } else if !seen_exponent && matches!(ch, 'E' | 'e') {
            seen_exponent = true;
            previous_was_exponent = true;
            end = offset + relative + ch.len_utf8();
        } else if previous_was_exponent && matches!(ch, '+' | '-') {
            previous_was_exponent = false;
            end = offset + relative + ch.len_utf8();
        } else {
            break;
        }
    }
    (&input[offset..end], end)
}

fn sqz_digit(ch: char) -> Option<i64> {
    match ch {
        '@' => Some(0),
        'A'..='I' => Some(ch as i64 - 'A' as i64 + 1),
        'a'..='i' => Some(-(ch as i64 - 'a' as i64 + 1)),
        _ => None,
    }
}

fn dif_digit(ch: char) -> Option<i64> {
    match ch {
        '%' => Some(0),
        'J'..='R' => Some(ch as i64 - 'J' as i64 + 1),
        'j'..='r' => Some(-(ch as i64 - 'j' as i64 + 1)),
        _ => None,
    }
}

fn dup_digit(ch: char) -> Option<i64> {
    match ch {
        'S'..='Z' => Some(ch as i64 - 'S' as i64 + 1),
        's' => Some(9),
        _ => None,
    }
}

fn axis_kind_unit(raw: &str) -> (AxisKind, String) {
    let upper = raw.trim().to_ascii_uppercase();
    if upper.contains("1/CM") || upper.contains("CM-1") || upper.contains("WAVENUMBER") {
        (AxisKind::Wavenumber, "cm-1".to_string())
    } else if upper.contains("MICROM") || upper == "UM" {
        (AxisKind::Wavelength, "um".to_string())
    } else if upper.contains("NM") || upper.contains("NANOM") {
        (AxisKind::Wavelength, "nm".to_string())
    } else if upper.contains("HZ") {
        (AxisKind::Frequency, "hz".to_string())
    } else if upper.contains("SECOND") || upper == "S" {
        (AxisKind::Index, "s".to_string())
    } else {
        (AxisKind::Index, "index".to_string())
    }
}

fn signal_type_from_yunits(raw: &str) -> SignalType {
    let upper = raw.trim().to_ascii_uppercase();
    if upper.contains("ABS") {
        SignalType::Absorbance
    } else if upper.contains("TRANSM") {
        SignalType::Transmittance
    } else if upper.contains("REFLECT") {
        SignalType::Reflectance
    } else {
        SignalType::Unknown
    }
}

fn dominant_signal_type(signals: &BTreeMap<String, SpectralArray>) -> SignalType {
    for preferred in [
        SignalType::Absorbance,
        SignalType::Reflectance,
        SignalType::Transmittance,
        SignalType::Irradiance,
    ] {
        if signals
            .values()
            .any(|signal| signal.signal_type == preferred)
        {
            return preferred;
        }
    }
    signals
        .values()
        .next()
        .map(|signal| signal.signal_type.clone())
        .unwrap_or(SignalType::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_signed_packed_affn_tokens() {
        assert_eq!(
            numbers_from_token("+10160+10159-12"),
            vec![10160.0, 10159.0, -12.0]
        );
    }

    #[test]
    fn decodes_squeeze_difference_and_duplicate_values() {
        assert_eq!(
            decode_asdf_values("C1276%Sj05Sl3"),
            vec![31_276.0, 31_276.0, 31_171.0, 31_138.0]
        );
        assert_eq!(
            decode_asdf_values("B254931p506547"),
            vec![2_254_931.0, -5_251_616.0]
        );
    }
}
