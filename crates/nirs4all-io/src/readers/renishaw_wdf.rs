use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

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
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(
        &self,
        path: &Path,
        bytes: &[u8],
    ) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let source = SourceFile::from_bytes(path, bytes, "primary");
        parse_renishaw_wdf(bytes, source, self.name())
    }
}

#[derive(Clone)]
struct WdfBlock {
    name: String,
    block_uid: u32,
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

#[derive(Clone)]
struct WdfOriginAxis {
    data_type_code: u32,
    data_type_label: &'static str,
    high_bit_set: bool,
    unit_code: u32,
    unit: String,
    annotation: String,
    values: WdfOriginValues,
}

#[derive(Clone)]
enum WdfOriginValues {
    Float(Vec<f64>),
    U64(Vec<u64>),
}

#[derive(Clone)]
struct WdfMapInfo {
    map_type_code: u32,
    map_type_label: &'static str,
    reserved: u32,
    offset_xyz: [f64; 3],
    scale_xyz: [f64; 3],
    size_xyz: [u32; 3],
    linefocus_size: u32,
}

#[derive(Default)]
struct WdfNavigation {
    origin_axes: Vec<WdfOriginAxis>,
    map: Option<WdfMapInfo>,
    warnings: Vec<String>,
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
    let navigation = parse_navigation(bytes, &blocks, &header, spectrum_count)?;

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
    insert_navigation_metadata(&mut metadata, &navigation);
    let map_analysis_ranges =
        insert_auxiliary_block_metadata(&mut metadata, bytes, &blocks, spectrum_count);

