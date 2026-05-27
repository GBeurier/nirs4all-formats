use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_formats_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralArray,
    SpectralAxis,
};
use serde_json::json;

use crate::readers::util::{record_from_signals, safe_signal_name};
use crate::Reader;

pub struct AvantesBinaryReader;

impl Reader for AvantesBinaryReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::avantes_binary"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        if head.starts_with(b"AVS82") || head.starts_with(b"AVS84") {
            return Some(FormatProbe::new(
                "avantes-avasoft8-binary",
                self.name(),
                Confidence::Definite,
                "AvaSoft 8 binary header detected",
            ));
        }

        let ext = lower_extension(path);
        if !matches!(ext.as_str(), "trm" | "abs" | "roh" | "drk" | "ref") {
            return None;
        }
        is_plausible_legacy_header(head).then(|| {
            FormatProbe::new(
                "avantes-legacy-binary",
                self.name(),
                Confidence::Definite,
                "AvaSoft 6/7 legacy binary header detected",
            )
        })
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(
        &self,
        path: &Path,
        bytes: &[u8],
    ) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
        let source = SourceFile::from_bytes(path, bytes, "primary");
        if bytes.starts_with(b"AVS82") || bytes.starts_with(b"AVS84") {
            read_avasoft8(self.name(), source, path, bytes)
        } else {
            read_legacy(self.name(), source, path, bytes)
        }
    }
}

fn lower_extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn is_plausible_legacy_header(head: &[u8]) -> bool {
    if head.len() < 400 {
        return false;
    }
    let Ok(version) = f32_at(head, 0) else {
        return false;
    };
    let Ok(first_pixel) = f32_at(head, 316) else {
        return false;
    };
    let Ok(last_pixel) = f32_at(head, 320) else {
        return false;
    };
    let Ok(a0) = f32_at(head, 296) else {
        return false;
    };
    (version - 70.0).abs() < 0.01
        && first_pixel >= 0.0
        && last_pixel >= first_pixel
        && last_pixel < 100_000.0
        && a0.is_finite()
}

