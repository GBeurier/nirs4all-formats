use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_formats_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralArray,
    SpectralAxis, SpectralRecord,
};
use serde_json::{json, Value};

use crate::readers::util::{provenance, safe_signal_name};
use crate::Reader;

/// Leading length byte (4) followed by the ASCII tag `MNIR`. The whole file is a
/// .NET `BinaryWriter` stream, so every string is 7-bit-length-prefixed.
const MAGIC_LEN: u8 = 4;
const MAGIC_TAG: &[u8] = b"MNIR";

/// .NET ticks (100 ns units) elapsed between 0001-01-01 and the Unix epoch,
/// used to turn the stored `DateTime` into an ISO timestamp.
const TICKS_AT_UNIX_EPOCH: i64 = 621_355_968_000_000_000;
/// Mask that drops the two high `DateTimeKind` bits from a serialized tick value.
const TICKS_MASK: i64 = (1 << 62) - 1;

pub struct ViaviMicroNirReader;

impl Reader for ViaviMicroNirReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::viavi_micronir"
    }

    fn sniff(&self, head: &[u8], _path: &Path) -> Option<FormatProbe> {
        if head.len() >= 5 && head[0] == MAGIC_LEN && &head[1..5] == MAGIC_TAG {
            Some(FormatProbe::new(
                "viavi-micronir-sam",
                self.name(),
                Confidence::Definite,
                "Viavi/JDSU MicroNIR .sam (.NET BinaryWriter MNIR container)",
            ))
        } else {
            None
        }
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(&self, path: &Path, bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
        let source = SourceFile::from_bytes(path, bytes, "primary");
        parse_micronir(bytes, source, self.name())
    }
}

struct Header {
    version: String,
    format_field: i32,
    channel_count: usize,
    raw_pixel_count: usize,
    reserved_field: i32,
    model: i32,
    sample_name: String,
    instrument_serial: String,
    scan_count: f64,
    wavelength_first: f64,
    wavelength_last: f64,
    timestamp: Option<String>,
    integration_time_ms: f32,
    operator: String,
    data_offset: usize,
}

fn parse_micronir(bytes: &[u8], source: SourceFile, reader: &str) -> Result<Vec<SpectralRecord>> {
    if !(bytes.len() >= 5 && bytes[0] == MAGIC_LEN && &bytes[1..5] == MAGIC_TAG) {
        return Err(Error::InvalidRecord(
            "missing Viavi MicroNIR MNIR header".to_string(),
        ));
    }
    let header = parse_header(bytes)?;

    let total_points = header
        .channel_count
        .checked_add(header.raw_pixel_count)
        .ok_or_else(|| Error::InvalidRecord("MicroNIR channel/raw counts overflow".to_string()))?;
    let needed = total_points
        .checked_mul(8)
        .and_then(|len| header.data_offset.checked_add(len));
    match needed {
        Some(end) if end <= bytes.len() => {}
        _ => {
            return Err(Error::InvalidRecord(format!(
                "MicroNIR data section needs {total_points} doubles from offset {} but file is {} bytes",
                header.data_offset,
                bytes.len()
            )))
        }
    }

    let absorbance = read_f64_slice(bytes, header.data_offset, header.channel_count)?;
    let raw_offset = header.data_offset + header.channel_count * 8;
    let raw_counts = read_f64_slice(bytes, raw_offset, header.raw_pixel_count)?;

    let axis_values = derived_axis(
        header.wavelength_first,
        header.wavelength_last,
        header.channel_count,
    );

    let mut signals = BTreeMap::new();
    let absorbance_axis = SpectralAxis::new(axis_values, "nm", AxisKind::Wavelength)?;
    let absorbance_signal = SpectralArray::new(
        absorbance_axis,
        absorbance,
        vec!["x".to_string()],
        SignalType::Absorbance,
        None,
        "absorbance",
        "file",
    )?;
    signals.insert("absorbance".to_string(), absorbance_signal);

    // The detector reports 128 raw single-beam pixels; only the first
    // `channel_count` map onto the optical wavelength grid, so the raw array
    // keeps its own pixel-index axis instead of the wavelength axis.
    let raw_axis = SpectralAxis::new(
        (0..header.raw_pixel_count)
            .map(|index| index as f64)
            .collect(),
        "pixel",
        AxisKind::Index,
    )?;
    let raw_signal = SpectralArray::new(
        raw_axis,
        raw_counts,
        vec!["x".to_string()],
        SignalType::SingleBeam,
        None,
        "raw_single_beam",
        "file",
    )?;
    signals.insert("raw_single_beam".to_string(), raw_signal);

    let metadata = build_metadata(&header);
    let warnings = vec!["viavi_micronir_reverse_engineered_header".to_string()];

    let record = SpectralRecord {
        signals,
        signal_type: SignalType::Absorbance,
        targets: BTreeMap::new(),
        metadata,
        provenance: provenance("viavi-micronir-sam", reader, source, warnings),
        quality_flags: Vec::new(),
    };
    record.validate()?;
    Ok(vec![record])
}