    let mut warnings = vec!["renishaw_wdf_reverse_engineered_chunks".to_string()];
    warnings.extend(navigation.warnings.iter().cloned());
    if spectrum_count > 1 && navigation.origin_axes.is_empty() && navigation.map.is_none() {
        warnings.push("renishaw_wdf_navigation_axes_missing".to_string());
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
        insert_record_navigation(&mut record_metadata, &navigation, spectrum_index);
        insert_record_map_analysis(&mut record_metadata, &map_analysis_ranges, spectrum_index);
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

fn parse_navigation(
    bytes: &[u8],
    blocks: &[WdfBlock],
    header: &WdfHeader,
    spectrum_count: usize,
) -> Result<WdfNavigation> {
    let mut navigation = WdfNavigation::default();

    if let Some(block) = blocks.iter().find(|block| block.name == "ORGN") {
        let (origin_axes, warnings) = parse_origin_axes(bytes, block, header, spectrum_count)?;
        navigation.origin_axes = origin_axes;
        navigation.warnings.extend(warnings);
    }

    if let Some(block) = blocks.iter().find(|block| block.name == "WMAP") {
        let map = parse_wmap(bytes, block)?;
        if !matches!(map.map_type_code, 0 | 2 | 128) {
            navigation
                .warnings
                .push("renishaw_wdf_map_type_not_fully_normalized".to_string());
        }
        navigation.map = Some(map);
    } else if header.measurement_type == 3 && spectrum_count > 1 {
        navigation
            .warnings
            .push("renishaw_wdf_mapping_wmap_missing".to_string());
    }

    Ok(navigation)
}

fn parse_origin_axes(
    bytes: &[u8],
    block: &WdfBlock,
    header: &WdfHeader,
    spectrum_count: usize,
) -> Result<(Vec<WdfOriginAxis>, Vec<String>)> {
    if block.payload_len < 4 {
        return Err(Error::InvalidRecord(
            "Renishaw WDF ORGN block is too short for axis count".to_string(),
        ));
    }

    let origin_count = read_u32(bytes, block.payload_offset)? as usize;
    let mut warnings = Vec::new();
    if header.other_data_count != origin_count as u32 {
        warnings.push("renishaw_wdf_origin_axis_count_mismatch".to_string());
    }
    if origin_count == 0 {
        return Ok((Vec::new(), warnings));
    }

    let capacity_count = usize::try_from(header.capacity).map_err(|_| {
        Error::InvalidRecord("Renishaw WDF ORGN capacity does not fit usize".to_string())
    })?;
    let capacity_stride = origin_axis_stride(capacity_count)?;
    let capacity_expected_len = origin_block_len(origin_count, capacity_stride)?;
    let value_slots = if capacity_expected_len <= block.payload_len {
        capacity_count
    } else {
        let count_stride = origin_axis_stride(spectrum_count)?;
        let count_expected_len = origin_block_len(origin_count, count_stride)?;
        if count_expected_len <= block.payload_len {
            warnings.push("renishaw_wdf_origin_axis_uses_count_stride".to_string());
            spectrum_count
        } else {
            return Err(Error::InvalidRecord(format!(
                "Renishaw WDF ORGN block length {} does not fit {origin_count} axes",
                block.payload_len
            )));
        }
    };
    let stride = origin_axis_stride(value_slots)?;

    let mut axes = Vec::with_capacity(origin_count);
    for axis_index in 0..origin_count {
        let offset = block.payload_offset + 4 + axis_index * stride;
        let raw_data_type = read_u32(bytes, offset)?;
        let data_type_code = raw_data_type & !(1u32 << 31);
        let unit_code = read_u32(bytes, offset + 4)?;
        let annotation = null_terminated_ascii(&bytes[offset + 8..offset + 24]);
        let values_offset = offset + 24;
        let values = if data_type_code == 11 {
            WdfOriginValues::U64(read_u64_values(bytes, values_offset, spectrum_count)?)
        } else {
            WdfOriginValues::Float(read_f64_values(bytes, values_offset, spectrum_count)?)
        };
        axes.push(WdfOriginAxis {
            data_type_code,
            data_type_label: wdf_origin_data_type_label(data_type_code),
            high_bit_set: (raw_data_type & (1u32 << 31)) != 0,
            unit_code,
            unit: wdf_unit_label(unit_code).to_string(),
            annotation,
            values,
        });
    }

    Ok((axes, warnings))
}

fn origin_axis_stride(value_slots: usize) -> Result<usize> {
    let data_len = value_slots.checked_mul(8).ok_or_else(|| {
        Error::InvalidRecord("Renishaw WDF ORGN axis data length overflow".to_string())
    })?;
    24usize
        .checked_add(data_len)
        .ok_or_else(|| Error::InvalidRecord("Renishaw WDF ORGN axis stride overflow".to_string()))
}

fn origin_block_len(origin_count: usize, stride: usize) -> Result<usize> {
    let axes_len = origin_count.checked_mul(stride).ok_or_else(|| {
        Error::InvalidRecord("Renishaw WDF ORGN block length overflow".to_string())
    })?;
    4usize
        .checked_add(axes_len)
        .ok_or_else(|| Error::InvalidRecord("Renishaw WDF ORGN block length overflow".to_string()))
}

fn parse_wmap(bytes: &[u8], block: &WdfBlock) -> Result<WdfMapInfo> {
    if block.payload_len < 48 {
        return Err(Error::InvalidRecord(
            "Renishaw WDF WMAP block is too short".to_string(),
        ));
    }
    let offset = block.payload_offset;
    let map_type_code = read_u32(bytes, offset)?;
    let reserved = read_u32(bytes, offset + 4)?;
    let offset_xyz = [
        read_f32(bytes, offset + 8)? as f64,
        read_f32(bytes, offset + 12)? as f64,
        read_f32(bytes, offset + 16)? as f64,
    ];
    let scale_xyz = [
        read_f32(bytes, offset + 20)? as f64,
        read_f32(bytes, offset + 24)? as f64,
        read_f32(bytes, offset + 28)? as f64,
    ];
    let size_xyz = [
        read_u32(bytes, offset + 32)?,
        read_u32(bytes, offset + 36)?,
        read_u32(bytes, offset + 40)?,
    ];
    let linefocus_size = read_u32(bytes, offset + 44)?;
    Ok(WdfMapInfo {
        map_type_code,
        map_type_label: wdf_map_type_label(map_type_code),
        reserved,
        offset_xyz,
        scale_xyz,
        size_xyz,
        linefocus_size,
    })
}

fn insert_navigation_metadata(metadata: &mut BTreeMap<String, Value>, navigation: &WdfNavigation) {
    if !navigation.origin_axes.is_empty() {
        metadata.insert(
            "origin_axis_count".to_string(),
            json!(navigation.origin_axes.len()),
        );
        metadata.insert(
            "origin_axes".to_string(),
            json!(navigation
                .origin_axes
                .iter()
                .map(origin_axis_metadata)
                .collect::<Vec<_>>()),
        );
    }

    if let Some(map) = &navigation.map {
        metadata.insert("map_type_code".to_string(), json!(map.map_type_code));
        metadata.insert("map_type_label".to_string(), json!(map.map_type_label));
        metadata.insert("map_width".to_string(), json!(map.size_xyz[0]));
        metadata.insert("map_height".to_string(), json!(map.size_xyz[1]));
        metadata.insert("map_depth".to_string(), json!(map.size_xyz[2]));
        metadata.insert("map_linefocus_size".to_string(), json!(map.linefocus_size));
        metadata.insert(
            "wmap".to_string(),
            json!({
                "map_type": map.map_type_code,
                "map_type_label": map.map_type_label,
                "reserved": map.reserved,
                "offset_xyz": map.offset_xyz,
                "scale_xyz": map.scale_xyz,
                "size_xyz": map.size_xyz,
                "linefocus_size": map.linefocus_size,
            }),
        );
    }
}

fn origin_axis_metadata(axis: &WdfOriginAxis) -> Value {
    json!({
        "data_type": axis.data_type_code,
        "data_type_label": axis.data_type_label,
        "high_bit_set": axis.high_bit_set,
        "unit_code": axis.unit_code,
        "unit": axis.unit.as_str(),
        "annotation": axis.annotation.as_str(),
        "record_count": axis.values.len(),
    })
}

fn insert_auxiliary_block_metadata(
    metadata: &mut BTreeMap<String, Value>,
    bytes: &[u8],
    blocks: &[WdfBlock],
    spectrum_count: usize,
) -> Vec<WdfMapAnalysisDataRange> {
    if let Some(block) = blocks.iter().find(|block| block.name == "WHTL") {
        metadata.insert(
            "white_light_image".to_string(),
            white_light_image_metadata(bytes, block),
        );
    }

    let parsed_map_blocks = blocks
        .iter()
        .filter(|block| block.name == "MAP ")
        .map(|block| map_analysis_block(bytes, block))
        .collect::<Vec<_>>();
    if !parsed_map_blocks.is_empty() {
        let map_blocks = parsed_map_blocks
            .iter()
            .map(|block| block.metadata.clone())
            .collect::<Vec<_>>();
        metadata.insert(
            "map_analysis_block_count".to_string(),
            json!(map_blocks.len()),
        );
        metadata.insert("map_analysis_blocks".to_string(), Value::Array(map_blocks));
    }
    parsed_map_blocks
        .into_iter()
        .filter_map(|block| block.data_range)
        .filter(|range| range.values.len() == spectrum_count)
        .collect()
}

fn white_light_image_metadata(bytes: &[u8], block: &WdfBlock) -> Value {
    let payload = block_payload(bytes, block);
    let mut object = serde_json::Map::new();
    object.insert("block_uid".to_string(), json!(block.block_uid));
    object.insert("byte_len".to_string(), json!(payload.len()));
    object.insert("sha256".to_string(), json!(sha256_hex(payload)));
    if payload.starts_with(b"\xff\xd8") {
        object.insert("format".to_string(), json!("jpeg"));
        object.insert("mime_type".to_string(), json!("image/jpeg"));
        if let Some(jpeg) = jpeg_metadata(payload) {
            if let Some(width) = jpeg.width_px {
                object.insert("width_px".to_string(), json!(width));
            }
            if let Some(height) = jpeg.height_px {
                object.insert("height_px".to_string(), json!(height));
            }
            if let Some(precision) = jpeg.precision_bits {
                object.insert("precision_bits".to_string(), json!(precision));
            }
            if let Some(components) = jpeg.components {
                object.insert("components".to_string(), json!(components));
            }
            if let Some(unit) = jpeg.jfif_density_unit {
                object.insert("jfif_density_unit".to_string(), json!(unit));
            }
            if let Some(density) = jpeg.jfif_x_density {
                object.insert("jfif_x_density".to_string(), json!(density));
            }
            if let Some(density) = jpeg.jfif_y_density {
                object.insert("jfif_y_density".to_string(), json!(density));
            }
            if let Some(description) = jpeg.exif_description {
                object.insert("exif_description".to_string(), json!(description));
            }
            if let Some(make) = jpeg.exif_make {
                object.insert("exif_make".to_string(), json!(make));
            }
        }
    } else {
        object.insert("format".to_string(), json!("unknown"));
    }
    Value::Object(object)
}

struct WdfMapAnalysisBlock {
    metadata: Value,
    data_range: Option<WdfMapAnalysisDataRange>,
}

struct WdfMapAnalysisDataRange {
    block_uid: u32,
    label: Option<String>,
    values: Vec<f64>,
}

fn map_analysis_block(bytes: &[u8], block: &WdfBlock) -> WdfMapAnalysisBlock {
    let payload = block_payload(bytes, block);
    let mut object = serde_json::Map::new();
    object.insert("block_uid".to_string(), json!(block.block_uid));
    object.insert("byte_len".to_string(), json!(payload.len()));
    object.insert("sha256".to_string(), json!(sha256_hex(payload)));
    let mut pset_declared_len = None;
    if payload.starts_with(b"PSET") {
        object.insert("payload_kind".to_string(), json!("pset"));
        if payload.len() >= 8 {
            let declared_len =
                u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]) as usize;
            pset_declared_len = Some(declared_len);
            object.insert("pset_declared_len".to_string(), json!(declared_len));
        }
    } else {
        object.insert("payload_kind".to_string(), json!("unknown"));
    }
    let preview_strings = printable_ascii_strings(payload, 4, 8);
    let label = map_analysis_label(&preview_strings);
    if !preview_strings.is_empty() {
        object.insert("ascii_preview".to_string(), json!(preview_strings));
    }
    let data_range = pset_declared_len.and_then(|declared_len| {
        parse_map_analysis_data_range(payload, declared_len).map(|values| {
            object.insert("data_range_value_count".to_string(), json!(values.len()));
            object.insert(
                "data_range_encoding".to_string(),
                json!("f32le_tail_after_pset"),
            );
            object.insert("data_range_indexed_by".to_string(), json!("spectrum_index"));
            if let Some(first) = values.first() {
                object.insert("data_range_first".to_string(), json!(first));
            }
            if let Some(last) = values.last() {
                object.insert("data_range_last".to_string(), json!(last));
            }
            WdfMapAnalysisDataRange {
                block_uid: block.block_uid,
                label,
                values,
            }
        })
    });
    WdfMapAnalysisBlock {
        metadata: Value::Object(object),
        data_range,
    }
}

fn parse_map_analysis_data_range(payload: &[u8], pset_declared_len: usize) -> Option<Vec<f64>> {
    if !payload
        .windows(b"dataRange".len())
        .any(|window| window == b"dataRange")
    {
        return None;
    }
    let value_start = 8usize.checked_add(pset_declared_len)?.checked_add(8)?;
    let value_bytes = payload.get(value_start..)?;
    if value_bytes.is_empty() || value_bytes.len() % 4 != 0 {
        return None;
    }
    let value_count = value_bytes.len() / 4;
    let mut values = Vec::with_capacity(value_count);
    for chunk in value_bytes.chunks_exact(4) {
        let value = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as f64;
        if !value.is_finite() {
            return None;
        }
        values.push(value);
    }
    Some(values)
}

fn map_analysis_label(preview_strings: &[String]) -> Option<String> {
    preview_strings
        .iter()
        .find(|value| {
            value.as_str() != "PSET"
                && value.as_str() != "dataRange"
                && !value.starts_with("dataRange")
                && !value.contains('{')
                && value.contains(' ')
        })
        .cloned()
        .or_else(|| {
            preview_strings
                .iter()
                .find(|value| {
                    value.as_str() != "PSET"
                        && value.as_str() != "dataRange"
                        && !value.starts_with("dataRange")
                        && !value.contains('{')
                })
                .cloned()
        })
}

fn block_payload<'a>(bytes: &'a [u8], block: &WdfBlock) -> &'a [u8] {
    &bytes[block.payload_offset..block.payload_offset + block.payload_len]
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[derive(Default)]
struct JpegMetadata {
    width_px: Option<u16>,
    height_px: Option<u16>,
    precision_bits: Option<u8>,
    components: Option<u8>,
    jfif_density_unit: Option<u8>,
    jfif_x_density: Option<u16>,
    jfif_y_density: Option<u16>,
    exif_description: Option<String>,
    exif_make: Option<String>,
}

