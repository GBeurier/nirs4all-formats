use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralArray,
    SpectralAxis, SpectralRecord,
};
use serde_json::json;

use crate::readers::util::{record_from_signals, single_signal_record, SingleSignalSpec};
use crate::Reader;

const OLE_MAGIC: &[u8] = b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1";

pub struct JascoJwsReader;

impl Reader for JascoJwsReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::jasco_jws"
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
        let source = SourceFile::from_path(path, "primary")?;
        let mut comp = cfb::open(path)
            .map_err(|error| Error::InvalidRecord(format!("JASCO JWS open error: {error}")))?;
        let data_info = read_stream(&mut comp, "DataInfo")?;
        let y_data = read_stream(&mut comp, "Y-Data")?;
        let base_info = read_stream(&mut comp, "BaseInfo").ok();
        parse_jws_streams(
            &data_info,
            &y_data,
            base_info.as_deref(),
            source,
            self.name(),
        )
    }
}

struct JwsDataInfo {
    channel_count: usize,
    point_count: usize,
    first_x: f64,
    last_x: f64,
}

fn parse_jws_streams(
    data_info: &[u8],
    y_data: &[u8],
    base_info: Option<&[u8]>,
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
    let mut metadata = BTreeMap::new();
    metadata.insert("channel_count".to_string(), json!(info.channel_count));
    metadata.insert("point_count".to_string(), json!(info.point_count));
    if let Some(path) = base_info.and_then(extract_base_path) {
        metadata.insert("source_path".to_string(), json!(path));
    }

    if info.channel_count == 1 {
        let signal_type = SignalType::Unknown;
        let record = single_signal_record(
            "jasco-jws",
            reader,
            source,
            SingleSignalSpec {
                axis_values: axis,
                axis_unit,
                axis_kind,
                values,
                signal_name: "signal".to_string(),
                signal_type,
                signal_unit: None,
                role: "signal".to_string(),
            },
            BTreeMap::new(),
            metadata,
            vec!["jasco_jws_reverse_engineered_data_info".to_string()],
        )?;
        return Ok(vec![record]);
    }

    let mut signals = BTreeMap::new();
    for channel in 0..info.channel_count {
        let start = channel * info.point_count;
        let end = start + info.point_count;
        let channel_values = values[start..end].to_vec();
        let spectral_axis = SpectralAxis::new(axis.clone(), axis_unit.clone(), axis_kind.clone())?;
        let signal_name = format!("channel_{}", channel + 1);
        let signal = SpectralArray::new(
            spectral_axis,
            channel_values,
            vec!["x".to_string()],
            SignalType::Unknown,
            None,
            signal_name.clone(),
            "file",
        )?;
        signals.insert(signal_name, signal);
    }
    let record = record_from_signals(
        "jasco-jws",
        reader,
        source,
        signals,
        SignalType::Unknown,
        metadata,
        vec!["jasco_jws_reverse_engineered_data_info".to_string()],
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
    let path_len = read_u32(bytes, 20).ok()? as usize;
    let start = 24;
    let end = start + path_len;
    if end > bytes.len() {
        return None;
    }
    let utf16 = bytes[start..end]
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .take_while(|value| *value != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&utf16)
        .ok()
        .filter(|value| !value.trim().is_empty())
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
