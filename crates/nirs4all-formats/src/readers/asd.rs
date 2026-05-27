use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_formats_core::{AxisKind, Confidence, FormatProbe, Result, SignalType, SourceFile};
use serde_json::{json, Value};

use crate::readers::util::{single_signal_record, SingleSignalSpec};
use crate::Reader;

const ASD_HEADER_LEN: usize = 484;
const ASD_HEADER_OFFSET: usize = 3;
const ASD_FOOTER_MARKER: &[u8] = b"\xFF\xFE\xFD";

pub struct AsdReader;

impl Reader for AsdReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::asd"
    }

    fn sniff(&self, head: &[u8], _path: &Path) -> Option<FormatProbe> {
        sniff_version(head).map(|version| {
            FormatProbe::new(
                "asd-fieldspec",
                self.name(),
                Confidence::Definite,
                format!("ASD FieldSpec binary revision {version}"),
            )
        })
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| nirs4all_formats_core::Error::Io {
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
        let parsed = parse_asd_bytes(bytes)?;
        let record = single_signal_record(
            "asd-fieldspec",
            self.name(),
            source,
            SingleSignalSpec {
                axis_values: parsed.axis,
                axis_unit: "nm".to_string(),
                axis_kind: AxisKind::Wavelength,
                values: parsed.values,
                signal_name: parsed.signal_name,
                signal_type: parsed.signal_type,
                signal_unit: parsed.signal_unit,
                role: parsed.role,
            },
            BTreeMap::new(),
            parsed.metadata,
            parsed.warnings,
        )?;
        Ok(vec![record])
    }
}

struct ParsedAsd {
    axis: Vec<f64>,
    values: Vec<f64>,
    signal_name: String,
    signal_type: SignalType,
    signal_unit: Option<String>,
    role: String,
    metadata: BTreeMap<String, Value>,
    warnings: Vec<String>,
}

struct AsdFixedHeader {
    comments: String,
    acquisition_time: AsdLocalTime,
    program_version: String,
    file_version: String,
    dark_corrected: bool,
    dark_time_unix_seconds: i32,
    data_type: u8,
    reference_time_unix_seconds: i32,
    channel1: f64,
    wavelength_step: f64,
    data_format: u8,
    application: u8,
    channels: usize,
    integration_time_ms: u32,
    foreoptic: i16,
    dark_current_correction: i16,
    calibration_series: u16,
    instrument_number: u16,
    y_min: f64,
    y_max: f64,
    x_min: f64,
    x_max: f64,
    ip_num_bits: i16,
    x_mode: i8,
    flags1: i8,
    flags2: i8,
    flags3: i8,
    flags4: i8,
    dark_current_count: u16,
    reference_count: u16,
    sample_count: u16,
    instrument: u8,
    calibration_bulb_id: u32,
    swir1_gain: u16,
    swir2_gain: u16,
    swir1_offset: u16,
    swir2_offset: u16,
    splice1_wavelength: f64,
    splice2_wavelength: f64,
    app_data_nonzero_bytes: usize,
    smart_detector_type: Option<String>,
}

struct AsdLocalTime {
    local: String,
    daylight_savings_flag: i16,
    weekday: i16,
    day_of_year: i16,
}

impl AsdLocalTime {
    fn to_metadata(&self) -> Value {
        json!({
            "local": self.local,
            "daylight_savings_flag": self.daylight_savings_flag,
            "weekday": self.weekday,
            "day_of_year": self.day_of_year,
        })
    }
}

struct TrailingBlockScan {
    blocks: Vec<Value>,
    total_trailing_bytes: usize,
    decoded_trailing_bytes: usize,
    undecoded_trailing_bytes: usize,
    secondary_spectrum_counts: BTreeMap<String, usize>,
    warnings: Vec<String>,
}