fn read_legacy(
    reader: &str,
    source: SourceFile,
    path: &Path,
    bytes: &[u8],
) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
    if bytes.len() < 400 {
        return Err(Error::InvalidRecord(
            "AvaSoft legacy file is shorter than the 400-byte header".to_string(),
        ));
    }
    let floats = decode_f32_slice(bytes)?;
    let first_pixel = floats[79].round() as usize;
    let last_pixel = floats[80].round() as usize;
    if last_pixel < first_pixel {
        return Err(Error::InvalidRecord(
            "AvaSoft legacy last pixel is before first pixel".to_string(),
        ));
    }
    let point_count = last_pixel - first_pixel + 1;
    let coeffs = [floats[74], floats[75], floats[76], floats[77], floats[78]];
    let axis_values = wavelengths_from_coefficients(&coeffs, first_pixel, point_count);
    let ext = lower_extension(path);
    let mode = legacy_mode(&ext);
    let data = &floats[100..];

    let mut signals = BTreeMap::new();
    let mut warnings = Vec::new();
    let dominant;
    if matches!(mode, LegacyMode::Transmittance | LegacyMode::Absorbance) {
        if data.len() < point_count * 3 {
            return Err(Error::InvalidRecord(format!(
                "AvaSoft legacy processed payload has {} floats; expected at least {}",
                data.len(),
                point_count * 3
            )));
        }
        let mut sample = Vec::with_capacity(point_count);
        let mut white = Vec::with_capacity(point_count);
        let mut dark = Vec::with_capacity(point_count);
        for triple in data[..point_count * 3].chunks_exact(3) {
            sample.push(triple[0] as f64);
            white.push(triple[1] as f64);
            dark.push(triple[2] as f64);
        }
        dominant = match mode {
            LegacyMode::Absorbance => SignalType::Absorbance,
            _ => SignalType::Transmittance,
        };
        let processed = compute_processed(&sample, &white, &dark, &dominant);
        signals.insert(
            legacy_processed_name(&mode).to_string(),
            make_signal(
                &axis_values,
                processed,
                dominant.clone(),
                legacy_processed_unit(&mode),
                legacy_processed_name(&mode),
            )?,
        );
        signals.insert(
            "sample".to_string(),
            make_signal(&axis_values, sample, SignalType::RawCounts, None, "sample")?,
        );
        signals.insert(
            "white_reference".to_string(),
            make_signal(
                &axis_values,
                white,
                SignalType::RawCounts,
                None,
                "white_reference",
            )?,
        );
        signals.insert(
            "dark_reference".to_string(),
            make_signal(
                &axis_values,
                dark,
                SignalType::RawCounts,
                None,
                "dark_reference",
            )?,
        );
        let trailing = data.len().saturating_sub(point_count * 3);
        if trailing > 3 {
            warnings.push(format!(
                "avantes_legacy_unparsed_trailing_floats:{trailing}"
            ));
        }
    } else {
        if data.len() < point_count {
            return Err(Error::InvalidRecord(format!(
                "AvaSoft legacy raw payload has {} floats; expected at least {point_count}",
                data.len()
            )));
        }
        let values: Vec<f64> = data[..point_count]
            .iter()
            .map(|value| *value as f64)
            .collect();
        let (name, role) = legacy_raw_signal_name(&mode);
        dominant = SignalType::RawCounts;
        signals.insert(
            name.to_string(),
            make_signal(&axis_values, values, SignalType::RawCounts, None, role)?,
        );
        let trailing = data.len().saturating_sub(point_count);
        if trailing > 3 {
            warnings.push(format!(
                "avantes_legacy_unparsed_trailing_floats:{trailing}"
            ));
        }
        // Raw-mode legacy files only carry a single channel. Flag that
        // downstream consumers need companion files to recompute processed
        // signals (transmittance/absorbance/reflectance).
        if matches!(
            mode,
            LegacyMode::RawScope | LegacyMode::DarkReference | LegacyMode::WhiteReference
        ) {
            warnings.push(format!(
                "avantes_legacy_single_channel:{}:companion_files_required",
                legacy_mode_label(&mode)
            ));
        }
    }

    let metadata = legacy_metadata(&floats, &coeffs, first_pixel, last_pixel, &mode);
    let record = record_from_signals(
        "avantes-legacy-binary",
        reader,
        source,
        signals,
        dominant,
        metadata,
        warnings,
    )?;
    Ok(vec![record])
}

/// Human-readable label for the legacy measurement mode, promoted at the top
/// level of the record metadata.
fn legacy_mode_label(mode: &LegacyMode) -> &'static str {
    match mode {
        LegacyMode::Absorbance => "absorbance",
        LegacyMode::Transmittance => "transmittance",
        LegacyMode::RawScope => "raw_scope",
        LegacyMode::DarkReference => "dark_reference",
        LegacyMode::WhiteReference => "white_reference",
    }
}

/// Human-readable label for the AvaSoft 8 measurement mode, promoted at the
/// top level of the record metadata.
fn avasoft8_mode_label(mode: &Avasoft8Mode) -> &'static str {
    match mode {
        Avasoft8Mode::Raw => "raw_scope",
        Avasoft8Mode::Absorbance => "absorbance",
        Avasoft8Mode::Transmittance => "transmittance",
        Avasoft8Mode::Reflectance => "reflectance",
        Avasoft8Mode::Irradiance => "irradiance",
    }
}

/// Map a file extension to the AvaSoft 8 mode it is expected to encode.
/// Returns `None` for extensions whose mode is not unambiguously implied.
fn avasoft8_expected_mode(ext: &str) -> Option<Avasoft8Mode> {
    match ext {
        "raw8" | "rwd8" => Some(Avasoft8Mode::Raw),
        "abs8" => Some(Avasoft8Mode::Absorbance),
        "trm8" => Some(Avasoft8Mode::Transmittance),
        "rfl8" => Some(Avasoft8Mode::Reflectance),
        "irr8" | "rir8" => Some(Avasoft8Mode::Irradiance),
        _ => None,
    }
}

