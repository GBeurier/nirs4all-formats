use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralArray,
    SpectralAxis,
};
use serde_json::{json, Value};

use crate::readers::util::record_from_signals;
use crate::Reader;

const FORMAT: &str = "hamamatsu-img";
const HEADER_LEN: usize = 64;

pub struct HamamatsuImgReader;

impl Reader for HamamatsuImgReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::hamamatsu_img"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext != "img" || head.len() < HEADER_LEN || !head.starts_with(b"IM") {
            return None;
        }
        let comment_len = i16::from_le_bytes([head[2], head[3]]);
        let width = i16::from_le_bytes([head[4], head[5]]);
        let height = i16::from_le_bytes([head[6], head[7]]);
        let file_type = i16::from_le_bytes([head[12], head[13]]);
        if comment_len <= 0 || width <= 0 || height <= 0 || !matches!(file_type, 0 | 2 | 3) {
            return None;
        }
        let text = String::from_utf8_lossy(head);
        if text.contains("[Application]") && text.contains("HPD-TA") {
            return Some(FormatProbe::new(
                FORMAT,
                self.name(),
                Confidence::Definite,
                "Hamamatsu HPD-TA streak camera IMG container",
            ));
        }
        None
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let source = SourceFile::from_bytes(path, &bytes, "primary");
        parse_hamamatsu_img(&bytes, source, self.name())
    }
}

#[derive(Clone)]
struct ImgHeader {
    character: String,
    comment_len: usize,
    width: usize,
    height: usize,
    offset_x: i16,
    offset_y: i16,
    file_type: i16,
    num_images_in_channel: i32,
    num_additional_channels: i16,
    channel_number: i16,
    timestamp: f64,
    marker: String,
    additional_info: String,
}

#[derive(Clone)]
struct AxisData {
    name: String,
    unit: String,
    values: Vec<f64>,
    kind: AxisKind,
}

fn parse_hamamatsu_img(
    bytes: &[u8],
    source: SourceFile,
    reader_name: &str,
) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
    let header = parse_header(bytes)?;
    if header.character != "IM" {
        return Err(Error::InvalidRecord(
            "missing Hamamatsu IMG signature".to_string(),
        ));
    }
    let comment_start = HEADER_LEN;
    let comment_end = comment_start
        .checked_add(header.comment_len)
        .ok_or_else(|| Error::InvalidRecord("Hamamatsu comment length overflows".to_string()))?;
    if comment_end > bytes.len() {
        return Err(Error::InvalidRecord(
            "Hamamatsu IMG comment is truncated".to_string(),
        ));
    }
    let comment = String::from_utf8_lossy(&bytes[comment_start..comment_end]).to_string();
    let sections = parse_comment_sections(&comment);

    let point_size = point_size_bytes(header.file_type)?;
    let data_points = header.width.checked_mul(header.height).ok_or_else(|| {
        Error::InvalidRecord("Hamamatsu IMG dimensions overflow usize".to_string())
    })?;
    let data_bytes = data_points
        .checked_mul(point_size)
        .ok_or_else(|| Error::InvalidRecord("Hamamatsu IMG payload size overflows".to_string()))?;
    let data_start = comment_end;
    let data_end = data_start
        .checked_add(data_bytes)
        .ok_or_else(|| Error::InvalidRecord("Hamamatsu IMG data offset overflows".to_string()))?;
    if data_end > bytes.len() {
        return Err(Error::InvalidRecord(
            "Hamamatsu IMG payload is truncated".to_string(),
        ));
    }

    let mut values = decode_payload(&bytes[data_start..data_end], header.file_type)?;
    let x_axis = build_axis(bytes, &sections, 'X', header.width)?;
    let y_axis = build_axis(bytes, &sections, 'Y', header.height)?;
    let (x_axis, reversed) = normalize_x_axis(x_axis);
    if reversed {
        reverse_rows(&mut values, header.width);
    }

    let spectral_axis = SpectralAxis::new(
        x_axis.values.clone(),
        x_axis.unit.clone(),
        x_axis.kind.clone(),
    )?;
    let signal = SpectralArray::new(
        spectral_axis,
        values,
        vec!["y".to_string(), "x".to_string()],
        SignalType::RawCounts,
        Some(signal_unit(&sections)),
        "intensity",
        "file",
    )?;
    let mut signals = BTreeMap::new();
    signals.insert("intensity".to_string(), signal);

    let mut metadata = base_metadata(&header, &sections, &x_axis, &y_axis);
    metadata.insert("image_width".to_string(), json!(header.width));
    metadata.insert("image_height".to_string(), json!(header.height));

    let mut warnings = vec!["hamamatsu_img_streak_camera_2d_signal".to_string()];
    if reversed {
        warnings.push("hamamatsu_img_x_axis_reversed_to_ascending".to_string());
    }
    if x_axis.kind == AxisKind::Index {
        warnings.push("hamamatsu_img_uncalibrated_x_axis".to_string());
    }
    if y_axis.name == "Vertical CCD Position" {
        warnings.push("hamamatsu_img_y_axis_is_detector_position".to_string());
    } else {
        warnings.push("hamamatsu_img_secondary_time_axis_in_metadata".to_string());
    }

    Ok(vec![record_from_signals(
        FORMAT,
        reader_name,
        source,
        signals,
        SignalType::RawCounts,
        metadata,
        warnings,
    )?])
}