fn parse_header(bytes: &[u8]) -> Result<Header> {
    let mut cursor = 0usize;
    // magic string ("MNIR") then version string ("3.1.0.0")
    let _magic = read_dotnet_string(bytes, &mut cursor)?;
    let version = read_dotnet_string(bytes, &mut cursor)?;

    let format_field = read_i32(bytes, &mut cursor)?;
    let channel_count = read_i32(bytes, &mut cursor)?;
    let raw_pixel_count = read_i32(bytes, &mut cursor)?;
    let reserved_field = read_i32(bytes, &mut cursor)?;
    if channel_count <= 0 || raw_pixel_count <= 0 {
        return Err(Error::InvalidRecord(format!(
            "MicroNIR header has non-positive counts (channels={channel_count}, raw={raw_pixel_count})"
        )));
    }
    let model = read_i32(bytes, &mut cursor)?;

    let sample_name = read_dotnet_string(bytes, &mut cursor)?;
    let _empty = read_dotnet_string(bytes, &mut cursor)?;
    let instrument_serial = read_dotnet_string(bytes, &mut cursor)?;

    let scan_count = read_f64(bytes, &mut cursor)?;
    let wavelength_first = read_f64(bytes, &mut cursor)?;
    let wavelength_last = read_f64(bytes, &mut cursor)?;
    if !wavelength_first.is_finite() || !wavelength_last.is_finite() {
        return Err(Error::InvalidRecord(
            "MicroNIR wavelength endpoints are not finite".to_string(),
        ));
    }

    let raw_ticks = read_i64(bytes, &mut cursor)?;
    let timestamp = dotnet_ticks_to_iso(raw_ticks);
    let integration_time_ms = read_f32(bytes, &mut cursor)?;
    let operator = read_dotnet_string(bytes, &mut cursor)?;

    Ok(Header {
        version,
        format_field,
        channel_count: channel_count as usize,
        raw_pixel_count: raw_pixel_count as usize,
        reserved_field,
        model,
        sample_name,
        instrument_serial,
        scan_count,
        wavelength_first,
        wavelength_last,
        timestamp,
        integration_time_ms,
        operator,
        data_offset: cursor,
    })
}

fn build_metadata(header: &Header) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    metadata.insert("format".to_string(), json!("viavi-micronir-sam"));
    metadata.insert("format_version".to_string(), json!(header.version));
    metadata.insert("format_field".to_string(), json!(header.format_field));
    metadata.insert("reserved_field".to_string(), json!(header.reserved_field));
    metadata.insert("instrument_model".to_string(), json!(header.model));
    metadata.insert("channel_count".to_string(), json!(header.channel_count));
    metadata.insert("raw_pixel_count".to_string(), json!(header.raw_pixel_count));
    if !header.sample_name.is_empty() {
        metadata.insert("sample_name".to_string(), json!(header.sample_name));
    }
    if !header.instrument_serial.is_empty() {
        metadata.insert(
            "instrument_serial".to_string(),
            json!(header.instrument_serial),
        );
    }
    if !header.operator.is_empty() {
        metadata.insert("operator".to_string(), json!(header.operator));
    }
    if let Some(timestamp) = &header.timestamp {
        metadata.insert("acquired_at".to_string(), json!(timestamp));
    }
    metadata.insert("scan_count".to_string(), json!(header.scan_count));
    metadata.insert(
        "integration_time_ms".to_string(),
        json!(header.integration_time_ms as f64),
    );
    metadata.insert(
        "wavelength_first_nm".to_string(),
        json!(header.wavelength_first),
    );
    metadata.insert(
        "wavelength_last_nm".to_string(),
        json!(header.wavelength_last),
    );
    metadata.insert(
        "absorbance_signal".to_string(),
        json!(safe_signal_name("absorbance", "absorbance")),
    );
    metadata
}

fn derived_axis(first: f64, last: f64, len: usize) -> Vec<f64> {
    if len <= 1 {
        return vec![first];
    }
    let step = (last - first) / (len - 1) as f64;
    (0..len).map(|index| first + step * index as f64).collect()
}

fn read_dotnet_string(bytes: &[u8], cursor: &mut usize) -> Result<String> {
    let len = read_7bit_len(bytes, cursor)?;
    let start = *cursor;
    let end = start
        .checked_add(len)
        .ok_or_else(|| Error::InvalidRecord("MicroNIR string length overflows".to_string()))?;
    let slice = bytes.get(start..end).ok_or_else(|| {
        Error::InvalidRecord("MicroNIR string extends past end of file".to_string())
    })?;
    *cursor = end;
    Ok(decode_text(slice).trim_end_matches('\0').trim().to_string())
}

