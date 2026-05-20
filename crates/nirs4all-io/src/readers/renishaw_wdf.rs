use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile};
use serde_json::json;

use crate::readers::util::{single_signal_record, SingleSignalSpec};
use crate::Reader;

const WDF_MAGIC: &[u8; 4] = b"WDF1";
const WDF_BLOCK_HEADER_LEN: usize = 16;
const WDF_HEADER_LEN: usize = 512;

pub struct RenishawWdfReader;

impl Reader for RenishawWdfReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::renishaw_wdf"
    }

    fn sniff(&self, head: &[u8], _path: &Path) -> Option<FormatProbe> {
        if !head.starts_with(WDF_MAGIC) {
            return None;
        }
        let block_size = read_u64_from(head, 8)?;
        if block_size < WDF_HEADER_LEN as u64 {
            return None;
        }
        Some(FormatProbe::new(
            "renishaw-wdf",
            self.name(),
            Confidence::Definite,
            "Renishaw WiRE WDF1 chunked spectral container",
        ))
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let source = SourceFile::from_bytes(path, &bytes, "primary");
        parse_renishaw_wdf(&bytes, source, self.name())
    }
}

#[derive(Clone)]
struct WdfBlock {
    name: String,
    payload_offset: usize,
    payload_len: usize,
}

struct WdfHeader {
    point_count: usize,
    capacity: u64,
    count: u64,
    accumulation_count: u32,
    y_size: u32,
    x_size: u32,
    other_data_count: u32,
    application_name: String,
    application_version: String,
    scan_type: u32,
    measurement_type: u32,
    spectral_unit: u32,
    laser_wavenumber: f32,
    username: String,
    title: String,
}