fn parse_asd_bytes(bytes: &[u8]) -> Result<ParsedAsd> {
    let version = sniff_version(bytes).ok_or_else(|| {
        nirs4all_formats_core::Error::InvalidRecord("missing ASD file-version magic".to_string())
    })?;
    if bytes.len() < ASD_HEADER_LEN {
        return Err(nirs4all_formats_core::Error::InvalidRecord(
            "ASD file shorter than fixed header".to_string(),
        ));
    }

    let header = parse_fixed_header(bytes)?;

    if header.channels == 0 {
        return Err(nirs4all_formats_core::Error::InvalidRecord(
            "ASD header declares zero spectral channels".to_string(),
        ));
    }

    let axis = (0..header.channels)
        .map(|index| header.channel1 + header.wavelength_step * index as f64)
        .collect::<Vec<_>>();
    let value_bytes = &bytes[ASD_HEADER_LEN..];
    let values = match header.data_format {
        0 => parse_f32_values(value_bytes, header.channels)?,
        1 => parse_i32_values(value_bytes, header.channels)?,
        2 => parse_f64_values(value_bytes, header.channels)?,
        other => {
            return Err(nirs4all_formats_core::Error::InvalidRecord(format!(
                "unsupported ASD data format {other}"
            )));
        }
    };
    let consumed = match header.data_format {
        0 | 1 => ASD_HEADER_LEN + header.channels * 4,
        2 => ASD_HEADER_LEN + header.channels * 8,
        _ => ASD_HEADER_LEN,
    };
    let trailing_scan = scan_trailing_blocks(bytes, version, header.channels, consumed);
    let trailing_block_bytes = trailing_scan.total_trailing_bytes;

    let mut asd_metadata = BTreeMap::new();
    asd_metadata.insert("version".to_string(), json!(version));
    asd_metadata.insert("channels".to_string(), json!(header.channels));
    asd_metadata.insert("channel1_wavelength".to_string(), json!(header.channel1));
    asd_metadata.insert("wavelength_step".to_string(), json!(header.wavelength_step));
    asd_metadata.insert(
        "data_type".to_string(),
        json!(data_type_label(header.data_type)),
    );
    asd_metadata.insert(
        "data_format".to_string(),
        json!(data_format_label(header.data_format)),
    );
    asd_metadata.insert("program_version".to_string(), json!(header.program_version));
    asd_metadata.insert("file_version".to_string(), json!(header.file_version));
    asd_metadata.insert(
        "acquisition_time".to_string(),
        header.acquisition_time.to_metadata(),
    );
    asd_metadata.insert("dark_corrected".to_string(), json!(header.dark_corrected));
    asd_metadata.insert(
        "dark_time_unix_seconds".to_string(),
        json!(header.dark_time_unix_seconds),
    );
    asd_metadata.insert(
        "reference_time_unix_seconds".to_string(),
        json!(header.reference_time_unix_seconds),
    );
    asd_metadata.insert("application".to_string(), json!(header.application));
    asd_metadata.insert("instrument".to_string(), json!(header.instrument));
    asd_metadata.insert(
        "instrument_type".to_string(),
        json!(instrument_label(header.instrument)),
    );
    asd_metadata.insert(
        "instrument_number".to_string(),
        json!(header.instrument_number),
    );
    asd_metadata.insert(
        "integration_time_ms".to_string(),
        json!(header.integration_time_ms),
    );
    asd_metadata.insert(
        "integration_time_ms_code".to_string(),
        json!(header.integration_time_ms),
    );
    asd_metadata.insert("foreoptic".to_string(), json!(header.foreoptic));
    asd_metadata.insert(
        "dark_current_correction".to_string(),
        json!(header.dark_current_correction),
    );
    asd_metadata.insert(
        "calibration_series".to_string(),
        json!(calibration_type_label(header.calibration_series)),
    );
    asd_metadata.insert(
        "calibration_series_code".to_string(),
        json!(header.calibration_series),
    );
    asd_metadata.insert(
        "display_range".to_string(),
        json!({
            "x_min": header.x_min,
            "x_max": header.x_max,
            "y_min": header.y_min,
            "y_max": header.y_max,
        }),
    );
    asd_metadata.insert("ip_num_bits".to_string(), json!(header.ip_num_bits));
    asd_metadata.insert("x_mode".to_string(), json!(header.x_mode));
    asd_metadata.insert(
        "flags".to_string(),
        json!([header.flags1, header.flags2, header.flags3, header.flags4]),
    );
    asd_metadata.insert(
        "dark_current_count".to_string(),
        json!(header.dark_current_count),
    );
    asd_metadata.insert("reference_count".to_string(), json!(header.reference_count));
    asd_metadata.insert("sample_count".to_string(), json!(header.sample_count));
    asd_metadata.insert(
        "calibration_bulb_id".to_string(),
        json!(header.calibration_bulb_id),
    );
    asd_metadata.insert("swir1_gain".to_string(), json!(header.swir1_gain));
    asd_metadata.insert("swir2_gain".to_string(), json!(header.swir2_gain));
    asd_metadata.insert("swir1_offset".to_string(), json!(header.swir1_offset));
    asd_metadata.insert("swir2_offset".to_string(), json!(header.swir2_offset));
    asd_metadata.insert(
        "splice1_wavelength".to_string(),
        json!(header.splice1_wavelength),
    );
    asd_metadata.insert(
        "splice2_wavelength".to_string(),
        json!(header.splice2_wavelength),
    );
    asd_metadata.insert(
        "app_data_nonzero_bytes".to_string(),
        json!(header.app_data_nonzero_bytes),
    );
    asd_metadata.insert(
        "smart_detector_type".to_string(),
        json!(header.smart_detector_type),
    );
    asd_metadata.insert("comments".to_string(), json!(header.comments));
    asd_metadata.insert(
        "trailing_block_bytes".to_string(),
        json!(trailing_block_bytes),
    );
    asd_metadata.insert(
        "decoded_trailing_block_bytes".to_string(),
        json!(trailing_scan.decoded_trailing_bytes),
    );
    asd_metadata.insert(
        "undecoded_trailing_block_bytes".to_string(),
        json!(trailing_scan.undecoded_trailing_bytes),
    );
    asd_metadata.insert("secondary_blocks".to_string(), json!(trailing_scan.blocks));

    let mut metadata = BTreeMap::new();
    metadata.insert("asd".to_string(), json!(asd_metadata));

    let mut warnings = Vec::new();
    if let Some(warning) = secondary_spectrum_warning(&trailing_scan.secondary_spectrum_counts) {
        warnings.push(warning);
    }
    warnings.extend(trailing_scan.warnings);

    let signal_type = signal_type_from_data_type(header.data_type);
    let signal_name = signal_name_from_type(&signal_type).to_string();
    Ok(ParsedAsd {
        axis,
        values,
        signal_name: signal_name.clone(),
        signal_type,
        signal_unit: None,
        role: signal_name,
        metadata,
        warnings,
    })
}

