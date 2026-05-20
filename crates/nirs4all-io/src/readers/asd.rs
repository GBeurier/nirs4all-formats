use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{AxisKind, Confidence, FormatProbe, Result, SignalType, SourceFile};
use serde_json::{json, Value};

use crate::readers::util::{single_signal_record, SingleSignalSpec};
use crate::Reader;

const ASD_HEADER_LEN: usize = 484;

pub struct AsdReader;

impl Reader for AsdReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::asd"
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

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| nirs4all_io_core::Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let source = SourceFile::from_bytes(path, &bytes, "primary");
        let parsed = parse_asd_bytes(&bytes)?;
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

fn parse_asd_bytes(bytes: &[u8]) -> Result<ParsedAsd> {
    let version = sniff_version(bytes).ok_or_else(|| {
        nirs4all_io_core::Error::InvalidRecord("missing ASD file-version magic".to_string())
    })?;
    if bytes.len() < ASD_HEADER_LEN {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "ASD file shorter than fixed header".to_string(),
        ));
    }

    let header_offset = 3;
    let data_type = bytes[header_offset + 183];
    let channel1 = le_f32(bytes, header_offset + 188)? as f64;
    let wavelength_step = le_f32(bytes, header_offset + 192)? as f64;
    let data_format = bytes[header_offset + 196];
    let channels = le_u16(bytes, header_offset + 201)? as usize;
    let instrument = bytes[header_offset + 428];
    let integration_time_ms = le_u32(bytes, header_offset + 387)?;
    let comments = clean_ascii(&bytes[header_offset..header_offset + 157]);

    if channels == 0 {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "ASD header declares zero spectral channels".to_string(),
        ));
    }

    let axis = (0..channels)
        .map(|index| channel1 + wavelength_step * index as f64)
        .collect::<Vec<_>>();
    let value_bytes = &bytes[ASD_HEADER_LEN..];
    let values = match data_format {
        0 => parse_f32_values(value_bytes, channels)?,
        1 => parse_i32_values(value_bytes, channels)?,
        2 => parse_f64_values(value_bytes, channels)?,
        other => {
            return Err(nirs4all_io_core::Error::InvalidRecord(format!(
                "unsupported ASD data format {other}"
            )));
        }
    };
    let consumed = match data_format {
        0 | 1 => ASD_HEADER_LEN + channels * 4,
        2 => ASD_HEADER_LEN + channels * 8,
        _ => ASD_HEADER_LEN,
    };
    let trailing_block_bytes = bytes.len().saturating_sub(consumed);

    let mut metadata = BTreeMap::new();
    metadata.insert(
        "asd".to_string(),
        json!({
            "version": version,
            "channels": channels,
            "channel1_wavelength": channel1,
            "wavelength_step": wavelength_step,
            "data_type": data_type_label(data_type),
            "data_format": data_format_label(data_format),
            "instrument": instrument,
            "integration_time_ms_code": integration_time_ms,
            "comments": comments,
            "trailing_block_bytes": trailing_block_bytes,
        }),
    );

    let mut warnings = Vec::new();
    if trailing_block_bytes > 0 {
        warnings.push(format!(
            "trailing_asd_blocks_not_decoded: {trailing_block_bytes} bytes"
        ));
    }

    let signal_type = signal_type_from_data_type(data_type);
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
        return Err(nirs4all_io_core::Error::InvalidRecord(format!(
            "{label} truncated: need {min_len} bytes, got {}",
            bytes.len()
        )));
    }
    Ok(())
}

fn le_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let data = bytes.get(offset..offset + 2).ok_or_else(|| {
        nirs4all_io_core::Error::InvalidRecord("ASD u16 field truncated".to_string())
    })?;
    Ok(u16::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn le_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let data = bytes.get(offset..offset + 4).ok_or_else(|| {
        nirs4all_io_core::Error::InvalidRecord("ASD u32 field truncated".to_string())
    })?;
    Ok(u32::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn le_i32(bytes: &[u8], offset: usize) -> Result<i32> {
    let data = bytes.get(offset..offset + 4).ok_or_else(|| {
        nirs4all_io_core::Error::InvalidRecord("ASD i32 field truncated".to_string())
    })?;
    Ok(i32::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn le_f32(bytes: &[u8], offset: usize) -> Result<f32> {
    let data = bytes.get(offset..offset + 4).ok_or_else(|| {
        nirs4all_io_core::Error::InvalidRecord("ASD f32 field truncated".to_string())
    })?;
    Ok(f32::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn le_f64(bytes: &[u8], offset: usize) -> Result<f64> {
    let data = bytes.get(offset..offset + 8).ok_or_else(|| {
        nirs4all_io_core::Error::InvalidRecord("ASD f64 field truncated".to_string())
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