fn parse_renishaw_wdf(
    bytes: &[u8],
    source: SourceFile,
    reader: &str,
) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
    if !bytes.starts_with(WDF_MAGIC) {
        return Err(Error::InvalidRecord(
            "missing Renishaw WDF1 header".to_string(),
        ));
    }
    let header = parse_wdf_header(bytes)?;
    if header.measurement_type == 0 {
        return Err(Error::InvalidRecord(format!(
            "Renishaw WDF acquisition has undefined measurement_type=0 count={}",
            header.count
        )));
    }
    if header.point_count == 0 || header.count == 0 {
        return Err(Error::InvalidRecord(
            "Renishaw WDF point or spectrum count is zero".to_string(),
        ));
    }

    let blocks = parse_blocks(bytes)?;
    let data_block = find_block(&blocks, "DATA")?;
    let xlist_block = find_block(&blocks, "XLST")?;
    let ylist_block = blocks.iter().find(|block| block.name == "YLST");

    let x_data_type = read_u32(bytes, xlist_block.payload_offset)?;
    let x_unit_code = read_u32(bytes, xlist_block.payload_offset + 4)?;
    let axis_values = read_f32_values(bytes, xlist_block.payload_offset + 8, header.point_count)?;
    let spectrum_count = usize::try_from(header.count).map_err(|_| {
        Error::InvalidRecord("Renishaw WDF spectrum count does not fit usize".to_string())
    })?;
    let available_spectra = data_block.payload_len / (header.point_count * 4);
    if available_spectra < spectrum_count {
        return Err(Error::InvalidRecord(format!(
            "Renishaw WDF DATA block contains {available_spectra} spectra but header count is {spectrum_count}"
        )));
    }
    let (axis_kind, axis_unit) = wdf_axis_kind_unit(x_unit_code);

    let (y_data_type, y_unit_code) = if let Some(block) = ylist_block {
        (
            Some(read_u32(bytes, block.payload_offset)?),
            Some(read_u32(bytes, block.payload_offset + 4)?),
        )
    } else {
        (None, None)
    };
    let signal_unit = y_unit_code.and_then(wdf_signal_unit);

    let mut metadata = BTreeMap::new();
    metadata.insert("container".to_string(), json!("wdf1"));
    metadata.insert("point_count".to_string(), json!(header.point_count));
    metadata.insert("capacity".to_string(), json!(header.capacity));
    metadata.insert("spectrum_count".to_string(), json!(header.count));
    metadata.insert(
        "accumulation_count".to_string(),
        json!(header.accumulation_count),
    );
    metadata.insert("x_size".to_string(), json!(header.x_size));
    metadata.insert("y_size".to_string(), json!(header.y_size));
    metadata.insert(
        "other_data_count".to_string(),
        json!(header.other_data_count),
    );
    metadata.insert("scan_type".to_string(), json!(header.scan_type));
    metadata.insert(
        "measurement_type".to_string(),
        json!(header.measurement_type),
    );
    metadata.insert(
        "measurement_type_label".to_string(),
        json!(measurement_type_label(header.measurement_type)),
    );
    metadata.insert(
        "spectral_unit_code".to_string(),
        json!(header.spectral_unit),
    );
    metadata.insert("x_data_type".to_string(), json!(x_data_type));
    metadata.insert("x_unit_code".to_string(), json!(x_unit_code));
    if let Some(value) = y_data_type {
        metadata.insert("y_data_type".to_string(), json!(value));
    }
    if let Some(value) = y_unit_code {
        metadata.insert("y_unit_code".to_string(), json!(value));
    }
    metadata.insert(
        "laser_wavenumber".to_string(),
        json!(header.laser_wavenumber),
    );
    if !header.application_name.is_empty() {
        metadata.insert(
            "application_name".to_string(),
            json!(header.application_name),
        );
    }
    if !header.application_version.is_empty() {
        metadata.insert(
            "application_version".to_string(),
            json!(header.application_version),
        );
    }
    if !header.username.is_empty() {
        metadata.insert("username".to_string(), json!(header.username));
    }
    if !header.title.is_empty() {
        metadata.insert("title".to_string(), json!(header.title));
    }

    let mut warnings = vec!["renishaw_wdf_reverse_engineered_chunks".to_string()];
    if spectrum_count > 1 {
        warnings.push("renishaw_wdf_navigation_axes_pending".to_string());
    }
    if header.capacity > header.count && available_spectra > spectrum_count {
        warnings.push("renishaw_wdf_interrupted_acquisition_truncated_to_count".to_string());
    }

    let mut records = Vec::with_capacity(spectrum_count);
    for spectrum_index in 0..spectrum_count {
        let offset = data_block.payload_offset + spectrum_index * header.point_count * 4;
        let values = read_f32_values(bytes, offset, header.point_count)?;
        let mut record_metadata = metadata.clone();
        record_metadata.insert("spectrum_index".to_string(), json!(spectrum_index));
        let record = single_signal_record(
            "renishaw-wdf",
            reader,
            source.clone(),
            SingleSignalSpec {
                axis_values: axis_values.clone(),
                axis_unit: axis_unit.clone(),
                axis_kind: axis_kind.clone(),
                values,
                signal_name: "raw_counts".to_string(),
                signal_type: SignalType::RawCounts,
                signal_unit: signal_unit.clone(),
                role: "raw_counts".to_string(),
            },
            BTreeMap::new(),
            record_metadata,
            warnings.clone(),
        )?;
        records.push(record);
    }
    Ok(records)
}

fn parse_wdf_header(bytes: &[u8]) -> Result<WdfHeader> {
    if bytes.len() < WDF_HEADER_LEN {
        return Err(Error::InvalidRecord(
            "Renishaw WDF file is too short for the WDF1 header".to_string(),
        ));
    }
    let point_count = read_u32(bytes, 0x003c)? as usize;
    let version = (0..4)
        .map(|index| read_u16(bytes, 0x0078 + index * 2).map(|value| value.to_string()))
        .collect::<Result<Vec<_>>>()?
        .join(".");

    Ok(WdfHeader {
        point_count,
        capacity: read_u64(bytes, 0x0040)?,
        count: read_u64(bytes, 0x0048)?,
        accumulation_count: read_u32(bytes, 0x0050)?,
        y_size: read_u32(bytes, 0x0054)?,
        x_size: read_u32(bytes, 0x0058)?,
        other_data_count: read_u32(bytes, 0x005c)?,
        application_name: null_terminated_ascii(&bytes[0x0060..0x0078]),
        application_version: version,
        scan_type: read_u32(bytes, 0x0080)?,
        measurement_type: read_u32(bytes, 0x0084)?,
        spectral_unit: read_u32(bytes, 0x0098)?,
        laser_wavenumber: read_f32(bytes, 0x009c)?,
        username: null_terminated_ascii(&bytes[0x00d0..0x00f0]),
        title: null_terminated_ascii(&bytes[0x00f0..0x0200]),
    })
}

