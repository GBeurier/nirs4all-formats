//! Observed NSAS DA/CN layouts; see docs/formats/foss-nsas-da.md.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nirs4all_formats_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SidecarResolver, SignalType, SourceFile,
    SpectralRecord,
};
use serde_json::{json, Value};

use crate::readers::util::{safe_signal_name, single_signal_record, SingleSignalSpec};
use crate::{FsSidecars, NoSidecars, ReadOptions, Reader, SidecarRequirement};

const FORMAT: &str = "foss-nsas-da";
const HEADER: usize = 128;
const CN_HEADER: usize = 384;
const CN_ROW: usize = 128;
const MAX_CONSTITUENTS: u16 = 32;

#[derive(Clone, Copy, PartialEq)]
struct Geometry {
    points: usize,
    start: u16,
    end: u16,
}

impl Geometry {
    fn from_header(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < HEADER {
            return None;
        }
        let geometry = Self {
            points: word(bytes, 14) as usize,
            start: word(bytes, 16),
            end: word(bytes, 18),
        };
        matches!(
            (geometry.points, geometry.start, geometry.end),
            (700, 1100, 2500) | (1050, 400, 2500) | (1700, 800, 1650) | (4200, 400, 2500)
        )
        .then_some(geometry)
    }

    fn stride(self) -> usize {
        (HEADER + 4 * self.points).div_ceil(128) * 128
    }

    fn step(self) -> f64 {
        f64::from(self.end - self.start) / self.points as f64
    }
}

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
                "observed NSAS DA header, known spectral geometry and acquisition fields",
            )
        })
    }

    fn sidecar_requirements(&self, name: &Path, bytes: &[u8]) -> Vec<SidecarRequirement> {
        if !supported_header(bytes) {
            return vec![];
        }
        let Some(file) = name.file_name() else {
            return vec![];
        };
        let file = PathBuf::from(file);
        vec![SidecarRequirement::new(
            FORMAT,
            self.name(),
            "constituent_reference",
            file.with_extension("CN").to_string_lossy(),
            false,
            "Optional NSAS constituent references; spectra remain available without CN, with an explicit warning when references are declared",
        )
        .with_alternatives(vec![file.with_extension("cn").to_string_lossy().into_owned()])]
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let base = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let sidecars: Arc<dyn SidecarResolver> = Arc::new(FsSidecars::new(base));
        let mut candidates = BTreeSet::new();
        // Filesystem resolvers do not enumerate directories. Match DOS-style
        // casing here, inside the reader, without changing the shared resolver.
        let expected = path.with_extension("CN");
        let entries = std::fs::read_dir(base).map_err(|source| Error::Io {
            path: base.to_path_buf(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| Error::Io {
                path: path.to_path_buf(),
                source,
            })?;
            if entry
                .file_name()
                .eq_ignore_ascii_case(expected.file_name().unwrap_or_default())
            {
                candidates.insert(PathBuf::from(entry.file_name()));
            }
        }
        decode(
            self.name(),
            path,
            &bytes,
            load_cn(path, &sidecars, candidates)?,
        )
    }

    fn read_bytes(&self, name: &Path, bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
        let sidecars: Arc<dyn SidecarResolver> = Arc::new(NoSidecars);
        self.read_bytes_with_sidecars(name, bytes, &sidecars, &ReadOptions::default())
    }

    fn read_bytes_with_sidecars(
        &self,
        name: &Path,
        bytes: &[u8],
        sidecars: &Arc<dyn SidecarResolver>,
        options: &ReadOptions,
    ) -> Result<Vec<SpectralRecord>> {
        if options.has_reader_options() {
            return Err(invalid("read options are unsupported"));
        }
        decode(
            self.name(),
            name,
            bytes,
            load_cn(name, sidecars, cn_candidates(name, sidecars))?,
        )
    }
}

fn cn_candidates(name: &Path, sidecars: &Arc<dyn SidecarResolver>) -> BTreeSet<PathBuf> {
    let mut candidates = BTreeSet::new();
    let Some(file) = name.file_name() else {
        return candidates;
    };
    let expected = PathBuf::from(file).with_extension("CN");
    let listed = sidecars.list();
    if listed.is_empty() {
        for extension in ["CN", "cn", "Cn", "cN"] {
            let path = expected.with_extension(extension);
            if sidecars.contains(&path) {
                candidates.insert(path);
            }
        }
    }
    for path in listed {
        if path.parent().is_some_and(|p| !p.as_os_str().is_empty()) {
            continue;
        }
        if path
            .file_name()
            .is_some_and(|f| f.eq_ignore_ascii_case(expected.file_name().unwrap_or_default()))
        {
            candidates.insert(path);
        }
    }
    candidates
}

fn load_cn(
    name: &Path,
    sidecars: &Arc<dyn SidecarResolver>,
    candidates: BTreeSet<PathBuf>,
) -> Result<Option<(PathBuf, Vec<u8>)>> {
    if candidates.len() > 1 {
        return Err(invalid("ambiguous CN companions with case-colliding names"));
    }
    candidates
        .into_iter()
        .next()
        .map(|path| {
            let bytes = sidecars.read(&path)?;
            Ok((name.with_file_name(path), bytes))
        })
        .transpose()
}