fn parse_fixed_header(bytes: &[u8]) -> Result<AsdFixedHeader> {
    let offset = ASD_HEADER_OFFSET;
    let acquisition_time = parse_asd_local_time(bytes, offset + 157)?;
    let app_data = bytes.get(offset + 203..offset + 331).ok_or_else(|| {
        nirs4all_formats_core::Error::InvalidRecord("ASD app data field truncated".to_string())
    })?;
    let smart_detector = clean_ascii(&bytes[offset + 449..offset + 476]);

    Ok(AsdFixedHeader {
        comments: clean_ascii(&bytes[offset..offset + 157]),
        acquisition_time,
        program_version: packed_version(bytes[offset + 175]),
        file_version: packed_version(bytes[offset + 176]),
        dark_corrected: bytes[offset + 178] != 0,
        dark_time_unix_seconds: le_i32(bytes, offset + 179)?,
        data_type: bytes[offset + 183],
        reference_time_unix_seconds: le_i32(bytes, offset + 184)?,
        channel1: le_f32(bytes, offset + 188)? as f64,
        wavelength_step: le_f32(bytes, offset + 192)? as f64,
        data_format: bytes[offset + 196],
        application: bytes[offset + 200],
        channels: le_u16(bytes, offset + 201)? as usize,
        integration_time_ms: le_u32(bytes, offset + 387)?,
        foreoptic: le_i16(bytes, offset + 391)?,
        dark_current_correction: le_i16(bytes, offset + 393)?,
        calibration_series: le_u16(bytes, offset + 395)?,
        instrument_number: le_u16(bytes, offset + 397)?,
        y_min: le_f32(bytes, offset + 399)? as f64,
        y_max: le_f32(bytes, offset + 403)? as f64,
        x_min: le_f32(bytes, offset + 407)? as f64,
        x_max: le_f32(bytes, offset + 411)? as f64,
        ip_num_bits: le_i16(bytes, offset + 415)?,
        x_mode: bytes[offset + 417] as i8,
        flags1: bytes[offset + 418] as i8,
        flags2: bytes[offset + 419] as i8,
        flags3: bytes[offset + 420] as i8,
        flags4: bytes[offset + 421] as i8,
        dark_current_count: le_u16(bytes, offset + 422)?,
        reference_count: le_u16(bytes, offset + 424)?,
        sample_count: le_u16(bytes, offset + 426)?,
        instrument: bytes[offset + 428],
        calibration_bulb_id: le_u32(bytes, offset + 429)?,
        swir1_gain: le_u16(bytes, offset + 433)?,
        swir2_gain: le_u16(bytes, offset + 435)?,
        swir1_offset: le_u16(bytes, offset + 437)?,
        swir2_offset: le_u16(bytes, offset + 439)?,
        splice1_wavelength: le_f32(bytes, offset + 441)? as f64,
        splice2_wavelength: le_f32(bytes, offset + 445)? as f64,
        app_data_nonzero_bytes: app_data.iter().filter(|byte| **byte != 0).count(),
        smart_detector_type: (!smart_detector.is_empty()).then_some(smart_detector),
    })
}

