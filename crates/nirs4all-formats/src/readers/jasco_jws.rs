use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::Path;

use nirs4all_formats_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralArray,
    SpectralAxis, SpectralRecord,
};
use serde_json::json;

use crate::readers::util::{
    read_bytes as read_path_bytes, record_from_signals, single_signal_record, SingleSignalSpec,
};
use crate::Reader;

const OLE_MAGIC: &[u8] = b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1";

pub struct JascoJwsReader;

impl Reader for JascoJwsReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::jasco_jws"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext == "jws" && head.starts_with(OLE_MAGIC) {
            Some(FormatProbe::new(
                "jasco-jws",
                self.name(),
                Confidence::Likely,
                "JASCO JWS OLE2 compound document header",
            ))
        } else {
            None
        }
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let bytes = read_path_bytes(path)?;
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(&self, path: &Path, bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
        let source = SourceFile::from_bytes(path, bytes, "primary");
        let mut comp = cfb::CompoundFile::open(Cursor::new(bytes.to_vec()))
            .map_err(|error| Error::InvalidRecord(format!("JASCO JWS open error: {error}")))?;
        let data_info = read_stream(&mut comp, "DataInfo")?;
        let y_data = read_stream(&mut comp, "Y-Data")?;
        let base_info = read_stream(&mut comp, "BaseInfo").ok();
        let hints = JwsTextHints {
            source_path: base_info.as_deref().and_then(extract_base_path),
            module_strings: read_stream(&mut comp, "ModuleInfo")
                .ok()
                .map(|stream| extract_utf16le_strings(&stream))
                .unwrap_or_default(),
            sample_strings: read_stream(&mut comp, "SampleInfo")
                .ok()
                .map(|stream| extract_utf16le_strings(&stream))
                .unwrap_or_default(),
            user_strings: read_stream(&mut comp, "UserInfo")
                .ok()
                .map(|stream| extract_utf16le_strings(&stream))
                .unwrap_or_default(),
            measurement_parameters: read_stream(&mut comp, "MeasParam")
                .ok()
                .map(|stream| extract_utf16le_strings(&stream))
                .unwrap_or_default(),
        };
        parse_jws_streams(&data_info, &y_data, hints, source, self.name())
    }
}

struct JwsDataInfo {
    channel_count: usize,
    point_count: usize,
    first_x: f64,
    last_x: f64,
}

#[derive(Default)]
struct JwsTextHints {
    source_path: Option<String>,
    module_strings: Vec<String>,
    sample_strings: Vec<String>,
    user_strings: Vec<String>,
    measurement_parameters: Vec<String>,
}

#[derive(Clone)]
struct JwsChannelSpec {
    name: String,
    signal_type: SignalType,
    unit: Option<String>,
}