fn parse_blocks(bytes: &[u8]) -> Result<Vec<WdfBlock>> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while offset + WDF_BLOCK_HEADER_LEN <= bytes.len() {
        let name = String::from_utf8_lossy(&bytes[offset..offset + 4]).into_owned();
        let block_size = read_u64(bytes, offset + 8)? as usize;
        if block_size < WDF_BLOCK_HEADER_LEN {
            return Err(Error::InvalidRecord(format!(
                "Renishaw WDF block {name} at {offset} is shorter than its header"
            )));
        }
        let next = offset.checked_add(block_size).ok_or_else(|| {
            Error::InvalidRecord("Renishaw WDF block offset overflow".to_string())
        })?;
        if next > bytes.len() {
            return Err(Error::InvalidRecord(format!(
                "Renishaw WDF block {name} at {offset} extends past end of file"
            )));
        }
        out.push(WdfBlock {
            name,
            payload_offset: offset + WDF_BLOCK_HEADER_LEN,
            payload_len: block_size - WDF_BLOCK_HEADER_LEN,
        });
        offset = next;
    }
    if offset != bytes.len() {
        return Err(Error::InvalidRecord(
            "Renishaw WDF block stream ended on a partial header".to_string(),
        ));
    }
    Ok(out)
}

fn find_block<'a>(blocks: &'a [WdfBlock], name: &str) -> Result<&'a WdfBlock> {
    blocks
        .iter()
        .find(|block| block.name == name)
        .ok_or_else(|| Error::InvalidRecord(format!("Renishaw WDF missing {name} block")))
}

fn read_f32_values(bytes: &[u8], offset: usize, count: usize) -> Result<Vec<f64>> {
    let byte_len = count
        .checked_mul(4)
        .ok_or_else(|| Error::InvalidRecord("Renishaw WDF vector length overflow".to_string()))?;
    if offset + byte_len > bytes.len() {
        return Err(Error::InvalidRecord(
            "Renishaw WDF vector extends past end of file".to_string(),
        ));
    }
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        out.push(read_f32(bytes, offset + index * 4)? as f64);
    }
    Ok(out)
}

fn wdf_axis_kind_unit(unit_code: u32) -> (AxisKind, String) {
    match unit_code {
        1 => (AxisKind::Wavenumber, "cm-1".to_string()),
        2 | 3 => (AxisKind::Wavelength, "nm".to_string()),
        _ => (AxisKind::Index, format!("wdf-unit-{unit_code}")),
    }
}

fn wdf_signal_unit(unit_code: u32) -> Option<String> {
    match unit_code {
        16 => Some("counts".to_string()),
        _ => None,
    }
}

fn measurement_type_label(measurement_type: u32) -> &'static str {
    match measurement_type {
        1 => "single",
        2 => "series",
        3 => "mapping",
        _ => "unknown",
    }
}

fn null_terminated_ascii(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let value = bytes.get(offset..offset + 2).ok_or_else(|| {
        Error::InvalidRecord("Renishaw WDF u16 read past end of file".to_string())
    })?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let value = bytes.get(offset..offset + 4).ok_or_else(|| {
        Error::InvalidRecord("Renishaw WDF u32 read past end of file".to_string())
    })?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    let value = bytes.get(offset..offset + 8).ok_or_else(|| {
        Error::InvalidRecord("Renishaw WDF u64 read past end of file".to_string())
    })?;
    Ok(u64::from_le_bytes([
        value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
    ]))
}

fn read_u64_from(bytes: &[u8], offset: usize) -> Option<u64> {
    let value = bytes.get(offset..offset + 8)?;
    Some(u64::from_le_bytes([
        value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
    ]))
}

fn read_f32(bytes: &[u8], offset: usize) -> Result<f32> {
    let value = bytes.get(offset..offset + 4).ok_or_else(|| {
        Error::InvalidRecord("Renishaw WDF f32 read past end of file".to_string())
    })?;
    Ok(f32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}