fn parse_asd_local_time(bytes: &[u8], offset: usize) -> Result<AsdLocalTime> {
    let second = le_i16(bytes, offset)?;
    let minute = le_i16(bytes, offset + 2)?;
    let hour = le_i16(bytes, offset + 4)?;
    let day = le_i16(bytes, offset + 6)?;
    let month_zero_based = le_i16(bytes, offset + 8)?;
    let raw_year = le_i16(bytes, offset + 10)?;
    let weekday = le_i16(bytes, offset + 12)?;
    let day_of_year = le_i16(bytes, offset + 14)?;
    let daylight_savings_flag = le_i16(bytes, offset + 16)?;
    let year = if raw_year < 1900 {
        raw_year + 1900
    } else {
        raw_year
    };
    let month = month_zero_based + 1;

    Ok(AsdLocalTime {
        local: format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}"),
        daylight_savings_flag,
        weekday,
        day_of_year,
    })
}

fn packed_version(version: u8) -> String {
    format!("{}.{}", (version & 0xF0) >> 4, version & 0x0F)
}

fn scan_trailing_blocks(
    bytes: &[u8],
    version: u8,
    channels: usize,
    primary_end: usize,
) -> TrailingBlockScan {
    let mut scanner = AsdBlockScanner::new(bytes, channels, primary_end);
    if let Err(error) = scanner.scan(version) {
        scanner
            .warnings
            .push(format!("asd_trailing_block_parse_failed: {error}"));
    }
    scanner.finish()
}

struct AsdBlockScanner<'a> {
    bytes: &'a [u8],
    channels: usize,
    primary_end: usize,
    offset: usize,
    blocks: Vec<Value>,
    secondary_spectrum_counts: BTreeMap<String, usize>,
    warnings: Vec<String>,
}

impl<'a> AsdBlockScanner<'a> {
    fn new(bytes: &'a [u8], channels: usize, primary_end: usize) -> Self {
        Self {
            bytes,
            channels,
            primary_end,
            offset: primary_end.min(bytes.len()),
            blocks: Vec::new(),
            secondary_spectrum_counts: BTreeMap::new(),
            warnings: Vec::new(),
        }
    }

    fn scan(&mut self, version: u8) -> Result<()> {
        if version >= 2 {
            self.parse_reference_blocks()?;
        }
        if version >= 6 {
            self.parse_classifier_data()?;
            self.parse_dependent_variables()?;
        }
        if version >= 7 {
            self.parse_calibration_blocks()?;
        }
        if version >= 8 {
            self.parse_audit_log()?;
            self.parse_signature()?;
        }
        self.parse_footer_marker();
        self.parse_zero_padding();
        Ok(())
    }

    fn finish(self) -> TrailingBlockScan {
        let undecoded_trailing_bytes = self.bytes.len().saturating_sub(self.offset);
        let mut warnings = self.warnings;
        if undecoded_trailing_bytes > 0 {
            warnings.push(format!(
                "trailing_asd_blocks_not_decoded: {undecoded_trailing_bytes} bytes"
            ));
        }

        TrailingBlockScan {
            blocks: self.blocks,
            total_trailing_bytes: self.bytes.len().saturating_sub(self.primary_end),
            decoded_trailing_bytes: self.offset.saturating_sub(self.primary_end),
            undecoded_trailing_bytes,
            secondary_spectrum_counts: self.secondary_spectrum_counts,
            warnings,
        }
    }