fn avasoft8_modes_match(extension: &Avasoft8Mode, observed: &Avasoft8Mode) -> bool {
    matches!(
        (extension, observed),
        (Avasoft8Mode::Raw, Avasoft8Mode::Raw)
            | (Avasoft8Mode::Absorbance, Avasoft8Mode::Absorbance)
            | (Avasoft8Mode::Transmittance, Avasoft8Mode::Transmittance)
            | (Avasoft8Mode::Reflectance, Avasoft8Mode::Reflectance)
            | (Avasoft8Mode::Irradiance, Avasoft8Mode::Irradiance)
    )
}

fn read_avasoft8(
    reader: &str,
    source: SourceFile,
    path: &Path,
    bytes: &[u8],
) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
    if bytes.len() < 328 {
        return Err(Error::InvalidRecord(
            "AvaSoft 8 file is shorter than the first subfile header".to_string(),
        ));
    }
    let magic = std::str::from_utf8(&bytes[..5]).unwrap_or("AVS8?");
    let spectra_count = bytes[5] as usize;
    let extension = lower_extension(path);
    let expected_mode = avasoft8_expected_mode(&extension);
    let mut offset = 6usize;
    let mut records = Vec::new();
    for index in 0..spectra_count {
        if offset + 328 > bytes.len() {
            return Err(Error::InvalidRecord(format!(
                "AvaSoft 8 subfile {index} header exceeds file length"
            )));
        }
        let length = u32_at(bytes, offset)? as usize;
        let sub_end_with_merge = offset
            .checked_add(length)
            .and_then(|value| value.checked_add(10))
            .ok_or_else(|| Error::InvalidRecord("AvaSoft 8 subfile length overflow".to_string()))?;
        if sub_end_with_merge > bytes.len() {
            return Err(Error::InvalidRecord(format!(
                "AvaSoft 8 subfile {index} length exceeds file length"
            )));
        }

        let start_pixel = u16_at(bytes, offset + 83)? as usize;
        let stop_pixel = u16_at(bytes, offset + 85)? as usize;
        if stop_pixel < start_pixel {
            return Err(Error::InvalidRecord(format!(
                "AvaSoft 8 subfile {index} stop pixel is before start pixel"
            )));
        }
        let point_count = stop_pixel - start_pixel + 1;
        let data_offset = offset + 322;
        let needed = data_offset
            .checked_add(point_count * 4 * 4)
            .ok_or_else(|| Error::InvalidRecord("AvaSoft 8 payload overflow".to_string()))?;
        if needed > bytes.len() {
            return Err(Error::InvalidRecord(format!(
                "AvaSoft 8 subfile {index} payload exceeds file length"
            )));
        }
        let x = read_f32_vec(bytes, data_offset, point_count)?;
        let sample = read_f32_vec(bytes, data_offset + point_count * 4, point_count)?;
        let dark = read_f32_vec(bytes, data_offset + point_count * 8, point_count)?;
        let reference = read_f32_vec(bytes, data_offset + point_count * 12, point_count)?;

        let mode = avasoft8_mode(bytes[offset + 5]);
        let mut signals = BTreeMap::new();
        let dominant = match mode {
            Avasoft8Mode::Absorbance => SignalType::Absorbance,
            Avasoft8Mode::Transmittance => SignalType::Transmittance,
            Avasoft8Mode::Reflectance => SignalType::Reflectance,
            Avasoft8Mode::Irradiance => SignalType::Irradiance,
            Avasoft8Mode::Raw => SignalType::RawCounts,
        };
        let primary_name = avasoft8_primary_name(&mode);
        let primary_values = match mode {
            Avasoft8Mode::Absorbance | Avasoft8Mode::Transmittance | Avasoft8Mode::Reflectance => {
                compute_processed(&sample, &reference, &dark, &dominant)
            }
            Avasoft8Mode::Irradiance | Avasoft8Mode::Raw => sample.clone(),
        };
        signals.insert(
            primary_name.to_string(),
            make_signal(
                &x,
                primary_values,
                dominant.clone(),
                avasoft8_primary_unit(&mode),
                primary_name,
            )?,
        );
        signals.insert(
            "sample".to_string(),
            make_signal(&x, sample, SignalType::RawCounts, None, "sample")?,
        );
        signals.insert(
            "dark_reference".to_string(),
            make_signal(&x, dark, SignalType::RawCounts, None, "dark_reference")?,
        );
        // In irradiance mode the fourth array is the per-pixel calibration
        // vector (values that span ~1e10..1e0), not a raw white reference. We
        // promote it under its true role and keep `white_reference` for the
        // reflectance/transmittance/absorbance modes where it really is the
        // raw white scan.
        if matches!(mode, Avasoft8Mode::Irradiance) {
            signals.insert(
                "irradiance_calibration".to_string(),
                make_signal(
                    &x,
                    reference,
                    SignalType::Unknown,
                    None,
                    "irradiance_calibration",
                )?,
            );
        } else {
            signals.insert(
                "white_reference".to_string(),
                make_signal(
                    &x,
                    reference,
                    SignalType::RawCounts,
                    None,
                    "white_reference",
                )?,
            );
        }

        let mut warnings = Vec::new();
        if matches!(mode, Avasoft8Mode::Irradiance) {
            warnings.push("avantes_irr8_irradiance_calibration_not_applied".to_string());
        }
        if let Some(expected) = expected_mode.as_ref() {
            if !avasoft8_modes_match(expected, &mode) {
                warnings.push(format!(
                    "avantes_avasoft8_extension_mode_mismatch:expected={}:observed={}",
                    avasoft8_mode_label(expected),
                    avasoft8_mode_label(&mode)
                ));
            }
        }
        let metadata =
            avasoft8_metadata(bytes, offset, magic, length, start_pixel, stop_pixel, &mode)?;
        records.push(record_from_signals(
            "avantes-avasoft8-binary",
            reader,
            source.clone(),
            signals,
            dominant,
            metadata,
            warnings,
        )?);

        offset = sub_end_with_merge;
    }
    Ok(records)
}