fn parse_jws_streams(
    data_info: &[u8],
    y_data: &[u8],
    hints: JwsTextHints,
    source: SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    let info = parse_data_info(data_info)?;
    let values = read_f32_values(y_data)?;
    let expected = info.channel_count * info.point_count;
    if values.len() != expected {
        return Err(Error::InvalidRecord(format!(
            "JASCO JWS Y-Data has {} float32 values, expected {expected}",
            values.len()
        )));
    }
    let axis_unit = axis_unit(info.first_x, info.last_x);
    let axis_kind = axis_kind(&axis_unit);
    let axis = linspace(info.first_x, info.last_x, info.point_count);
    let (channel_specs, measurement_mode) = infer_channel_specs(&info, &axis_unit, &values, &hints);
    let mut metadata = BTreeMap::new();
    metadata.insert("channel_count".to_string(), json!(info.channel_count));
    metadata.insert(
        "channel_labels".to_string(),
        json!(channel_specs
            .iter()
            .map(|spec| spec.name.as_str())
            .collect::<Vec<_>>()),
    );
    metadata.insert("point_count".to_string(), json!(info.point_count));
    if let Some(path) = hints.source_path.as_ref() {
        metadata.insert("source_path".to_string(), json!(path));
    }
    if let Some(model) = hints.module_strings.first() {
        metadata.insert("instrument_model".to_string(), json!(model));
    }
    if !hints.module_strings.is_empty() {
        metadata.insert(
            "instrument_modules".to_string(),
            json!(hints.module_strings),
        );
    }
    if let Some(sample) = hints.sample_strings.first() {
        metadata.insert("sample_label".to_string(), json!(sample));
    }
    if let Some(user) = hints.user_strings.first() {
        metadata.insert("operator_organization".to_string(), json!(user));
    }
    if !hints.measurement_parameters.is_empty() {
        metadata.insert(
            "measurement_parameters".to_string(),
            json!(hints.measurement_parameters),
        );
    }
    if let Some(mode) = measurement_mode {
        metadata.insert("measurement_mode".to_string(), json!(mode));
    }

    let mut warnings = vec!["jasco_jws_reverse_engineered_data_info".to_string()];
    if measurement_mode.is_some() {
        warnings.push("jasco_jws_semantic_channels_inferred".to_string());
    }

    if info.channel_count == 1 {
        let spec = channel_specs
            .first()
            .expect("single-channel JASCO spec")
            .clone();
        let record = single_signal_record(
            "jasco-jws",
            reader,
            source,
            SingleSignalSpec {
                axis_values: axis,
                axis_unit,
                axis_kind,
                values,
                signal_name: spec.name.clone(),
                signal_type: spec.signal_type.clone(),
                signal_unit: spec.unit,
                role: spec.name,
            },
            BTreeMap::new(),
            metadata,
            warnings,
        )?;
        return Ok(vec![record]);
    }

    let mut signals = BTreeMap::new();
    for channel in 0..info.channel_count {
        let spec = channel_specs
            .get(channel)
            .expect("multi-channel JASCO spec");
        let start = channel * info.point_count;
        let end = start + info.point_count;
        let channel_values = values[start..end].to_vec();
        let spectral_axis = SpectralAxis::new(axis.clone(), axis_unit.clone(), axis_kind.clone())?;
        let signal = SpectralArray::new(
            spectral_axis,
            channel_values,
            vec!["x".to_string()],
            spec.signal_type.clone(),
            spec.unit.clone(),
            spec.name.clone(),
            "file",
        )?;
        signals.insert(spec.name.clone(), signal);
    }
    let record = record_from_signals(
        "jasco-jws",
        reader,
        source,
        signals,
        dominant_signal_type(&channel_specs),
        metadata,
        warnings,
    )?;
    Ok(vec![record])
}

fn read_stream<F: Read + std::io::Seek>(
    comp: &mut cfb::CompoundFile<F>,
    path: &str,
) -> Result<Vec<u8>> {
    let mut stream = comp
        .open_stream(path)
        .map_err(|error| Error::InvalidRecord(format!("JASCO JWS stream {path} error: {error}")))?;
    let mut out = Vec::new();
    stream.read_to_end(&mut out).map_err(|error| {
        Error::InvalidRecord(format!("JASCO JWS stream {path} read error: {error}"))
    })?;
    Ok(out)
}

fn parse_data_info(bytes: &[u8]) -> Result<JwsDataInfo> {
    if bytes.len() < 48 {
        return Err(Error::InvalidRecord(
            "JASCO JWS DataInfo stream is too short".to_string(),
        ));
    }
    let channel_count = read_u32(bytes, 12)? as usize;
    let point_count = read_u32(bytes, 20)? as usize;
    if channel_count == 0 || point_count == 0 {
        return Err(Error::InvalidRecord(
            "JASCO JWS DataInfo has zero channels or points".to_string(),
        ));
    }
    Ok(JwsDataInfo {
        channel_count,
        point_count,
        first_x: read_f64(bytes, 24)?,
        last_x: read_f64(bytes, 32)?,
    })
}

fn read_f32_values(bytes: &[u8]) -> Result<Vec<f64>> {
    if !bytes.len().is_multiple_of(4) {
        return Err(Error::InvalidRecord(
            "JASCO JWS Y-Data length is not a multiple of float32".to_string(),
        ));
    }
    (0..bytes.len() / 4)
        .map(|index| {
            let start = index * 4;
            Ok(f32::from_le_bytes(bytes[start..start + 4].try_into().expect("slice len")) as f64)
        })
        .collect()
}

fn axis_unit(first: f64, last: f64) -> String {
    let min = first.min(last);
    let max = first.max(last);
    if min >= 150.0 && max <= 2500.0 {
        "nm".to_string()
    } else {
        "cm-1".to_string()
    }
}

fn axis_kind(unit: &str) -> AxisKind {
    if unit == "nm" {
        AxisKind::Wavelength
    } else {
        AxisKind::Wavenumber
    }
}