fn parse_header(bytes: &[u8]) -> Result<ImgHeader> {
    if bytes.len() < HEADER_LEN {
        return Err(Error::InvalidRecord(
            "Hamamatsu IMG header is truncated".to_string(),
        ));
    }
    let comment_len = read_i16(bytes, 2)?;
    if comment_len <= 0 {
        return Err(Error::InvalidRecord(format!(
            "Hamamatsu IMG comment length must be positive, got {comment_len}"
        )));
    }
    let width = read_i16(bytes, 4)?;
    let height = read_i16(bytes, 6)?;
    if width <= 0 || height <= 0 {
        return Err(Error::InvalidRecord(format!(
            "Hamamatsu IMG dimensions must be positive, got {width}x{height}"
        )));
    }
    Ok(ImgHeader {
        character: read_string(bytes, 0, 2)?,
        comment_len: comment_len as usize,
        width: width as usize,
        height: height as usize,
        offset_x: read_i16(bytes, 8)?,
        offset_y: read_i16(bytes, 10)?,
        file_type: read_i16(bytes, 12)?,
        num_images_in_channel: read_i32(bytes, 14)?,
        num_additional_channels: read_i16(bytes, 18)?,
        channel_number: read_i16(bytes, 20)?,
        timestamp: read_f64(bytes, 22)?,
        marker: read_string(bytes, 30, 4)?,
        additional_info: read_string(bytes, 34, 30)?,
    })
}

fn decode_payload(bytes: &[u8], file_type: i16) -> Result<Vec<f64>> {
    match file_type {
        0 => Ok(bytes.iter().map(|value| f64::from(*value)).collect()),
        2 => Ok(bytes
            .chunks_exact(2)
            .map(|chunk| f64::from(u16::from_le_bytes([chunk[0], chunk[1]])))
            .collect()),
        3 => Ok(bytes
            .chunks_exact(4)
            .map(|chunk| f64::from(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])))
            .collect()),
        other => Err(Error::InvalidRecord(format!(
            "Hamamatsu IMG file_type {other} is unsupported"
        ))),
    }
}