fn read_7bit_len(bytes: &[u8], cursor: &mut usize) -> Result<usize> {
    let mut result: usize = 0;
    let mut shift = 0u32;
    loop {
        if shift >= 35 {
            return Err(Error::InvalidRecord(
                "MicroNIR 7-bit length prefix is too long".to_string(),
            ));
        }
        let byte = *bytes.get(*cursor).ok_or_else(|| {
            Error::InvalidRecord("truncated MicroNIR string length prefix".to_string())
        })?;
        *cursor += 1;
        result |= ((byte & 0x7f) as usize) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    Ok(result)
}

fn read_f64_slice(bytes: &[u8], offset: usize, len: usize) -> Result<Vec<f64>> {
    (0..len)
        .map(|index| {
            let start = offset + index * 8;
            let raw = bytes
                .get(start..start + 8)
                .ok_or_else(|| Error::InvalidRecord("truncated MicroNIR data array".to_string()))?;
            Ok(f64::from_le_bytes(raw.try_into().expect("slice len")))
        })
        .collect()
}

fn read_i32(bytes: &[u8], cursor: &mut usize) -> Result<i32> {
    let raw = bytes
        .get(*cursor..*cursor + 4)
        .ok_or_else(|| Error::InvalidRecord("truncated MicroNIR i32 field".to_string()))?;
    *cursor += 4;
    Ok(i32::from_le_bytes(raw.try_into().expect("slice len")))
}

fn read_i64(bytes: &[u8], cursor: &mut usize) -> Result<i64> {
    let raw = bytes
        .get(*cursor..*cursor + 8)
        .ok_or_else(|| Error::InvalidRecord("truncated MicroNIR i64 field".to_string()))?;
    *cursor += 8;
    Ok(i64::from_le_bytes(raw.try_into().expect("slice len")))
}

fn read_f32(bytes: &[u8], cursor: &mut usize) -> Result<f32> {
    let raw = bytes
        .get(*cursor..*cursor + 4)
        .ok_or_else(|| Error::InvalidRecord("truncated MicroNIR f32 field".to_string()))?;
    *cursor += 4;
    Ok(f32::from_le_bytes(raw.try_into().expect("slice len")))
}

fn read_f64(bytes: &[u8], cursor: &mut usize) -> Result<f64> {
    let raw = bytes
        .get(*cursor..*cursor + 8)
        .ok_or_else(|| Error::InvalidRecord("truncated MicroNIR f64 field".to_string()))?;
    *cursor += 8;
    Ok(f64::from_le_bytes(raw.try_into().expect("slice len")))
}

fn dotnet_ticks_to_iso(raw_ticks: i64) -> Option<String> {
    let ticks = raw_ticks & TICKS_MASK;
    let unix_ticks = ticks.checked_sub(TICKS_AT_UNIX_EPOCH)?;
    let unix_seconds = unix_ticks.div_euclid(10_000_000);
    let sub_ticks = unix_ticks.rem_euclid(10_000_000);
    if !(-62_135_596_800..=253_402_300_799).contains(&unix_seconds) {
        return None;
    }
    let micros = sub_ticks / 10;
    Some(format_iso_utc(unix_seconds, micros as u32))
}

fn format_iso_utc(unix_seconds: i64, micros: u32) -> String {
    let days = unix_seconds.div_euclid(86_400);
    let secs_of_day = unix_seconds.rem_euclid(86_400);
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;
    let (year, month, day) = civil_from_days(days);
    if micros == 0 {
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
    } else {
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{micros:06}Z")
    }
}

/// Convert a day count from the Unix epoch (1970-01-01) into a civil date,
/// using Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if month <= 2 { year + 1 } else { year };
    (year, month, day)
}

fn decode_text(bytes: &[u8]) -> String {
    std::str::from_utf8(bytes)
        .map(|value| value.to_string())
        .unwrap_or_else(|_| bytes.iter().map(|byte| *byte as char).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_decode_matches_filename_timestamp() {
        // 639141756640835594 -> 2026-05-12T09:41:04 (from COT26A001_1).
        let iso = dotnet_ticks_to_iso(639_141_756_640_835_594).unwrap();
        assert!(iso.starts_with("2026-05-12T09:41:04"), "{iso}");
    }

    #[test]
    fn derived_axis_spans_endpoints() {
        let axis = derived_axis(908.1, 1676.2, 125);
        assert_eq!(axis.len(), 125);
        assert!((axis[0] - 908.1).abs() < 1e-9);
        assert!((axis[124] - 1676.2).abs() < 1e-9);
    }
}
