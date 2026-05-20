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

    let xfactor = get_f64(&ldr, "xfactor").unwrap_or(1.0);
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
    for line in xy_lines {
        let numbers = numbers_from_xy_line(&line);
        if numbers.len() < 2 {
            continue;
        }
        for y in numbers.iter().skip(1) {
            values.push(y * yfactor);
        }
    }
    if values.is_empty() {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "JCAMP XYDATA contains no plain AFFN values; packed DIF/DUP support is pending"
                .to_string(),
        ));
    }

    let axis: Vec<f64> = (0..values.len())
        .map(|idx| (firstx + deltax * idx as f64) * xfactor)
        .collect();
    let xunits = ldr.get("xunits").map(String::as_str).unwrap_or("index");
    let (axis_kind, axis_unit) = axis_kind_unit(xunits);
    let yunits = ldr.get("yunits").map(String::as_str).unwrap_or("");
    let signal_type = signal_type_from_yunits(yunits);
    let mut warnings = Vec::new();
    if let Some(expected) = get_f64(&ldr, "npoints") {
        if (expected as usize) != values.len() {
            warnings.push(format!(
                "npoints_mismatch: declared {}, parsed {}",
                expected as usize,
                values.len()
            ));
        }
    }
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

fn numbers_from_xy_line(line: &str) -> Vec<f64> {
    let mut out = Vec::new();
    for token in line.split_whitespace() {
        out.extend(numbers_from_token(token));
    }
    out
}

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
}