fn build_axis(
    bytes: &[u8],
    sections: &BTreeMap<String, BTreeMap<String, String>>,
    axis: char,
    fallback_size: usize,
) -> Result<AxisData> {
    let scaling = sections.get("Scaling");
    let prefix = format!("Scaling{axis}");
    let scale_type = scaling
        .and_then(|section| section.get(&format!("{prefix}Type")))
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(1);
    let raw_unit = scaling
        .and_then(|section| section.get(&format!("{prefix}Unit")))
        .map(String::as_str)
        .unwrap_or("");
    let unit = normalize_unit(raw_unit);

    if scale_type == 1 {
        let (name, unit) = if axis == 'X' {
            ("Uncalibrated X axis".to_string(), "px".to_string())
        } else {
            ("Vertical CCD Position".to_string(), "px".to_string())
        };
        return Ok(AxisData {
            name,
            unit,
            values: (0..fallback_size).map(|index| index as f64).collect(),
            kind: AxisKind::Index,
        });
    }

    if scale_type != 2 {
        return Err(Error::InvalidRecord(format!(
            "Hamamatsu IMG Scaling{axis}Type {scale_type} is unsupported"
        )));
    }

    let cal_ref = scaling
        .and_then(|section| section.get(&format!("{prefix}ScalingFile")))
        .ok_or_else(|| {
            Error::InvalidRecord(format!("Hamamatsu IMG Scaling{axis}ScalingFile is missing"))
        })?;
    let (offset, size) = parse_calibration_ref(cal_ref)?;
    let values = read_f32_array(bytes, offset, size)?;
    let name = if axis == 'X' { "Wavelength" } else { "Time" };
    Ok(AxisData {
        name: name.to_string(),
        unit,
        values,
        kind: if axis == 'X' {
            AxisKind::Wavelength
        } else {
            AxisKind::Index
        },
    })
}

fn normalize_x_axis(mut axis: AxisData) -> (AxisData, bool) {
    if axis.values.len() > 1 && axis.values[0] > axis.values[1] {
        axis.values.reverse();
        (axis, true)
    } else {
        (axis, false)
    }
}

fn reverse_rows(values: &mut [f64], width: usize) {
    for row in values.chunks_exact_mut(width) {
        row.reverse();
    }
}

fn parse_calibration_ref(value: &str) -> Result<(usize, usize)> {
    let trimmed = value.trim().trim_matches('"');
    let Some(raw) = trimmed.strip_prefix('#') else {
        return Err(Error::InvalidRecord(format!(
            "Hamamatsu IMG calibration reference {value:?} is not an address"
        )));
    };
    let mut parts = raw.split(',');
    let offset = parts
        .next()
        .and_then(|part| part.parse::<usize>().ok())
        .ok_or_else(|| {
            Error::InvalidRecord(format!(
                "Hamamatsu IMG calibration reference {value:?} has invalid offset"
            ))
        })?;
    let size = parts
        .next()
        .and_then(|part| part.parse::<usize>().ok())
        .ok_or_else(|| {
            Error::InvalidRecord(format!(
                "Hamamatsu IMG calibration reference {value:?} has invalid size"
            ))
        })?;
    Ok((offset, size))
}

fn read_f32_array(bytes: &[u8], offset: usize, size: usize) -> Result<Vec<f64>> {
    let byte_len = size.checked_mul(4).ok_or_else(|| {
        Error::InvalidRecord("Hamamatsu IMG calibration byte length overflows".to_string())
    })?;
    let end = offset.checked_add(byte_len).ok_or_else(|| {
        Error::InvalidRecord("Hamamatsu IMG calibration offset overflows".to_string())
    })?;
    if end > bytes.len() {
        return Err(Error::InvalidRecord(
            "Hamamatsu IMG calibration array is truncated".to_string(),
        ));
    }
    Ok(bytes[offset..end]
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as f64)
        .collect())
}

fn parse_comment_sections(comment: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut sections = BTreeMap::new();
    for part in comment.trim_start_matches('[').split('[') {
        let Some((name, body)) = part.split_once(']') else {
            continue;
        };
        sections.insert(name.trim().to_string(), parse_section_body(body));
    }
    sections
}

