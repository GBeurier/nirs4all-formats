//! Observed NIRSystems 6500 / Rapitec NSAS DA layout; see docs/formats/foss-nsas-da.md.
use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_formats_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralRecord,
};
use serde_json::json;

use crate::readers::util::{single_signal_record, SingleSignalSpec};
use crate::Reader;

const FORMAT: &str = "foss-nsas-da";
const HEADER: usize = 128;
const POINTS: usize = 1050;
const STRIDE: usize = 4352;
const SPECTRUM_END: usize = HEADER + POINTS * 4;

pub struct FossNsasDaReader;

impl Reader for FossNsasDaReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::foss_nsas_da"
    }

    fn sniff(&self, head: &[u8], _path: &Path) -> Option<FormatProbe> {
        supported_header(head).then(|| {
            FormatProbe::new(
                FORMAT,
                self.name(),
                Confidence::Likely,
                "observed NIRSystems 6500 NSAS DA record header (1050 points, 400–2498 nm)",
            )
        })
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(&self, path: &Path, bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
        if bytes.is_empty() || !bytes.len().is_multiple_of(STRIDE) {
            return Err(invalid("incomplete 4352-byte record or unsupported layout"));
        }
        let source = SourceFile::from_bytes(path, bytes, "primary");
        bytes
            .chunks_exact(STRIDE)
            .enumerate()
            .map(|(index, block)| {
                if !supported_header(block) {
                    return Err(invalid(&format!("unsupported header at record {index}")));
                }
                let values: Vec<f64> = block[HEADER..SPECTRUM_END]
                    .chunks_exact(4)
                    .map(|v| f32::from_le_bytes(v.try_into().expect("four-byte chunk")) as f64)
                    .collect();
                if values.iter().any(|v| !v.is_finite()) {
                    return Err(invalid(&format!("non-finite spectrum at record {index}")));
                }
                let sensors: Vec<f64> = block[80..92]
                    .chunks_exact(4)
                    .map(|v| f32::from_le_bytes(v.try_into().expect("four-byte chunk")) as f64)
                    .collect();
                if sensors.iter().any(|v| !v.is_finite()) {
                    return Err(invalid(&format!("non-finite sensor value at record {index}")));
                }
                let mut metadata = BTreeMap::new();
                metadata.insert("sample_id".into(), json!(String::from_utf8_lossy(&block[4..14]).trim()));
                metadata.insert("record_index".into(), json!(index));
                // This number repeats and skips; never use it as a unique key.
                metadata.insert("vendor_record_number".into(), json!(word(block, 2)));
                metadata.insert("instrument_model".into(), json!("NIRSYSTEMS 6500"));
                metadata.insert("scan_count".into(), json!(word(block, 76)));
                metadata.insert("sensor_values".into(), json!(sensors));
                metadata.insert("sensor_names".into(), json!(["TRANSDUCER1", "TRANSDUCER2", "TRANSDUCER3"]));
                metadata.insert("da_header_hex".into(), json!(hex(&block[..HEADER])));
                metadata.insert("da_trailer_hex".into(), json!(hex(&block[SPECTRUM_END..])));
                let mut warnings = vec![
                    "foss_nsas_da_reverse_engineered: only the observed 6500/1050-point layout is supported; unknown header fields and trailer preserved as hex".into(),
                    "foss_nsas_da_semantics: wavelength, absorbance, scan count and transducer mapping validated against the paired SPC export; sensor units and calibration workflow are unknown".into(),
                ];
                let date: Vec<_> = (64..76).step_by(2).map(|p| word(block, p)).collect();
                metadata.insert("da_date_time_words".into(), json!(date));
                if valid_date(&date) {
                    metadata.insert("acquired_local".into(), json!(format!(
                        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
                        date[2], date[1], date[0], date[3], date[4], date[5]
                    )));
                    warnings.push("foss_nsas_da_timezone_unknown: acquisition clock has no UTC offset".into());
                } else {
                    warnings.push("foss_nsas_da_invalid_date: raw date/time words preserved without normalization".into());
                }
                single_signal_record(
                    FORMAT, self.name(), source.clone(),
                    SingleSignalSpec {
                        axis_values: (0..POINTS).map(|i| 400.0 + 2.0 * i as f64).collect(),
                        axis_unit: "nm".into(), axis_kind: AxisKind::Wavelength,
                        values, signal_name: "absorbance".into(), signal_type: SignalType::Absorbance,
                        signal_unit: None, role: "absorbance".into(),
                    },
                    BTreeMap::new(), metadata, warnings,
                )
            })
            .collect()
    }
}

fn supported_header(bytes: &[u8]) -> bool {
    bytes.len() >= HEADER
        && word(bytes, 0) == 6500
        && bytes[4..14]
            .iter()
            .all(|b| b.is_ascii_graphic() || *b == b' ')
        && bytes[4..14].iter().any(u8::is_ascii_graphic)
        && [
            (14, 1050),
            (16, 400),
            (18, 2500),
            (20, 1),
            (22, 2),
            (52, 1),
            (54, 1026),
            (56, 1),
            (58, 1),
            (60, 2),
        ]
        .iter()
        .all(|&(p, v)| word(bytes, p) == v)
}

fn valid_date(d: &[u16]) -> bool {
    let [day, month, year, hour, minute, second] = *d else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    };
    (1..=9999).contains(&year) && day > 0 && day <= days && hour < 24 && minute < 60 && second < 60
}

fn word(bytes: &[u8], p: usize) -> u16 {
    u16::from_le_bytes([bytes[p], bytes[p + 1]])
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn invalid(message: &str) -> Error {
    Error::InvalidRecord(format!("FOSS NSAS DA: {message}"))
}