    fn has_remaining(&self) -> bool {
        self.offset < self.bytes.len()
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn parse_reference_blocks(&mut self) -> Result<()> {
        if !self.has_remaining() {
            return Ok(());
        }
        let start = self.offset;
        let reference_present = self.read_bool16("ASD reference flag")?;
        let reference_time_ole = self.read_f64("ASD reference time")?;
        let spectrum_time_ole = self.read_f64("ASD reference spectrum time")?;
        let description = self.read_bstr("ASD reference description")?;
        self.blocks.push(json!({
            "kind": "reference_header",
            "offset": start,
            "byte_length": self.offset - start,
            "reference_present": reference_present,
            "reference_time_ole": reference_time_ole,
            "spectrum_time_ole": spectrum_time_ole,
            "description": empty_string_as_null(description),
        }));

        self.parse_secondary_spectrum("reference_spectrum", None)
    }

    fn parse_classifier_data(&mut self) -> Result<()> {
        if !self.has_remaining() {
            return Ok(());
        }
        let start = self.offset;
        let y_code = self.read_i8("ASD classifier y code")?;
        let y_model_type = self.read_i8("ASD classifier model type")?;
        let mut strings = BTreeMap::new();
        for field in [
            "title",
            "subtitle",
            "product_name",
            "vendor",
            "lot_number",
            "sample",
            "model_name",
            "operator",
            "date_time",
            "instrument",
            "serial_number",
            "display_mode",
            "comments",
            "units",
            "filename",
            "username",
            "reserved1",
            "reserved2",
            "reserved3",
            "reserved4",
        ] {
            let value = self.read_bstr("ASD classifier string")?;
            if !value.is_empty() {
                strings.insert(field.to_string(), json!(value));
            }
        }
        let constituent_count = self.read_u16("ASD classifier constituent count")? as usize;
        let mut constituents = Vec::new();
        if constituent_count > 0 {
            self.skip(10, "ASD classifier constituent prelude")?;
            for _ in 0..constituent_count {
                let name = self.read_bstr("ASD classifier constituent name")?;
                let pass_fail = self.read_bstr("ASD classifier constituent pass/fail")?;
                self.skip(92, "ASD classifier constituent numeric payload")?;
                constituents.push(json!({
                    "name": name,
                    "pass_fail": pass_fail,
                }));
            }
        } else {
            self.skip(2, "ASD empty classifier reserved bytes")?;
        }

        self.blocks.push(json!({
            "kind": "classifier_data",
            "offset": start,
            "byte_length": self.offset - start,
            "y_code": y_code,
            "y_model_type": y_model_type,
            "constituent_count": constituent_count,
            "strings": strings,
            "constituents": constituents,
        }));
        Ok(())
    }

    fn parse_dependent_variables(&mut self) -> Result<()> {
        if !self.has_remaining() {
            return Ok(());
        }
        let start = self.offset;
        let save_dependent_variables = self.read_bool16("ASD dependent variables flag")?;
        let count = self.read_i16("ASD dependent variable count")?;
        if count < 0 {
            return Err(invalid_record(format!(
                "ASD dependent variable count is negative: {count}"
            )));
        }

        let mut labels = Vec::new();
        let mut values = Vec::new();
        if count > 0 {
            self.skip(10, "ASD dependent variable label prelude")?;
            for _ in 0..count {
                labels.push(self.read_bstr("ASD dependent variable label")?);
            }
            self.skip(10, "ASD dependent variable value prelude")?;
            for _ in 0..count {
                values.push(self.read_f32("ASD dependent variable value")? as f64);
            }
        } else {
            self.skip(4, "ASD empty dependent variables reserved bytes")?;
        }

        self.blocks.push(json!({
            "kind": "dependent_variables",
            "offset": start,
            "byte_length": self.offset - start,
            "save_dependent_variables": save_dependent_variables,
            "count": count,
            "labels": labels,
            "values": values,
        }));
        Ok(())
    }

    fn parse_calibration_blocks(&mut self) -> Result<()> {
        if !self.has_remaining() {
            return Ok(());
        }
        let start = self.offset;
        let count = self.read_i8("ASD calibration series count")?;
        if count < 0 {
            return Err(invalid_record(format!(
                "ASD calibration series count is negative: {count}"
            )));
        }

        let mut entries = Vec::new();
        for _ in 0..count {
            let calibration_type = self.read_i8("ASD calibration type")?;
            let name = clean_ascii(self.read_bytes(20, "ASD calibration name")?);
            let integration_time_ms = self.read_i32("ASD calibration integration time")?;
            let swir1_gain = self.read_i16("ASD calibration swir1 gain")?;
            let swir2_gain = self.read_i16("ASD calibration swir2 gain")?;
            entries.push(CalibrationEntry {
                calibration_type,
                name,
                integration_time_ms,
                swir1_gain,
                swir2_gain,
            });
        }

        self.blocks.push(json!({
            "kind": "calibration_header",
            "offset": start,
            "byte_length": self.offset - start,
            "count": count,
            "series": entries.iter().map(|entry| entry.to_metadata()).collect::<Vec<_>>(),
        }));

        for entry in entries {
            self.parse_secondary_spectrum("calibration_spectrum", Some(&entry))?;
        }
        Ok(())
    }

    fn parse_audit_log(&mut self) -> Result<()> {
        if !self.has_remaining() {
            return Ok(());
        }
        let start = self.offset;
        let count = self.read_i32("ASD audit event count")?;
        if count < 0 {
            return Err(invalid_record(format!(
                "ASD audit event count is negative: {count}"
            )));
        }

        let mut events = Vec::new();
        if count > 0 {
            self.skip(10, "ASD audit log prelude")?;
            for _ in 0..count {
                let xml_len = self.read_u16("ASD audit event XML length")? as usize;
                let xml = String::from_utf8_lossy(
                    self.read_bytes(xml_len, "ASD audit event XML payload")?,
                )
                .to_string();
                events.push(audit_event_summary(&xml));
            }
        }

        self.blocks.push(json!({
            "kind": "audit_log",
            "offset": start,
            "byte_length": self.offset - start,
            "event_count": count,
            "events": events,
        }));
        Ok(())
    }

    fn parse_signature(&mut self) -> Result<()> {
        if !self.has_remaining() || self.remaining() < 151 {
            return Ok(());
        }

        let start = self.offset;
        let signed_code = self.read_i8("ASD signature state")?;
        let signature_time_ole = self.read_f64("ASD signature time")?;
        let mut strings = BTreeMap::new();
        let mut public_key_bytes = 0usize;
        for field in [
            "user_domain",
            "user_login",
            "user_name",
            "source",
            "reason",
            "notes",
            "public_key",
        ] {
            let value = self.read_bstr("ASD signature string")?;
            if field == "public_key" {
                public_key_bytes = value.len();
            } else if !value.is_empty() {
                strings.insert(field.to_string(), json!(value));
            }
        }
        let signature = self.read_bytes(128, "ASD signature bytes")?;
        let signature_nonzero_bytes = signature.iter().filter(|byte| **byte != 0).count();

        self.blocks.push(json!({
            "kind": "signature",
            "offset": start,
            "byte_length": self.offset - start,
            "signed_code": signed_code,
            "signed": signature_state_label(signed_code),
            "signature_time_ole": signature_time_ole,
            "strings": strings,
            "public_key_bytes": public_key_bytes,
            "signature_bytes": signature.len(),
            "signature_nonzero_bytes": signature_nonzero_bytes,
        }));
        Ok(())
    }

    fn parse_secondary_spectrum(
        &mut self,
        kind: &str,
        calibration: Option<&CalibrationEntry>,
    ) -> Result<()> {
        let start = self.offset;
        let byte_length = self
            .channels
            .checked_mul(8)
            .ok_or_else(|| invalid_record("ASD secondary spectrum byte length overflow"))?;
        self.skip(byte_length, "ASD secondary spectrum")?;
        *self
            .secondary_spectrum_counts
            .entry(kind.to_string())
            .or_insert(0) += 1;

        let mut block = json!({
            "kind": kind,
            "offset": start,
            "byte_length": byte_length,
            "channels": self.channels,
            "data_format": "float64",
        });
        if let Some(calibration) = calibration {
            block["calibration_type_code"] = json!(calibration.calibration_type);
            block["calibration_type"] =
                json!(calibration_type_label(calibration.calibration_type as u16));
            block["name"] = json!(calibration.name.clone());
        }
        self.blocks.push(block);
        Ok(())
    }

    fn parse_footer_marker(&mut self) {
        if self
            .bytes
            .get(self.offset..self.offset + ASD_FOOTER_MARKER.len())
            == Some(ASD_FOOTER_MARKER)
        {
            let start = self.offset;
            self.offset += ASD_FOOTER_MARKER.len();
            self.blocks.push(json!({
                "kind": "footer_marker",
                "offset": start,
                "byte_length": ASD_FOOTER_MARKER.len(),
                "marker": "ff_fefd",
            }));
        }
    }

    fn parse_zero_padding(&mut self) {
        if self.has_remaining() && self.bytes[self.offset..].iter().all(|byte| *byte == 0) {
            let start = self.offset;
            let byte_length = self.remaining();
            self.offset = self.bytes.len();
            self.blocks.push(json!({
                "kind": "zero_padding",
                "offset": start,
                "byte_length": byte_length,
            }));
        }
    }

    fn read_bytes(&mut self, len: usize, label: &str) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| invalid_record(format!("{label} offset overflow")))?;
        let data = self.bytes.get(self.offset..end).ok_or_else(|| {
            invalid_record(format!(
                "{label} truncated: need {len} bytes, got {}",
                self.remaining()
            ))
        })?;
        self.offset = end;
        Ok(data)
    }

    fn skip(&mut self, len: usize, label: &str) -> Result<()> {
        self.read_bytes(len, label).map(|_| ())
    }

    fn read_bool16(&mut self, label: &str) -> Result<bool> {
        match self.read_bytes(2, label)? {
            b"\xFF\xFF" => Ok(true),
            b"\x00\x00" => Ok(false),
            other => Err(invalid_record(format!(
                "{label} has invalid bool16 value {:02x}{:02x}",
                other[0], other[1]
            ))),
        }
    }

    fn read_i8(&mut self, label: &str) -> Result<i8> {
        Ok(self.read_bytes(1, label)?[0] as i8)
    }

    fn read_u16(&mut self, label: &str) -> Result<u16> {
        let bytes = self.read_bytes(2, label)?;
        Ok(u16::from_le_bytes(
            bytes.try_into().expect("slice length checked"),
        ))
    }

    fn read_i16(&mut self, label: &str) -> Result<i16> {
        let bytes = self.read_bytes(2, label)?;
        Ok(i16::from_le_bytes(
            bytes.try_into().expect("slice length checked"),
        ))
    }

    fn read_i32(&mut self, label: &str) -> Result<i32> {
        let bytes = self.read_bytes(4, label)?;
        Ok(i32::from_le_bytes(
            bytes.try_into().expect("slice length checked"),
        ))
    }

    fn read_f32(&mut self, label: &str) -> Result<f32> {
        let bytes = self.read_bytes(4, label)?;
        Ok(f32::from_le_bytes(
            bytes.try_into().expect("slice length checked"),
        ))
    }

    fn read_f64(&mut self, label: &str) -> Result<f64> {
        let bytes = self.read_bytes(8, label)?;
        Ok(f64::from_le_bytes(
            bytes.try_into().expect("slice length checked"),
        ))
    }

    fn read_bstr(&mut self, label: &str) -> Result<String> {
        let len = self.read_i16(label)?;
        if len < 0 {
            return Ok(String::new());
        }
        let bytes = self.read_bytes(len as usize, label)?;
        Ok(String::from_utf8_lossy(bytes)
            .trim_matches(char::from(0))
            .to_string())
    }
}