fn parse_section_body(body: &str) -> BTreeMap<String, String> {
    let mut entries = BTreeMap::new();
    let bytes = body
        .trim_start_matches(',')
        .trim_matches(|ch| matches!(ch, '\0' | '\r' | '\n'))
        .as_bytes();
    let mut pos = 0usize;
    while pos < bytes.len() {
        let key_start = pos;
        while pos < bytes.len() && bytes[pos] != b'=' {
            pos += 1;
        }
        if pos >= bytes.len() {
            break;
        }
        let key = String::from_utf8_lossy(&bytes[key_start..pos])
            .trim()
            .to_string();
        pos += 1;
        let value = if pos < bytes.len() && bytes[pos] == b'"' {
            pos += 1;
            let value_start = pos;
            while pos < bytes.len() && bytes[pos] != b'"' {
                pos += 1;
            }
            let value = String::from_utf8_lossy(&bytes[value_start..pos]).to_string();
            if pos < bytes.len() && bytes[pos] == b'"' {
                pos += 1;
            }
            value
        } else {
            let value_start = pos;
            while pos < bytes.len() && bytes[pos] != b',' {
                pos += 1;
            }
            String::from_utf8_lossy(&bytes[value_start..pos])
                .trim()
                .to_string()
        };
        if !key.is_empty() {
            entries.insert(key, value);
        }
        if pos < bytes.len() && bytes[pos] == b',' {
            pos += 1;
        }
    }
    entries
}

fn base_metadata(
    header: &ImgHeader,
    sections: &BTreeMap<String, BTreeMap<String, String>>,
    x_axis: &AxisData,
    y_axis: &AxisData,
) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    metadata.insert("container".to_string(), json!("hamamatsu_img"));
    metadata.insert("comment_length".to_string(), json!(header.comment_len));
    metadata.insert("offset_x".to_string(), json!(header.offset_x));
    metadata.insert("offset_y".to_string(), json!(header.offset_y));
    metadata.insert("file_type_code".to_string(), json!(header.file_type));
    metadata.insert(
        "file_type_label".to_string(),
        json!(file_type_label(header.file_type)),
    );
    metadata.insert(
        "num_images_in_channel".to_string(),
        json!(header.num_images_in_channel),
    );
    metadata.insert(
        "num_additional_channels".to_string(),
        json!(header.num_additional_channels),
    );
    metadata.insert("channel_number".to_string(), json!(header.channel_number));
    metadata.insert("timestamp".to_string(), json!(header.timestamp));
    if !header.marker.is_empty() {
        metadata.insert("marker".to_string(), json!(header.marker));
    }
    if !header.additional_info.is_empty() {
        metadata.insert("additional_info".to_string(), json!(header.additional_info));
    }

    insert_axis_metadata(&mut metadata, "x_axis", x_axis);
    insert_axis_metadata(&mut metadata, "y_axis", y_axis);
    metadata.insert("y_axis_values".to_string(), json!(y_axis.values));
    insert_section_value(&mut metadata, sections, "Application", "Date", "date");
    insert_section_value(&mut metadata, sections, "Application", "Time", "time");
    insert_section_value(
        &mut metadata,
        sections,
        "Application",
        "Software",
        "software",
    );
    insert_section_value(
        &mut metadata,
        sections,
        "Application",
        "SoftwareVersion",
        "software_version",
    );
    insert_section_value(
        &mut metadata,
        sections,
        "Acquisition",
        "AcqMode",
        "acquisition_mode_code",
    );
    metadata.insert(
        "acquisition_mode_label".to_string(),
        json!(acquisition_mode_label(
            section_value(sections, "Acquisition", "AcqMode")
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(0)
        )),
    );
    insert_section_value(
        &mut metadata,
        sections,
        "Acquisition",
        "ExposureTime",
        "exposure_time",
    );
    insert_section_value(
        &mut metadata,
        sections,
        "Acquisition",
        "NrExposure",
        "exposure_count",
    );
    insert_section_value(
        &mut metadata,
        sections,
        "Acquisition",
        "pntBinning",
        "binning",
    );
    insert_section_value(
        &mut metadata,
        sections,
        "Streak camera",
        "DeviceName",
        "streak_camera_model",
    );
    insert_section_value(
        &mut metadata,
        sections,
        "Streak camera",
        "Mode",
        "streak_camera_mode",
    );
    insert_section_value(
        &mut metadata,
        sections,
        "Streak camera",
        "Time Range",
        "time_range",
    );
    insert_section_value(
        &mut metadata,
        sections,
        "Streak camera",
        "MCP Gain",
        "mcp_gain",
    );
    insert_section_value(
        &mut metadata,
        sections,
        "Spectrograph",
        "DeviceName",
        "spectrograph_model",
    );
    insert_section_value(
        &mut metadata,
        sections,
        "Spectrograph",
        "Wavelength",
        "central_wavelength",
    );
    insert_section_value(
        &mut metadata,
        sections,
        "Spectrograph",
        "Grating",
        "grating",
    );
    metadata
}