#[derive(Clone, Debug)]
enum LegacyMode {
    Transmittance,
    Absorbance,
    RawScope,
    DarkReference,
    WhiteReference,
}

fn legacy_mode(ext: &str) -> LegacyMode {
    match ext {
        "abs" => LegacyMode::Absorbance,
        "trm" => LegacyMode::Transmittance,
        "drk" => LegacyMode::DarkReference,
        "ref" => LegacyMode::WhiteReference,
        _ => LegacyMode::RawScope,
    }
}

fn legacy_processed_name(mode: &LegacyMode) -> &'static str {
    match mode {
        LegacyMode::Absorbance => "absorbance",
        _ => "transmittance",
    }
}

fn legacy_processed_unit(mode: &LegacyMode) -> Option<String> {
    match mode {
        LegacyMode::Transmittance => Some("%".to_string()),
        _ => None,
    }
}

fn legacy_raw_signal_name(mode: &LegacyMode) -> (&'static str, &'static str) {
    match mode {
        LegacyMode::DarkReference => ("dark_reference", "dark_reference"),
        LegacyMode::WhiteReference => ("white_reference", "white_reference"),
        _ => ("scope", "scope"),
    }
}

#[derive(Clone, Debug)]
enum Avasoft8Mode {
    Raw,
    Absorbance,
    Transmittance,
    Reflectance,
    Irradiance,
}

fn avasoft8_mode(mode: u8) -> Avasoft8Mode {
    match mode {
        1 => Avasoft8Mode::Absorbance,
        2 => Avasoft8Mode::Transmittance,
        3 => Avasoft8Mode::Reflectance,
        4 | 5 => Avasoft8Mode::Irradiance,
        _ => Avasoft8Mode::Raw,
    }
}