fn extract_base_path(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 24 {
        return None;
    }
    let path_len = u16::from_be_bytes([bytes[20], bytes[21]]) as usize;
    let start = 24;
    let end = start + path_len;
    if end > bytes.len() {
        return None;
    }
    let utf16 = bytes[start..end]
        .chunks_exact(2)
        .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
        .take_while(|value| *value != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&utf16)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn extract_utf16le_strings(bytes: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = Vec::new();
    for chunk in bytes.chunks_exact(2) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        if (0x20..=0x7e).contains(&value) {
            current.push(value);
        } else {
            push_utf16_string(&mut out, &mut current);
        }
    }
    push_utf16_string(&mut out, &mut current);
    out
}

fn push_utf16_string(out: &mut Vec<String>, current: &mut Vec<u16>) {
    if current.len() >= 2 {
        if let Ok(value) = String::from_utf16(current) {
            let trimmed = value.trim();
            if !trimmed.is_empty() && !out.iter().any(|existing| existing == trimmed) {
                out.push(trimmed.to_string());
            }
        }
    }
    current.clear();
}

fn infer_channel_specs(
    info: &JwsDataInfo,
    axis_unit: &str,
    values: &[f64],
    hints: &JwsTextHints,
) -> (Vec<JwsChannelSpec>, Option<&'static str>) {
    let haystack = hints.haystack();
    if info.channel_count == 3
        && (haystack.contains("cd-1500")
            || haystack.contains("j-1500")
            || haystack.contains("cd1500")
            || (haystack.contains("mdeg") && haystack.contains("dod")))
    {
        return (
            vec![
                JwsChannelSpec::new("cd", SignalType::Unknown, Some("mdeg")),
                JwsChannelSpec::new("ht", SignalType::Unknown, Some("V")),
                JwsChannelSpec::new("absorbance", SignalType::Absorbance, Some("dOD")),
            ],
            Some("circular_dichroism"),
        );
    }

    if info.channel_count == 1 {
        if haystack.contains("fp-") || haystack.contains("fluorescence") {
            return (
                vec![JwsChannelSpec::new(
                    "fluorescence",
                    SignalType::Unknown,
                    None,
                )],
                Some("fluorescence"),
            );
        }
        if axis_unit == "cm-1"
            && haystack.contains("ft/ir")
            && values.iter().all(|value| (-5.0..=125.0).contains(value))
        {
            return (
                vec![JwsChannelSpec::new(
                    "transmittance",
                    SignalType::Transmittance,
                    Some("%T"),
                )],
                Some("ftir_transmittance"),
            );
        }
    }

    (fallback_channel_specs(info.channel_count), None)
}

impl JwsTextHints {
    fn haystack(&self) -> String {
        let mut values = Vec::new();
        if let Some(path) = self.source_path.as_ref() {
            values.push(path.as_str());
        }
        values.extend(self.module_strings.iter().map(String::as_str));
        values.extend(self.sample_strings.iter().map(String::as_str));
        values.extend(self.user_strings.iter().map(String::as_str));
        values.extend(self.measurement_parameters.iter().map(String::as_str));
        values.join("\n").to_ascii_lowercase()
    }
}

impl JwsChannelSpec {
    fn new(name: &str, signal_type: SignalType, unit: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            signal_type,
            unit: unit.map(str::to_string),
        }
    }
}

fn fallback_channel_specs(channel_count: usize) -> Vec<JwsChannelSpec> {
    if channel_count == 1 {
        return vec![JwsChannelSpec::new("signal", SignalType::Unknown, None)];
    }
    (0..channel_count)
        .map(|channel| {
            JwsChannelSpec::new(
                &format!("channel_{}", channel + 1),
                SignalType::Unknown,
                None,
            )
        })
        .collect()
}

fn dominant_signal_type(specs: &[JwsChannelSpec]) -> SignalType {
    let Some(first) = specs.first() else {
        return SignalType::Unknown;
    };
    if specs
        .iter()
        .all(|spec| spec.signal_type == first.signal_type)
    {
        first.signal_type.clone()
    } else {
        SignalType::Unknown
    }
}

fn linspace(first: f64, last: f64, len: usize) -> Vec<f64> {
    if len <= 1 {
        return vec![first];
    }
    let step = (last - first) / (len - 1) as f64;
    (0..len).map(|index| first + step * index as f64).collect()
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| Error::InvalidRecord("truncated JASCO JWS u32 field".to_string()))?;
    Ok(u32::from_le_bytes(value.try_into().expect("slice len")))
}

fn read_f64(bytes: &[u8], offset: usize) -> Result<f64> {
    let value = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| Error::InvalidRecord("truncated JASCO JWS f64 field".to_string()))?;
    Ok(f64::from_le_bytes(value.try_into().expect("slice len")))
}