struct Constituent {
    key: String,
    name: Option<String>,
    raw: String,
}

struct Cn {
    bytes: Vec<u8>,
    source: SourceFile,
    columns: Vec<Constituent>,
    names_warning: bool,
}

impl Cn {
    fn parse(path: &Path, bytes: Vec<u8>, rows: usize, count: usize) -> Result<Self> {
        if bytes.len() != CN_HEADER + CN_ROW * rows {
            return Err(invalid(
                "CN size must be 384 header bytes plus one 128-byte row per DA spectrum",
            ));
        }
        let mut keys = BTreeSet::new();
        let mut names_warning = false;
        let columns = (0..count)
            .map(|index| {
                let raw = &bytes[index * 10..index * 10 + 10];
                let text = trim_label(raw);
                let name = (!text.is_empty() && text.iter().all(u8::is_ascii_graphic_or_space))
                    .then(|| String::from_utf8_lossy(text).into_owned());
                let fallback = format!("constituent_{}", index + 1);
                let base = name
                    .as_deref()
                    .map_or_else(|| fallback.clone(), |s| safe_signal_name(s, &fallback));
                let mut key = base.clone();
                let mut suffix = 2;
                while keys.contains(&key) {
                    key = format!("{base}_{suffix}");
                    suffix += 1;
                }
                names_warning |= name.is_none() || key != base;
                keys.insert(key.clone());
                Constituent {
                    key,
                    name,
                    raw: hex(raw),
                }
            })
            .collect();
        Ok(Self {
            source: SourceFile::from_bytes(path, &bytes, "constituent_reference"),
            bytes,
            columns,
            names_warning,
        })
    }
}