struct CalibrationEntry {
    calibration_type: i8,
    name: String,
    integration_time_ms: i32,
    swir1_gain: i16,
    swir2_gain: i16,
}

impl CalibrationEntry {
    fn to_metadata(&self) -> Value {
        json!({
            "calibration_type_code": self.calibration_type,
            "calibration_type": calibration_type_label(self.calibration_type as u16),
            "name": empty_string_as_null(self.name.clone()),
            "integration_time_ms": self.integration_time_ms,
            "swir1_gain": self.swir1_gain,
            "swir2_gain": self.swir2_gain,
        })
    }
}

fn invalid_record(message: impl Into<String>) -> nirs4all_formats_core::Error {
    nirs4all_formats_core::Error::InvalidRecord(message.into())
}

fn empty_string_as_null(value: String) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        json!(value)
    }
}

fn audit_event_summary(xml: &str) -> Value {
    let mut summary = BTreeMap::new();
    for (key, tag) in [
        ("application", "Audit_Application"),
        ("app_version", "Audit_AppVersion"),
        ("name", "Audit_Name"),
        ("login", "Audit_Login"),
        ("time", "Audit_Time"),
        ("source", "Audit_Source"),
        ("function", "Audit_Function"),
        ("notes", "Audit_Notes"),
    ] {
        if let Some(value) = xml_tag_text(xml, tag) {
            summary.insert(key.to_string(), json!(value));
        }
    }
    json!(summary)
}

