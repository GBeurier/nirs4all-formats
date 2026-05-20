use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{AxisKind, Confidence, FormatProbe, Result, SignalType};
use serde_json::{json, Value};

use crate::readers::util::{
    normalize_key, parse_number, read_text_lossy, single_signal_record, SingleSignalSpec,
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
        let metadata = parsed.metadata;
        let record = single_signal_record(
            "jcamp-dx",
            self.name(),
            source,
            SingleSignalSpec {
                axis_values: parsed.axis,
                axis_unit: parsed.axis_unit,
                axis_kind: parsed.axis_kind,
                values: parsed.values,
                signal_name: "signal".to_string(),
                signal_type: parsed.signal_type,
                signal_unit: parsed.signal_unit,
                role: "signal".to_string(),
            },
            BTreeMap::new(),
            metadata,
            parsed.warnings,
        )?;
        Ok(vec![record])
    }
}

struct ParsedJcamp {
    axis: Vec<f64>,
    values: Vec<f64>,
    axis_unit: String,
    axis_kind: AxisKind,
    signal_type: SignalType,
    signal_unit: Option<String>,
    metadata: BTreeMap<String, Value>,
    warnings: Vec<String>,
}

fn parse_jcamp_text(text: &str) -> Result<ParsedJcamp> {
    let mut ldr = BTreeMap::<String, String>::new();
    let mut xy_lines = Vec::new();
    let mut in_xy = false;
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
                if normalized == "xydata" {
                    in_xy = true;
                }
                ldr.insert(normalized, value.trim().to_string());
            }
        } else if in_xy {
            xy_lines.push(line.to_string());
        }
    }

    let yfactor = get_f64(&ldr, "yfactor").unwrap_or(1.0);
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

    let mut values = Vec::new();
    let mut warnings = Vec::new();
    let mut last_raw_y: Option<f64> = None;
    for line in xy_lines {
        let has_dif = xy_line_has_dif(&line);
        let mut line_values = values_from_xy_line(&line);
        if has_dif && last_raw_y.is_some() && !line_values.is_empty() {
            let checkpoint = line_values.remove(0);
            if let Some(previous) = last_raw_y {
                let tolerance = previous.abs().max(1.0) * 1e-6;
                if (checkpoint - previous).abs() > tolerance {
                    warnings.push(format!(
                        "jcamp_dif_checkpoint_mismatch: previous {previous}, checkpoint {checkpoint}"
                    ));
                }
            }
        }
        for raw_y in line_values {
            last_raw_y = Some(raw_y);
            values.push(raw_y * yfactor);
        }
    }
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
        } else if values.len() < expected {
            warnings.push(format!(
                "npoints_mismatch: declared {expected}, parsed {}",
                values.len()
            ));
        }
    }

    let axis: Vec<f64> = (0..values.len())
        .map(|idx| firstx + deltax * idx as f64)
        .collect();
    let xunits = ldr.get("xunits").map(String::as_str).unwrap_or("index");
    let (axis_kind, axis_unit) = axis_kind_unit(xunits);
    let yunits = ldr.get("yunits").map(String::as_str).unwrap_or("");
    let signal_type = signal_type_from_yunits(yunits);
    let mut metadata = BTreeMap::new();
    metadata.insert("jcamp".to_string(), json!(ldr));
    Ok(ParsedJcamp {
        axis,
        values,
        axis_unit,
        axis_kind,
        signal_type,
        signal_unit: (!yunits.is_empty()).then(|| yunits.to_string()),
        metadata,
        warnings,
    })
}

fn get_f64(map: &BTreeMap<String, String>, key: &str) -> Option<f64> {
    map.get(key).and_then(|value| parse_number(value))
}

fn values_from_xy_line(line: &str) -> Vec<f64> {
    let Some((_, rest)) = split_xy_line(line) else {
        return Vec::new();
    };
    decode_asdf_values(rest)
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