fn avasoft8_primary_name(mode: &Avasoft8Mode) -> &'static str {
    match mode {
        Avasoft8Mode::Absorbance => "absorbance",
        Avasoft8Mode::Transmittance => "transmittance",
        Avasoft8Mode::Reflectance => "reflectance",
        Avasoft8Mode::Irradiance => "irradiance",
        Avasoft8Mode::Raw => "scope",
    }
}

fn avasoft8_primary_unit(mode: &Avasoft8Mode) -> Option<String> {
    match mode {
        Avasoft8Mode::Transmittance | Avasoft8Mode::Reflectance => Some("%".to_string()),
        _ => None,
    }
}

fn compute_processed(
    sample: &[f64],
    white: &[f64],
    dark: &[f64],
    signal_type: &SignalType,
) -> Vec<f64> {
    sample
        .iter()
        .zip(white)
        .zip(dark)
        .map(|((sample, white), dark)| {
            let denominator = white - dark;
            let ratio = if denominator == 0.0 {
                f64::NAN
            } else {
                (sample - dark) / denominator
            };
            if *signal_type == SignalType::Absorbance {
                if ratio > 0.0 {
                    -ratio.log10()
                } else {
                    f64::NAN
                }
            } else {
                ratio * 100.0
            }
        })
        .collect()
}

fn make_signal(
    axis_values: &[f64],
    values: Vec<f64>,
    signal_type: SignalType,
    unit: Option<String>,
    role: &str,
) -> Result<SpectralArray> {
    let axis = SpectralAxis::new(axis_values.to_vec(), "nm", AxisKind::Wavelength)?;
    SpectralArray::new(
        axis,
        values,
        vec!["x".to_string()],
        signal_type,
        unit,
        safe_signal_name(role, "signal"),
        "file",
    )
}

fn legacy_metadata(
    floats: &[f32],
    coeffs: &[f32; 5],
    first_pixel: usize,
    last_pixel: usize,
    mode: &LegacyMode,
) -> BTreeMap<String, serde_json::Value> {
    let mut metadata = BTreeMap::new();
    let spec_id = f32_ascii(&floats[1..10]);
    let user_name = f32_ascii(&floats[10..74]);
    let trailer_offset = legacy_trailer_offset(first_pixel, last_pixel, mode);
    let integration_time = floats.get(trailer_offset).copied();
    let averages = floats.get(trailer_offset + 1).copied();
    let integration_delay = floats.get(trailer_offset + 2).copied();
    let detector_temperature = floats.get(99).copied();
    let point_count = last_pixel - first_pixel + 1;
    let version_id = floats[0];

    if !spec_id.is_empty() {
        metadata.insert("instrument_serial".to_string(), json!(spec_id));
    }
    if !user_name.is_empty() {
        metadata.insert("operator".to_string(), json!(user_name));
    }
    metadata.insert(
        "measurement_mode".to_string(),
        json!(legacy_mode_label(mode)),
    );
    if let Some(value) = integration_time {
        if value.is_finite() {
            metadata.insert("integration_time_ms".to_string(), json!(value as f64));
        }
    }
    if let Some(value) = averages {
        if value.is_finite() && value >= 0.0 {
            metadata.insert("averages_count".to_string(), json!(value.round() as i64));
        }
    }
    if let Some(value) = integration_delay {
        if value.is_finite() {
            metadata.insert("integration_delay".to_string(), json!(value as f64));
        }
    }
    if let Some(value) = detector_temperature {
        if value.is_finite() {
            metadata.insert("detector_temperature_c".to_string(), json!(value as f64));
        }
    }
    metadata.insert("point_count".to_string(), json!(point_count));
    metadata.insert("first_pixel".to_string(), json!(first_pixel));
    metadata.insert("last_pixel".to_string(), json!(last_pixel));
    metadata.insert("version_id".to_string(), json!(version_id));

    metadata.insert(
        "avantes".to_string(),
        json!({
            "family": "AvaSoft legacy",
            "version_id": version_id,
            "spec_id": spec_id,
            "user_name": user_name,
            "wavelength_coefficients": coeffs,
            "first_pixel": first_pixel,
            "last_pixel": last_pixel,
            "measure_mode": floats[81],
            "mode": format!("{mode:?}"),
            "integration_time": integration_time,
            "averages": averages,
            "integration_delay": integration_delay,
            "smooth_pixels": floats.get(89).copied(),
            "trigger": floats.get(91).copied(),
            "detector_temperature": detector_temperature,
        }),
    );
    metadata
}