fn xml_tag_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].to_string())
}

fn secondary_spectrum_warning(counts: &BTreeMap<String, usize>) -> Option<String> {
    let reference_count = counts.get("reference_spectrum").copied().unwrap_or(0);
    let calibration_count = counts.get("calibration_spectrum").copied().unwrap_or(0);
    if reference_count == 0 && calibration_count == 0 {
        return None;
    }

    let mut parts = Vec::new();
    if reference_count > 0 {
        parts.push(format!("reference_spectrum={reference_count}"));
    }
    if calibration_count > 0 {
        parts.push(format!("calibration_spectrum={calibration_count}"));
    }
    Some(format!(
        "asd_secondary_spectra_not_emitted: {}",
        parts.join(", ")
    ))
}

fn sniff_version(bytes: &[u8]) -> Option<u8> {
    match bytes.get(..3)? {
        b"ASD" => Some(1),
        b"as2" => Some(2),
        b"as3" => Some(3),
        b"as4" => Some(4),
        b"as5" => Some(5),
        b"as6" => Some(6),
        b"as7" => Some(7),
        b"as8" => Some(8),
        _ => None,
    }
}

fn parse_f32_values(bytes: &[u8], count: usize) -> Result<Vec<f64>> {
    require_len(bytes, count * 4, "ASD float32 spectrum")?;
    (0..count)
        .map(|index| le_f32(bytes, index * 4).map(|value| value as f64))
        .collect()
}

fn parse_i32_values(bytes: &[u8], count: usize) -> Result<Vec<f64>> {
    require_len(bytes, count * 4, "ASD int32 spectrum")?;
    (0..count)
        .map(|index| le_i32(bytes, index * 4).map(|value| value as f64))
        .collect()
}