fn decode(
    reader: &'static str,
    name: &Path,
    bytes: &[u8],
    cn: Option<(PathBuf, Vec<u8>)>,
) -> Result<Vec<SpectralRecord>> {
    if !supported_header(bytes) {
        return Err(invalid("unsupported or incomplete DA header"));
    }
    let geometry = Geometry::from_header(bytes).unwrap();
    let stride = geometry.stride();
    if !bytes.len().is_multiple_of(stride) {
        return Err(invalid("incomplete aligned DA record"));
    }
    let count = word(bytes, 30) as usize;
    let cn = cn
        .map(|(path, data)| Cn::parse(&path, data, bytes.len() / stride, count))
        .transpose()?;
    let source = SourceFile::from_bytes(name, bytes, "primary");
    bytes.chunks_exact(stride).enumerate().map(|(index, block)| {
        if !supported_header(block) || Geometry::from_header(block) != Some(geometry) || word(block, 30) as usize != count {
            return Err(invalid(&format!("unsupported or inconsistent header at record {index}")));
        }
        let end = HEADER + geometry.points * 4;
        let values: Vec<f64> = block[HEADER..end].as_chunks::<4>().0.iter()
            .map(|v| f32::from_le_bytes(*v) as f64).collect();
        if values.iter().any(|v| !v.is_finite()) {
            return Err(invalid(&format!("non-finite spectrum at record {index}")));
        }
        let absorbance = legacy_absorbance(block);
        let sensors: Vec<Option<f64>> = block[80..92].as_chunks::<4>().0.iter()
            .map(|v| { let v = f32::from_le_bytes(*v) as f64; v.is_finite().then_some(v) }).collect();
        if absorbance && sensors.iter().any(Option::is_none) {
            return Err(invalid(&format!("non-finite sensor value at record {index}")));
        }
        let mut metadata = BTreeMap::new();
        metadata.insert("sample_id".into(), json!(String::from_utf8_lossy(trim_label(&block[4..14]))));
        metadata.insert("record_index".into(), json!(index));
        metadata.insert("vendor_record_number".into(), json!(word(block, 2)));
        let model = word(block, 0);
        metadata.insert("instrument_model_code".into(), json!(model));
        if model != 0 {
            metadata.insert("instrument_model".into(), json!(format!("NIRSYSTEMS {model}")));
        }
        metadata.insert("scan_count".into(), json!(word(block, 76)));
        metadata.insert("sensor_values".into(), json!(sensors));
        metadata.insert("sensor_names".into(), json!(["TRANSDUCER1", "TRANSDUCER2", "TRANSDUCER3"]));
        metadata.insert("da_constituent_count".into(), json!(count));
        metadata.insert("da_point_count".into(), json!(geometry.points));
        metadata.insert("da_wavelength_start".into(), json!(geometry.start));
        metadata.insert("da_wavelength_end".into(), json!(geometry.end));
        metadata.insert("da_wavelength_step".into(), json!(geometry.step()));
        metadata.insert("da_record_bytes".into(), json!(stride));
        metadata.insert("da_header_hex".into(), json!(hex(&block[..HEADER])));
        metadata.insert("da_trailer_hex".into(), json!(hex(&block[end..])));
        let mut warnings = vec![
            "foss_nsas_da_reverse_engineered: only observed spectral geometries are supported; unknown header fields and padding preserved as hex".into(),
            if absorbance {
                "foss_nsas_da_semantics: wavelength, absorbance, scan count and transducer mapping validated against the paired SPC export; sensor units and calibration-file byte layouts remain unverified".into()
            } else {
                "foss_nsas_da_semantics: wavelength geometry follows the NSAS importer; signal semantics, sensor units and calibration-file byte layouts remain unverified".into()
            },
        ];
        if sensors.iter().any(Option::is_none) {
            warnings.push("foss_nsas_da_nonfinite_sensor: non-finite sensor fields emitted as null; original bytes retained in da_header_hex".into());
        }
        let date: Vec<_> = (64..76).step_by(2).map(|p| word(block, p)).collect();
        metadata.insert("da_date_time_words".into(), json!(date));
        if valid_date(&date) {
            metadata.insert("acquired_local".into(), json!(format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", date[2], date[1], date[0], date[3], date[4], date[5])));
            warnings.push("foss_nsas_da_timezone_unknown: acquisition clock has no UTC offset".into());
        } else {
            warnings.push("foss_nsas_da_invalid_date: raw date/time words preserved without normalization".into());
        }
        let mut targets = BTreeMap::new();
        if let Some(cn) = &cn {
            let offset = CN_HEADER + index * CN_ROW;
            let row = &cn.bytes[offset..offset + CN_ROW];
            for (i, column) in cn.columns.iter().enumerate() {
                let value = f32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap());
                if !value.is_finite() {
                    return Err(invalid(&format!("non-finite active CN value at record {index}, column {i}")));
                }
                targets.insert(column.key.clone(), json!(value as f64));
            }
            metadata.insert("cn_header_hex".into(), json!(hex(&cn.bytes[..CN_HEADER])));
            metadata.insert("cn_record_hex".into(), json!(hex(row)));
            metadata.insert("cn_constituents".into(), Value::Array(cn.columns.iter().enumerate().map(|(i, c)| json!({"index": i, "name": c.name, "target_key": c.key, "name_hex": c.raw})).collect()));
            warnings.push("foss_nsas_cn_experimental: references joined by physical record order; target units and missing-value conventions are unknown; vendor corpus validates only zero or one active constituent".into());
            if cn.names_warning {
                warnings.push("foss_nsas_cn_names: blank, undecoded or duplicate labels use unique target keys; original name bytes preserved".into());
            }
        } else if count > 0 {
            warnings.push("foss_nsas_da_missing_cn: DA declares constituents but no matching CN was supplied; spectra recovered without reference targets".into());
        }
        let signal_name = if absorbance { "absorbance" } else { "spectrum" };
        let mut record = single_signal_record(
            FORMAT, reader, source.clone(),
            SingleSignalSpec {
                axis_values: (0..geometry.points).map(|i| f64::from(geometry.start) + geometry.step() * i as f64).collect(),
                axis_unit: "nm".into(), axis_kind: AxisKind::Wavelength,
                values, signal_name: signal_name.into(),
                signal_type: if absorbance { SignalType::Absorbance } else { SignalType::Unknown },
                signal_unit: None, role: signal_name.into(),
            }, targets, metadata, warnings,
        )?;
        if let Some(cn) = &cn {
            record.provenance.sources.push(cn.source.clone());
        }
        Ok(record)
    }).collect()
}

fn supported_header(bytes: &[u8]) -> bool {
    Geometry::from_header(bytes).is_some()
        && matches!(word(bytes, 0), 0 | 5000 | 6500)
        && valid_label(&bytes[4..14])
        && matches!(word(bytes, 20), 1 | 11)
        && matches!(word(bytes, 22), 0 | 2)
        && word(bytes, 30) <= MAX_CONSTITUENTS
        && matches!(word(bytes, 52), 1 | 2 | 769)
        && matches!(word(bytes, 54), 0 | 1025 | 1026 | 1030 | 65535)
        && matches!(word(bytes, 56), 0 | 1)
        && matches!(word(bytes, 58), 0 | 1 | 65535)
        && matches!(word(bytes, 60), 0 | 2 | 65534)
}

fn legacy_absorbance(bytes: &[u8]) -> bool {
    [
        (0, 6500),
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

fn trim_label(raw: &[u8]) -> &[u8] {
    let end = raw
        .iter()
        .rposition(|b| *b != 0 && *b != b' ')
        .map_or(0, |i| i + 1);
    let start = raw[..end].iter().position(|b| *b != b' ').unwrap_or(end);
    &raw[start..end]
}

fn valid_label(raw: &[u8]) -> bool {
    let text = trim_label(raw);
    !text.is_empty() && text.iter().all(u8::is_ascii_graphic_or_space)
}

trait AsciiLabel {
    fn is_ascii_graphic_or_space(&self) -> bool;
}
impl AsciiLabel for u8 {
    fn is_ascii_graphic_or_space(&self) -> bool {
        self.is_ascii_graphic() || *self == b' '
    }
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
    Error::InvalidRecord(format!("FOSS NSAS DA/CN: {message}"))
}