fn legacy_trailer_offset(first_pixel: usize, last_pixel: usize, mode: &LegacyMode) -> usize {
    let point_count = last_pixel - first_pixel + 1;
    let channels = match mode {
        LegacyMode::Absorbance | LegacyMode::Transmittance => 3,
        _ => 1,
    };
    100 + point_count * channels
}

fn avasoft8_metadata(
    bytes: &[u8],
    offset: usize,
    magic: &str,
    length: usize,
    start_pixel: usize,
    stop_pixel: usize,
    mode: &Avasoft8Mode,
) -> Result<BTreeMap<String, serde_json::Value>> {
    let mut metadata = BTreeMap::new();
    let spc_date = u32_at(bytes, offset + 128)?;
    let decoded_date = decode_spc_datetime(spc_date);
    let spec_id = bytes_ascii(bytes, offset + 8, 10);
    let user_name = bytes_ascii(bytes, offset + 18, 64);
    let comment = bytes_ascii(bytes, offset + 192, 130);
    let integration_time = f32_at(bytes, offset + 87)?;
    let integration_delay = u32_at(bytes, offset + 91)?;
    let averages = u32_at(bytes, offset + 95)?;
    let measure_mode_byte = bytes[offset + 5];
    let point_count = stop_pixel - start_pixel + 1;

    if let Some(datetime) = decoded_date.as_ref() {
        metadata.insert("acquisition_start_date".to_string(), json!(datetime.date));
        metadata.insert("acquisition_start_time".to_string(), json!(datetime.time));
    }
    if !spec_id.is_empty() {
        metadata.insert("instrument_serial".to_string(), json!(spec_id));
    }
    if !user_name.is_empty() {
        metadata.insert("operator".to_string(), json!(user_name));
    }
    if !comment.is_empty() {
        metadata.insert("comment".to_string(), json!(comment));
    }
    metadata.insert(
        "measurement_mode".to_string(),
        json!(avasoft8_mode_label(mode)),
    );
    if integration_time.is_finite() {
        metadata.insert(
            "integration_time_ms".to_string(),
            json!(integration_time as f64),
        );
    }
    metadata.insert("averages_count".to_string(), json!(averages));
    metadata.insert("integration_delay".to_string(), json!(integration_delay));
    metadata.insert("point_count".to_string(), json!(point_count));
    metadata.insert("first_pixel".to_string(), json!(start_pixel));
    metadata.insert("last_pixel".to_string(), json!(stop_pixel));
    metadata.insert("magic".to_string(), json!(magic));

    metadata.insert(
        "avantes".to_string(),
        json!({
            "family": "AvaSoft 8",
            "magic": magic,
            "subfile_length": length,
            "sequence": bytes[offset + 4],
            "measure_mode": measure_mode_byte,
            "bitness": bytes[offset + 6],
            "sd_marker": bytes[offset + 7],
            "spec_id": spec_id,
            "user_name": user_name,
            "status": bytes[offset + 82],
            "first_pixel": start_pixel,
            "last_pixel": stop_pixel,
            "integration_time": integration_time,
            "integration_delay": integration_delay,
            "averages": averages,
            "spc_date": spc_date,
            "spc_date_decoded": decoded_date.as_ref().map(AvasoftSpcDate::as_json),
            "comment": comment,
        }),
    );
    Ok(metadata)
}

struct AvasoftSpcDate {
    date: String,
    time: String,
    year: u32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
}