fn jpeg_metadata(bytes: &[u8]) -> Option<JpegMetadata> {
    if !bytes.starts_with(b"\xff\xd8") {
        return None;
    }
    let mut metadata = JpegMetadata::default();
    let mut offset = 2usize;
    while offset + 4 <= bytes.len() {
        if bytes[offset] != 0xff {
            offset += 1;
            continue;
        }
        while offset < bytes.len() && bytes[offset] == 0xff {
            offset += 1;
        }
        if offset >= bytes.len() {
            return None;
        }
        let marker = bytes[offset];
        offset += 1;
        if marker == 0xd9 || marker == 0xda {
            break;
        }
        if marker == 0xd8 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        let segment_len = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        if segment_len < 2 || offset + segment_len > bytes.len() {
            return None;
        }
        let payload = &bytes[offset + 2..offset + segment_len];
        if marker == 0xe0 && payload.starts_with(b"JFIF\0") && payload.len() >= 12 {
            metadata.jfif_density_unit = Some(payload[7]);
            metadata.jfif_x_density = Some(u16::from_be_bytes([payload[8], payload[9]]));
            metadata.jfif_y_density = Some(u16::from_be_bytes([payload[10], payload[11]]));
        } else if marker == 0xe1 && payload.starts_with(b"Exif\0\0") {
            parse_exif_tiff_ifd0(&payload[6..], &mut metadata);
        } else if is_jpeg_start_of_frame(marker) && segment_len >= 8 {
            metadata.precision_bits = Some(bytes[offset + 2]);
            let height = u16::from_be_bytes([bytes[offset + 3], bytes[offset + 4]]);
            let width = u16::from_be_bytes([bytes[offset + 5], bytes[offset + 6]]);
            metadata.width_px = Some(width);
            metadata.height_px = Some(height);
            metadata.components = Some(bytes[offset + 7]);
        }
        offset += segment_len;
    }
    Some(metadata)
}