fn insert_axis_metadata(metadata: &mut BTreeMap<String, Value>, prefix: &str, axis: &AxisData) {
    metadata.insert(format!("{prefix}_name"), json!(axis.name));
    metadata.insert(format!("{prefix}_unit"), json!(axis.unit));
    metadata.insert(format!("{prefix}_kind"), json!(axis.kind));
    metadata.insert(format!("{prefix}_len"), json!(axis.values.len()));
    if let Some(first) = axis.values.first() {
        metadata.insert(format!("{prefix}_first"), json!(first));
    }
    if let Some(last) = axis.values.last() {
        metadata.insert(format!("{prefix}_last"), json!(last));
    }
}

fn insert_section_value(
    metadata: &mut BTreeMap<String, Value>,
    sections: &BTreeMap<String, BTreeMap<String, String>>,
    section: &str,
    key: &str,
    metadata_key: &str,
) {
    if let Some(value) = section_value(sections, section, key) {
        metadata.insert(metadata_key.to_string(), json!(value));
    }
}

fn section_value<'a>(
    sections: &'a BTreeMap<String, BTreeMap<String, String>>,
    section: &str,
    key: &str,
) -> Option<&'a str> {
    sections
        .get(section)
        .and_then(|values| values.get(key))
        .map(String::as_str)
}

fn signal_unit(sections: &BTreeMap<String, BTreeMap<String, String>>) -> String {
    match section_value(sections, "Acquisition", "ZAxisUnit") {
        Some("Count") => "Counts".to_string(),
        Some(value) if !value.is_empty() => value.to_string(),
        _ => "Counts".to_string(),
    }
}

fn normalize_unit(unit: &str) -> String {
    let trimmed = unit.trim();
    if trimmed.is_empty() {
        "px".to_string()
    } else {
        trimmed.to_string()
    }
}

fn point_size_bytes(file_type: i16) -> Result<usize> {
    match file_type {
        0 => Ok(1),
        2 => Ok(2),
        3 => Ok(4),
        other => Err(Error::InvalidRecord(format!(
            "Hamamatsu IMG file_type {other} is unsupported"
        ))),
    }
}

fn file_type_label(file_type: i16) -> &'static str {
    match file_type {
        0 => "bit8",
        1 => "compressed",
        2 => "bit16",
        3 => "bit32",
        _ => "unknown",
    }
}

fn acquisition_mode_label(code: i32) -> &'static str {
    match code {
        1 => "live",
        2 => "acquire",
        3 => "photon_counting",
        4 => "analog_integration",
        _ => "unknown",
    }
}

fn read_string(bytes: &[u8], offset: usize, len: usize) -> Result<String> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| Error::InvalidRecord("Hamamatsu string offset overflows".to_string()))?;
    if end > bytes.len() {
        return Err(Error::InvalidRecord(
            "Hamamatsu string field is truncated".to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&bytes[offset..end])
        .replace('\0', "")
        .trim()
        .to_string())
}

fn read_i16(bytes: &[u8], offset: usize) -> Result<i16> {
    let raw = read_array::<2>(bytes, offset)?;
    Ok(i16::from_le_bytes(raw))
}

fn read_i32(bytes: &[u8], offset: usize) -> Result<i32> {
    let raw = read_array::<4>(bytes, offset)?;
    Ok(i32::from_le_bytes(raw))
}

fn read_f64(bytes: &[u8], offset: usize) -> Result<f64> {
    let raw = read_array::<8>(bytes, offset)?;
    Ok(f64::from_le_bytes(raw))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset
        .checked_add(N)
        .ok_or_else(|| Error::InvalidRecord("Hamamatsu field offset overflows".to_string()))?;
    if end > bytes.len() {
        return Err(Error::InvalidRecord(
            "Hamamatsu numeric field is truncated".to_string(),
        ));
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes[offset..end]);
    Ok(out)
}