fn parse_f64_values(bytes: &[u8], count: usize) -> Result<Vec<f64>> {
    require_len(bytes, count * 8, "ASD float64 spectrum")?;
    (0..count).map(|index| le_f64(bytes, index * 8)).collect()
}

fn require_len(bytes: &[u8], min_len: usize, label: &str) -> Result<()> {
    if bytes.len() < min_len {
        return Err(nirs4all_formats_core::Error::InvalidRecord(format!(
            "{label} truncated: need {min_len} bytes, got {}",
            bytes.len()
        )));
    }
    Ok(())
}

fn le_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let data = bytes.get(offset..offset + 2).ok_or_else(|| {
        nirs4all_formats_core::Error::InvalidRecord("ASD u16 field truncated".to_string())
    })?;
    Ok(u16::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn le_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let data = bytes.get(offset..offset + 4).ok_or_else(|| {
        nirs4all_formats_core::Error::InvalidRecord("ASD u32 field truncated".to_string())
    })?;
    Ok(u32::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn le_i16(bytes: &[u8], offset: usize) -> Result<i16> {
    let data = bytes.get(offset..offset + 2).ok_or_else(|| {
        nirs4all_formats_core::Error::InvalidRecord("ASD i16 field truncated".to_string())
    })?;
    Ok(i16::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn le_i32(bytes: &[u8], offset: usize) -> Result<i32> {
    let data = bytes.get(offset..offset + 4).ok_or_else(|| {
        nirs4all_formats_core::Error::InvalidRecord("ASD i32 field truncated".to_string())
    })?;
    Ok(i32::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn le_f32(bytes: &[u8], offset: usize) -> Result<f32> {
    let data = bytes.get(offset..offset + 4).ok_or_else(|| {
        nirs4all_formats_core::Error::InvalidRecord("ASD f32 field truncated".to_string())
    })?;
    Ok(f32::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn le_f64(bytes: &[u8], offset: usize) -> Result<f64> {
    let data = bytes.get(offset..offset + 8).ok_or_else(|| {
        nirs4all_formats_core::Error::InvalidRecord("ASD f64 field truncated".to_string())
    })?;
    Ok(f64::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn clean_ascii(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_matches(char::from(0))
        .trim()
        .to_string()
}

fn signal_type_from_data_type(data_type: u8) -> SignalType {
    match data_type {
        0 => SignalType::RawCounts,
        1 | 8 => SignalType::Reflectance,
        2 => SignalType::Radiance,
        4 => SignalType::Irradiance,
        6 => SignalType::Transmittance,
        _ => SignalType::Unknown,
    }
}

fn signal_name_from_type(signal_type: &SignalType) -> &'static str {
    match signal_type {
        SignalType::RawCounts => "raw",
        SignalType::Reflectance => "reflectance",
        SignalType::Radiance => "radiance",
        SignalType::Irradiance => "irradiance",
        SignalType::Transmittance => "transmittance",
        _ => "signal",
    }
}

fn data_type_label(data_type: u8) -> &'static str {
    match data_type {
        0 => "raw",
        1 => "reflectance",
        2 => "radiance",
        3 => "no_units",
        4 => "irradiance",
        5 => "quality_index",
        6 => "transmittance",
        8 => "absolute_reflectance",
        _ => "unknown",
    }
}

fn data_format_label(data_format: u8) -> &'static str {
    match data_format {
        0 => "float32",
        1 => "int32",
        2 => "float64",
        _ => "unknown",
    }
}

fn instrument_label(instrument: u8) -> &'static str {
    match instrument {
        0 => "unknown",
        1 => "psii",
        2 => "lsvnir",
        3 => "fsvnir",
        4 => "fieldspec_full_range",
        5 => "fsnir",
        6 => "chem",
        7 => "labspec_pro",
        10 => "handheld",
        _ => "unknown",
    }
}

fn calibration_type_label(calibration_type: u16) -> &'static str {
    match calibration_type {
        0 => "absolute",
        1 => "base",
        2 => "lamp",
        3 => "fiber_optic",
        4 => "unknown",
        _ => "unknown",
    }
}

fn signature_state_label(signed_code: i8) -> &'static str {
    match signed_code {
        0 => "unsigned",
        1 => "signed",
        _ => "invalid",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_all_committed_asd_revisions() {
        assert_eq!(sniff_version(b"ASD\0"), Some(1));
        assert_eq!(sniff_version(b"as6\0"), Some(6));
        assert_eq!(sniff_version(b"as7\0"), Some(7));
        assert_eq!(sniff_version(b"as8\0"), Some(8));
    }
}