fn parse_exif_tiff_ifd0(tiff: &[u8], metadata: &mut JpegMetadata) {
    if tiff.len() < 8 {
        return;
    }
    let little_endian = match &tiff[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return,
    };
    if tiff_u16(tiff, 2, little_endian) != Some(42) {
        return;
    }
    let Some(ifd_offset) =
        tiff_u32(tiff, 4, little_endian).and_then(|value| usize::try_from(value).ok())
    else {
        return;
    };
    let Some(entry_count) = tiff_u16(tiff, ifd_offset, little_endian).map(usize::from) else {
        return;
    };
    let entries_offset = ifd_offset + 2;
    for entry_index in 0..entry_count {
        let offset = entries_offset + entry_index * 12;
        if offset + 12 > tiff.len() {
            return;
        }
        let Some(tag) = tiff_u16(tiff, offset, little_endian) else {
            continue;
        };
        if tag != 0x010e && tag != 0x010f {
            continue;
        }
        let Some(field_type) = tiff_u16(tiff, offset + 2, little_endian) else {
            continue;
        };
        if field_type != 2 {
            continue;
        }
        let Some(count) =
            tiff_u32(tiff, offset + 4, little_endian).and_then(|value| usize::try_from(value).ok())
        else {
            continue;
        };
        let value = if count <= 4 {
            ascii_value(&tiff[offset + 8..offset + 8 + count.min(4)])
        } else {
            let Some(value_offset) = tiff_u32(tiff, offset + 8, little_endian)
                .and_then(|value| usize::try_from(value).ok())
            else {
                continue;
            };
            let Some(bytes) = tiff.get(value_offset..value_offset.saturating_add(count)) else {
                continue;
            };
            ascii_value(bytes)
        };
        if value.is_empty() {
            continue;
        }
        match tag {
            0x010e => metadata.exif_description = Some(value),
            0x010f => metadata.exif_make = Some(value),
            _ => {}
        }
    }
}