impl AvasoftSpcDate {
    fn as_json(&self) -> serde_json::Value {
        json!({
            "date": &self.date,
            "time": &self.time,
            "year": self.year,
            "month": self.month,
            "day": self.day,
            "hour": self.hour,
            "minute": self.minute,
        })
    }
}

fn decode_spc_datetime(raw: u32) -> Option<AvasoftSpcDate> {
    if raw == 0 {
        return None;
    }
    let year = (raw >> 20) & 0x0fff;
    let month = (raw >> 16) & 0x0f;
    let day = (raw >> 11) & 0x1f;
    let hour = (raw >> 6) & 0x1f;
    let minute = raw & 0x3f;
    if year == 0
        || !(1..=12).contains(&month)
        || day == 0
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
    {
        return None;
    }
    Some(AvasoftSpcDate {
        date: format!("{year:04}-{month:02}-{day:02}"),
        time: format!("{hour:02}:{minute:02}"),
        year,
        month,
        day,
        hour,
        minute,
    })
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

fn wavelengths_from_coefficients(coeffs: &[f32; 5], first_pixel: usize, count: usize) -> Vec<f64> {
    (0..count)
        .map(|index| {
            let pixel = (first_pixel + index) as f64;
            coeffs[0] as f64
                + coeffs[1] as f64 * pixel
                + coeffs[2] as f64 * pixel.powi(2)
                + coeffs[3] as f64 * pixel.powi(3)
                + coeffs[4] as f64 * pixel.powi(4)
        })
        .collect()
}

fn decode_f32_slice(bytes: &[u8]) -> Result<Vec<f32>> {
    if !bytes.len().is_multiple_of(4) {
        return Err(Error::InvalidRecord(format!(
            "AvaSoft binary length {} is not float32-aligned",
            bytes.len()
        )));
    }
    let mut values = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        values.push(f32::from_le_bytes(chunk.try_into().expect("chunk width")));
    }
    Ok(values)
}

fn read_f32_vec(bytes: &[u8], offset: usize, count: usize) -> Result<Vec<f64>> {
    (0..count)
        .map(|index| f32_at(bytes, offset + index * 4).map(|value| value as f64))
        .collect()
}

fn f32_at(bytes: &[u8], offset: usize) -> Result<f32> {
    let chunk = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| Error::InvalidRecord(format!("missing f32 at byte offset {offset}")))?;
    Ok(f32::from_le_bytes(chunk.try_into().expect("chunk width")))
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16> {
    let chunk = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| Error::InvalidRecord(format!("missing u16 at byte offset {offset}")))?;
    Ok(u16::from_le_bytes(chunk.try_into().expect("chunk width")))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
    let chunk = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| Error::InvalidRecord(format!("missing u32 at byte offset {offset}")))?;
    Ok(u32::from_le_bytes(chunk.try_into().expect("chunk width")))
}

fn f32_ascii(values: &[f32]) -> String {
    let mut bytes = Vec::new();
    for value in values {
        let rounded = value.round();
        if !rounded.is_finite() || rounded <= 0.0 {
            break;
        }
        if rounded <= u8::MAX as f32 {
            bytes.push(rounded as u8);
        }
    }
    trim_ascii(&bytes)
}

fn bytes_ascii(bytes: &[u8], offset: usize, length: usize) -> String {
    let end = (offset + length).min(bytes.len());
    let slice = &bytes[offset..end];
    // AvaSoft 8 fixed-length text fields are NUL-terminated C strings; the
    // trailing bytes after the NUL are uninitialised memory that often
    // happens to contain ASCII-range bytes. Stop at the first NUL so callers
    // get the intended label rather than binary trailers.
    let stop = slice
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(slice.len());
    trim_ascii(&slice[..stop])
}

fn trim_ascii(bytes: &[u8]) -> String {
    let text = bytes
        .iter()
        .filter(|byte| byte.is_ascii() && !byte.is_ascii_control())
        .map(|byte| char::from(*byte))
        .collect::<String>();
    text.trim().to_string()
}