fn tiff_u16(bytes: &[u8], offset: usize, little_endian: bool) -> Option<u16> {
    let raw = [*bytes.get(offset)?, *bytes.get(offset + 1)?];
    Some(if little_endian {
        u16::from_le_bytes(raw)
    } else {
        u16::from_be_bytes(raw)
    })
}

fn tiff_u32(bytes: &[u8], offset: usize, little_endian: bool) -> Option<u32> {
    let raw = [
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
        *bytes.get(offset + 2)?,
        *bytes.get(offset + 3)?,
    ];
    Some(if little_endian {
        u32::from_le_bytes(raw)
    } else {
        u32::from_be_bytes(raw)
    })
}

fn ascii_value(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

fn is_jpeg_start_of_frame(marker: u8) -> bool {
    matches!(
        marker,
        0xc0 | 0xc1 | 0xc2 | 0xc3 | 0xc5 | 0xc6 | 0xc7 | 0xc9 | 0xca | 0xcb | 0xcd | 0xce | 0xcf
    )
}

fn printable_ascii_strings(bytes: &[u8], min_len: usize, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = Vec::new();
    for byte in bytes {
        if (0x20..=0x7e).contains(byte) {
            current.push(*byte);
        } else {
            push_ascii_preview(&mut out, &mut current, min_len, limit);
            if out.len() >= limit {
                return out;
            }
        }
    }
    push_ascii_preview(&mut out, &mut current, min_len, limit);
    out
}

fn push_ascii_preview(out: &mut Vec<String>, current: &mut Vec<u8>, min_len: usize, limit: usize) {
    if current.len() >= min_len && out.len() < limit {
        let value = String::from_utf8_lossy(current).trim().to_string();
        if !value.is_empty() && !out.contains(&value) {
            out.push(value);
        }
    }
    current.clear();
}

fn insert_record_navigation(
    metadata: &mut BTreeMap<String, Value>,
    navigation: &WdfNavigation,
    spectrum_index: usize,
) {
    for axis in &navigation.origin_axes {
        insert_origin_axis_value(metadata, axis, spectrum_index);
    }
    if let Some(map) = &navigation.map {
        insert_map_indices(metadata, map, spectrum_index);
    }
}

fn insert_record_map_analysis(
    metadata: &mut BTreeMap<String, Value>,
    ranges: &[WdfMapAnalysisDataRange],
    spectrum_index: usize,
) {
    let values = ranges
        .iter()
        .filter_map(|range| {
            range.values.get(spectrum_index).map(|value| {
                let mut object = serde_json::Map::new();
                object.insert("block_uid".to_string(), json!(range.block_uid));
                if let Some(label) = &range.label {
                    object.insert("label".to_string(), json!(label));
                }
                object.insert("value".to_string(), json!(value));
                object.insert("source".to_string(), json!("MAP dataRange"));
                Value::Object(object)
            })
        })
        .collect::<Vec<_>>();
    if !values.is_empty() {
        metadata.insert("map_analysis_values".to_string(), Value::Array(values));
    }
}

fn insert_origin_axis_value(
    metadata: &mut BTreeMap<String, Value>,
    axis: &WdfOriginAxis,
    spectrum_index: usize,
) {
    match axis.data_type_code {
        3 => {
            if let Some(value) = axis.values.float(spectrum_index) {
                insert_spatial(metadata, "x", value, &axis.unit);
            }
        }
        4 => {
            if let Some(value) = axis.values.float(spectrum_index) {
                insert_spatial(metadata, "y", value, &axis.unit);
            }
        }
        5 => {
            if let Some(value) = axis.values.float(spectrum_index) {
                insert_spatial(metadata, "z", value, &axis.unit);
            }
        }
        11 => {
            if let Some(value) = axis.values.u64(spectrum_index) {
                metadata.insert("time_filetime_100ns".to_string(), json!(value));
                if let Some(first) = axis.values.u64(0) {
                    metadata.insert(
                        "elapsed_time_seconds".to_string(),
                        json!(elapsed_filetime_seconds(value, first)),
                    );
                    metadata.insert("elapsed_time_unit".to_string(), json!("s"));
                }
            }
        }
        14 => {
            if let Some(value) = axis.values.float(spectrum_index) {
                metadata.insert("focus_track_z".to_string(), json!(value));
                metadata.insert("focus_track_z_unit".to_string(), json!(axis.unit.as_str()));
            }
        }
        18 => {
            if let Some(value) = axis.values.float(spectrum_index) {
                metadata.insert("elapsed_time_interval".to_string(), json!(value));
                metadata.insert(
                    "elapsed_time_interval_unit".to_string(),
                    json!(axis.unit.as_str()),
                );
            }
        }
        22 => {
            if let Some(value) = axis.values.float(spectrum_index) {
                metadata.insert("multiwell_spatial_x".to_string(), json!(value));
                metadata.insert(
                    "multiwell_spatial_x_unit".to_string(),
                    json!(axis.unit.as_str()),
                );
            }
        }
        23 => {
            if let Some(value) = axis.values.float(spectrum_index) {
                metadata.insert("multiwell_spatial_y".to_string(), json!(value));
                metadata.insert(
                    "multiwell_spatial_y_unit".to_string(),
                    json!(axis.unit.as_str()),
                );
            }
        }
        24 => {
            if let Some(value) = axis.values.float(spectrum_index) {
                metadata.insert("multiwell_location_index".to_string(), json!(value));
            }
        }
        27 => {
            if let Some(value) = axis.values.float(spectrum_index) {
                metadata.insert("exposure_time".to_string(), json!(value));
                metadata.insert("exposure_time_unit".to_string(), json!(axis.unit.as_str()));
            }
        }
        _ => {}
    }
}

fn insert_spatial(metadata: &mut BTreeMap<String, Value>, axis: &str, value: f64, unit: &str) {
    metadata.insert(format!("spatial_{axis}"), json!(value));
    metadata.insert(format!("spatial_{axis}_unit"), json!(unit));
}

fn insert_map_indices(metadata: &mut BTreeMap<String, Value>, map: &WdfMapInfo, index: usize) {
    let size_x = map.size_xyz[0] as usize;
    let size_y = map.size_xyz[1] as usize;
    if size_x == 0 || size_y == 0 {
        return;
    }

    let (map_x_index, map_y_index) = if map.map_type_code == 2 {
        (index / size_y, index % size_y)
    } else {
        (index % size_x, index / size_x)
    };
    metadata.insert("map_x_index".to_string(), json!(map_x_index));
    metadata.insert("map_y_index".to_string(), json!(map_y_index));

    if map.map_type_code == 128 {
        if let (Some(x), Some(y)) = (
            metadata.get("spatial_x").and_then(Value::as_f64),
            metadata.get("spatial_y").and_then(Value::as_f64),
        ) {
            let dx = x - map.offset_xyz[0];
            let dy = y - map.offset_xyz[1];
            metadata.insert(
                "spatial_distance".to_string(),
                json!((dx * dx + dy * dy).sqrt()),
            );
            let unit = metadata
                .get("spatial_x_unit")
                .and_then(Value::as_str)
                .or_else(|| metadata.get("spatial_y_unit").and_then(Value::as_str))
                .map(str::to_string);
            if let Some(unit) = unit {
                metadata.insert("spatial_distance_unit".to_string(), json!(unit));
            }
        }
    }
}

impl WdfOriginValues {
    fn len(&self) -> usize {
        match self {
            WdfOriginValues::Float(values) => values.len(),
            WdfOriginValues::U64(values) => values.len(),
        }
    }

    fn float(&self, index: usize) -> Option<f64> {
        match self {
            WdfOriginValues::Float(values) => values.get(index).copied(),
            WdfOriginValues::U64(_) => None,
        }
    }

    fn u64(&self, index: usize) -> Option<u64> {
        match self {
            WdfOriginValues::Float(_) => None,
            WdfOriginValues::U64(values) => values.get(index).copied(),
        }
    }
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
            block_uid: read_u32(bytes, offset + 4)?,
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

fn read_f64_values(bytes: &[u8], offset: usize, count: usize) -> Result<Vec<f64>> {
    let byte_len = count
        .checked_mul(8)
        .ok_or_else(|| Error::InvalidRecord("Renishaw WDF vector length overflow".to_string()))?;
    if offset + byte_len > bytes.len() {
        return Err(Error::InvalidRecord(
            "Renishaw WDF vector extends past end of file".to_string(),
        ));
    }
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        out.push(read_f64(bytes, offset + index * 8)?);
    }
    Ok(out)
}

fn read_u64_values(bytes: &[u8], offset: usize, count: usize) -> Result<Vec<u64>> {
    let byte_len = count
        .checked_mul(8)
        .ok_or_else(|| Error::InvalidRecord("Renishaw WDF vector length overflow".to_string()))?;
    if offset + byte_len > bytes.len() {
        return Err(Error::InvalidRecord(
            "Renishaw WDF vector extends past end of file".to_string(),
        ));
    }
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        out.push(read_u64(bytes, offset + index * 8)?);
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

fn wdf_unit_label(unit_code: u32) -> &'static str {
    match unit_code {
        0 => "",
        1 => "cm-1",
        2 | 3 => "nm",
        4 => "eV",
        5 => "um",
        6 => "counts",
        7 => "electrons",
        8 => "mm",
        9 => "m",
        10 => "K",
        11 => "Pa",
        12 => "s",
        13 => "ms",
        14 => "h",
        15 => "d",
        16 => "px",
        17 => "intensity",
        18 => "relative_intensity",
        19 => "deg",
        20 => "rad",
        21 => "degC",
        22 => "degF",
        23 => "K/min",
        24 => "windows-filetime-100ns",
        25 => "us",
        _ => "unknown",
    }
}

fn wdf_origin_data_type_label(data_type_code: u32) -> &'static str {
    match data_type_code {
        0 => "arbitrary",
        1 => "spectral",
        2 => "intensity",
        3 => "x",
        4 => "y",
        5 => "z",
        6 => "spatial_r",
        7 => "spatial_theta",
        8 => "spatial_phi",
        9 => "temperature",
        10 => "pressure",
        11 => "time",
        12 => "derivative",
        13 => "polarization",
        14 => "focus_track_z",
        15 => "temperature_ramp_rate",
        16 => "spectrum_data_checksum",
        17 => "bit_flags",
        18 => "elapsed_time_intervals",
        19 => "frequency",
        22 => "multiwell_spatial_x",
        23 => "multiwell_spatial_y",
        24 => "multiwell_location_index",
        25 => "multiwell_reference",
        26 => "end_marker",
        27 => "exposure_time",
        _ => "unknown",
    }
}

fn wdf_map_type_label(map_type_code: u32) -> &'static str {
    match map_type_code {
        0 => "unspecified",
        1 => "random_points",
        2 => "column_major",
        4 => "alternating",
        8 => "linefocus_mapping",
        16 => "inverted_rows",
        32 => "inverted_columns",
        64 => "surface_profile",
        128 => "xyline",
        _ => "unknown",
    }
}

fn elapsed_filetime_seconds(value: u64, first: u64) -> f64 {
    if value >= first {
        (value - first) as f64 / 10_000_000.0
    } else {
        -((first - value) as f64 / 10_000_000.0)
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

fn read_f64(bytes: &[u8], offset: usize) -> Result<f64> {
    let value = bytes.get(offset..offset + 8).ok_or_else(|| {
        Error::InvalidRecord("Renishaw WDF f64 read past end of file".to_string())
    })?;
    Ok(f64::from_le_bytes([
        value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
    ]))
}
