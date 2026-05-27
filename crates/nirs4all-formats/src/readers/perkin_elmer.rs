use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_formats_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralRecord,
};
use serde_json::json;

use crate::readers::util::{
    safe_signal_name, signal_type_from_label, single_signal_record, SingleSignalSpec,
};
use crate::Reader;

const PE_MAGIC: &[u8] = b"PEPE";
const DESCRIPTION_OFFSET: usize = 4;
const DESCRIPTION_LEN: usize = 40;
const ROOT_BLOCK_OFFSET: usize = 44;
const BLOCK_HEADER_LEN: usize = 6;

const TAG_F64_PAIR: u16 = 0x751d;
const TAG_F64: u16 = 0x751b;
const TAG_I32: u16 = 0x752b;
const TAG_STRING: u16 = 0x7523;
const TAG_F64_ARRAY: u16 = 0x7516;

pub struct PerkinElmerReader;

impl Reader for PerkinElmerReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::perkin_elmer"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        if !head.starts_with(PE_MAGIC) {
            return None;
        }
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext == "fsm" {
            return Some(FormatProbe::new(
                "perkin-elmer-fsm",
                self.name(),
                Confidence::Definite,
                "Perkin Elmer Spotlight imaging header; imaging is out of scope for v1",
            ));
        }
        Some(FormatProbe::new(
            "perkin-elmer-sp",
            self.name(),
            Confidence::Definite,
            "Perkin Elmer PEPE spectral dataset header",
        ))
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(&self, path: &Path, bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext == "fsm" {
            return Err(Error::InvalidRecord(
                "Perkin Elmer .fsm imaging files are recognized but out of scope for v1"
                    .to_string(),
            ));
        }
        let source = SourceFile::from_bytes(path, bytes, "primary");
        parse_perkin_elmer_sp(bytes, source, self.name())
    }
}

#[derive(Clone, Debug)]
struct Block {
    id: u16,
    offset: usize,
    payload_offset: usize,
    payload_len: usize,
}

struct SpPayload {
    first_x: f64,
    last_x: f64,
    signal_min: f64,
    signal_max: f64,
    step: f64,
    n_points: usize,
    axis_unit: String,
    signal_unit: Option<String>,
    signal_label: String,
    values: Vec<f64>,
}

fn parse_perkin_elmer_sp(
    bytes: &[u8],
    source: SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    if !bytes.starts_with(PE_MAGIC) {
        return Err(Error::InvalidRecord(
            "missing Perkin Elmer PEPE header".to_string(),
        ));
    }
    let blocks = parse_blocks(bytes)?;
    let payload = parse_sp_payload(bytes, &blocks)?;
    let axis_values = axis_values(
        payload.first_x,
        payload.last_x,
        payload.step,
        payload.n_points,
    );
    let axis_kind = axis_kind_from_unit(&payload.axis_unit);
    let signal_type = signal_type_from_label(&payload.signal_label);
    let signal_type =
        if signal_type == SignalType::Unknown && payload.signal_unit.as_deref() == Some("A") {
            SignalType::Absorbance
        } else {
            signal_type
        };
    let signal_name = safe_signal_name(&payload.signal_label, "signal");

    let mut metadata = metadata_from_blocks(bytes, &blocks);
    metadata.insert("description".to_string(), json!(description(bytes)));
    metadata.insert("data_min".to_string(), json!(payload.signal_min));
    metadata.insert("data_max".to_string(), json!(payload.signal_max));
    metadata.insert("x_step".to_string(), json!(payload.step));
    metadata.insert("point_count".to_string(), json!(payload.n_points));

    let record = single_signal_record(
        "perkin-elmer-sp",
        reader,
        source,
        SingleSignalSpec {
            axis_values,
            axis_unit: payload.axis_unit,
            axis_kind,
            values: payload.values,
            signal_name,
            signal_type,
            signal_unit: payload.signal_unit,
            role: safe_signal_name(&payload.signal_label, "signal"),
        },
        BTreeMap::new(),
        metadata,
        vec!["perkin_elmer_reverse_engineered_blocks".to_string()],
    )?;
    Ok(vec![record])
}

fn parse_sp_payload(bytes: &[u8], blocks: &[Block]) -> Result<SpPayload> {
    let (first_x, last_x) = f64_pair(bytes, find_typed_block(bytes, blocks, 35698, TAG_F64_PAIR)?)?;
    let (signal_min, signal_max) =
        f64_pair(bytes, find_typed_block(bytes, blocks, 35699, TAG_F64_PAIR)?)?;
    let step = f64_value(bytes, find_typed_block(bytes, blocks, 35700, TAG_F64)?)?;
    let n_points = i32_value(bytes, find_typed_block(bytes, blocks, 35701, TAG_I32)?)? as usize;
    let axis_unit = string_value(bytes, find_typed_block(bytes, blocks, 35703, TAG_STRING)?)?;
    let signal_unit = string_value(bytes, find_typed_block(bytes, blocks, 35704, TAG_STRING)?)
        .ok()
        .filter(|value| !value.is_empty());
    let signal_label = blocks
        .iter()
        .filter(|block| block.id == 35699 && payload_tag(bytes, block) == Some(TAG_STRING))
        .filter_map(|block| string_value(bytes, block).ok())
        .find(|value| !value.is_empty())
        .unwrap_or_else(|| signal_unit.clone().unwrap_or_else(|| "signal".to_string()));
    let values = f64_array(
        bytes,
        find_typed_block(bytes, blocks, 35708, TAG_F64_ARRAY)?,
        n_points,
    )?;

    Ok(SpPayload {
        first_x,
        last_x,
        signal_min,
        signal_max,
        step,
        n_points,
        axis_unit,
        signal_unit,
        signal_label,
        values,
    })
}

fn parse_blocks(bytes: &[u8]) -> Result<Vec<Block>> {
    if bytes.len() < ROOT_BLOCK_OFFSET + BLOCK_HEADER_LEN {
        return Err(Error::InvalidRecord(
            "Perkin Elmer file is too short for a root block".to_string(),
        ));
    }
    let root_id = read_u16(bytes, ROOT_BLOCK_OFFSET)?;
    let root_len = read_i32(bytes, ROOT_BLOCK_OFFSET + 2)?;
    if root_id != 120 || root_len < 0 {
        return Err(Error::InvalidRecord(
            "Perkin Elmer root block is missing or invalid".to_string(),
        ));
    }
    let root_end = ROOT_BLOCK_OFFSET + BLOCK_HEADER_LEN + root_len as usize;
    if root_end != bytes.len() {
        return Err(Error::InvalidRecord(format!(
            "Perkin Elmer root block ends at {root_end}, expected {}",
            bytes.len()
        )));
    }

    let mut out = Vec::new();
    parse_block_sequence(bytes, ROOT_BLOCK_OFFSET, bytes.len(), &mut out)?;
    Ok(out)
}

fn parse_block_sequence(
    bytes: &[u8],
    start: usize,
    end: usize,
    out: &mut Vec<Block>,
) -> Result<()> {
    let mut offset = start;
    while offset + BLOCK_HEADER_LEN <= end {
        let id = read_u16(bytes, offset)?;
        let payload_len = read_i32(bytes, offset + 2)?;
        if payload_len < 0 {
            return Err(Error::InvalidRecord(
                "Perkin Elmer block has a negative payload length".to_string(),
            ));
        }
        let payload_len = payload_len as usize;
        let payload_offset = offset + BLOCK_HEADER_LEN;
        let next = payload_offset + payload_len;
        if next > end {
            return Err(Error::InvalidRecord(format!(
                "Perkin Elmer block {id} at {offset} extends past its parent"
            )));
        }
        let block = Block {
            id,
            offset,
            payload_offset,
            payload_len,
        };
        out.push(block.clone());
        if is_container_block(id) && payload_is_block_sequence(bytes, payload_offset, next) {
            parse_block_sequence(bytes, payload_offset, next, out)?;
        }
        offset = next;
    }
    if offset != end {
        return Err(Error::InvalidRecord(
            "Perkin Elmer block sequence ended on a partial header".to_string(),
        ));
    }
    Ok(())
}

fn is_container_block(id: u16) -> bool {
    matches!(id, 120 | 121 | 122 | 123 | 124 | 35703 | 35705 | 35711)
}

fn payload_is_block_sequence(bytes: &[u8], start: usize, end: usize) -> bool {
    let mut offset = start;
    let mut saw_block = false;
    while offset + BLOCK_HEADER_LEN <= end {
        let Ok(payload_len) = read_i32(bytes, offset + 2) else {
            return false;
        };
        if payload_len < 0 {
            return false;
        }
        let next = offset + BLOCK_HEADER_LEN + payload_len as usize;
        if next > end {
            return false;
        }
        saw_block = true;
        offset = next;
    }
    saw_block && offset == end
}

fn metadata_from_blocks(bytes: &[u8], blocks: &[Block]) -> BTreeMap<String, serde_json::Value> {
    let mut metadata = BTreeMap::new();
    insert_string(bytes, blocks, &mut metadata, "source_path", 35709);
    insert_string(bytes, blocks, &mut metadata, "sample_id", 35713);
    insert_string(bytes, blocks, &mut metadata, "instrument", 35837);
    insert_string(bytes, blocks, &mut metadata, "instrument_serial", 35838);
    insert_string(bytes, blocks, &mut metadata, "software", 35839);
    insert_string(bytes, blocks, &mut metadata, "detector", 35841);
    insert_string(bytes, blocks, &mut metadata, "source_type", 35842);
    insert_string(bytes, blocks, &mut metadata, "beam_splitter", 35843);
    insert_string(bytes, blocks, &mut metadata, "apodization", 35845);
    insert_string(bytes, blocks, &mut metadata, "measurement_mode", 35846);
    insert_string(bytes, blocks, &mut metadata, "processing_mode", 35847);
    insert_string(bytes, blocks, &mut metadata, "ordinate_mode", 35849);
    insert_string(bytes, blocks, &mut metadata, "accessory", 35854);
    insert_string(bytes, blocks, &mut metadata, "ratio_mode", 35870);

    if let Some(date) = blocks
        .iter()
        .filter(|block| block.id == 35700 && payload_tag(bytes, block) == Some(TAG_STRING))
        .filter_map(|block| string_value(bytes, block).ok())
        .rfind(|value| !value.is_empty())
    {
        metadata.insert("scan_date".to_string(), json!(date));
    }
    if let Some(image_name) = blocks
        .iter()
        .filter(|block| block.id == 35702 && payload_tag(bytes, block) == Some(TAG_STRING))
        .filter_map(|block| string_value(bytes, block).ok())
        .find(|value| value.contains("Micrometers"))
    {
        metadata.insert("image_name".to_string(), json!(image_name));
    }
    metadata
}

fn insert_string(
    bytes: &[u8],
    blocks: &[Block],
    metadata: &mut BTreeMap<String, serde_json::Value>,
    key: &str,
    id: u16,
) {
    if let Some(value) = blocks
        .iter()
        .filter(|block| block.id == id && payload_tag(bytes, block) == Some(TAG_STRING))
        .filter_map(|block| string_value(bytes, block).ok())
        .find(|value| !value.is_empty())
    {
        metadata.insert(key.to_string(), json!(value));
    }
}

fn find_typed_block<'a>(bytes: &[u8], blocks: &'a [Block], id: u16, tag: u16) -> Result<&'a Block> {
    blocks
        .iter()
        .find(|block| block.id == id && payload_tag(bytes, block) == Some(tag))
        .ok_or_else(|| {
            Error::InvalidRecord(format!(
                "Perkin Elmer block {id} with tag 0x{tag:04x} is missing"
            ))
        })
}

fn payload_tag(bytes: &[u8], block: &Block) -> Option<u16> {
    (block.payload_len >= 2)
        .then(|| read_u16(bytes, block.payload_offset).ok())
        .flatten()
}

fn f64_pair(bytes: &[u8], block: &Block) -> Result<(f64, f64)> {
    require_tag(bytes, block, TAG_F64_PAIR)?;
    if block.payload_len < 18 {
        return Err(Error::InvalidRecord(
            "Perkin Elmer f64-pair block is truncated".to_string(),
        ));
    }
    Ok((
        read_f64(bytes, block.payload_offset + 2)?,
        read_f64(bytes, block.payload_offset + 10)?,
    ))
}

fn f64_value(bytes: &[u8], block: &Block) -> Result<f64> {
    require_tag(bytes, block, TAG_F64)?;
    if block.payload_len < 10 {
        return Err(Error::InvalidRecord(
            "Perkin Elmer f64 block is truncated".to_string(),
        ));
    }
    read_f64(bytes, block.payload_offset + 2)
}

fn i32_value(bytes: &[u8], block: &Block) -> Result<i32> {
    require_tag(bytes, block, TAG_I32)?;
    if block.payload_len < 6 {
        return Err(Error::InvalidRecord(
            "Perkin Elmer i32 block is truncated".to_string(),
        ));
    }
    read_i32(bytes, block.payload_offset + 2)
}

fn string_value(bytes: &[u8], block: &Block) -> Result<String> {
    require_tag(bytes, block, TAG_STRING)?;
    if block.payload_len < 4 {
        return Err(Error::InvalidRecord(
            "Perkin Elmer string block is truncated".to_string(),
        ));
    }
    let len = read_u16(bytes, block.payload_offset + 2)? as usize;
    let start = block.payload_offset + 4;
    let end = start + len;
    if end > block.payload_offset + block.payload_len {
        return Err(Error::InvalidRecord(
            "Perkin Elmer string length exceeds block payload".to_string(),
        ));
    }
    Ok(decode_text(&bytes[start..end]).trim().to_string())
}

fn f64_array(bytes: &[u8], block: &Block, expected_len: usize) -> Result<Vec<f64>> {
    require_tag(bytes, block, TAG_F64_ARRAY)?;
    if block.payload_len < 6 {
        return Err(Error::InvalidRecord(
            "Perkin Elmer f64-array block is truncated".to_string(),
        ));
    }
    let byte_len = read_i32(bytes, block.payload_offset + 2)?;
    if byte_len < 0 {
        return Err(Error::InvalidRecord(
            "Perkin Elmer f64-array block has negative byte length".to_string(),
        ));
    }
    let byte_len = byte_len as usize;
    if byte_len != expected_len * 8 {
        return Err(Error::InvalidRecord(format!(
            "Perkin Elmer f64-array has {byte_len} bytes, expected {}",
            expected_len * 8
        )));
    }
    let start = block.payload_offset + 6;
    let end = start + byte_len;
    if end > block.payload_offset + block.payload_len {
        return Err(Error::InvalidRecord(
            "Perkin Elmer f64-array data exceeds block payload".to_string(),
        ));
    }
    let values = (0..expected_len)
        .map(|index| read_f64(bytes, start + index * 8))
        .collect::<Result<Vec<_>>>()?;
    Ok(values)
}

fn require_tag(bytes: &[u8], block: &Block, expected: u16) -> Result<()> {
    let actual = payload_tag(bytes, block).ok_or_else(|| {
        Error::InvalidRecord(format!("Perkin Elmer block {} has no type tag", block.id))
    })?;
    if actual != expected {
        return Err(Error::InvalidRecord(format!(
            "Perkin Elmer block {} at {} has tag 0x{actual:04x}, expected 0x{expected:04x}",
            block.id, block.offset
        )));
    }
    Ok(())
}

fn axis_values(first: f64, last: f64, step: f64, len: usize) -> Vec<f64> {
    if len <= 1 {
        return vec![first];
    }
    if step.is_finite() && step != 0.0 {
        return (0..len).map(|index| first + step * index as f64).collect();
    }
    let inferred_step = (last - first) / (len - 1) as f64;
    (0..len)
        .map(|index| first + inferred_step * index as f64)
        .collect()
}

fn axis_kind_from_unit(unit: &str) -> AxisKind {
    match unit.trim().to_ascii_lowercase().as_str() {
        "cm-1" | "cm^-1" | "1/cm" => AxisKind::Wavenumber,
        "nm" | "um" | "µm" => AxisKind::Wavelength,
        _ => AxisKind::Index,
    }
}

fn description(bytes: &[u8]) -> String {
    let end = (DESCRIPTION_OFFSET + DESCRIPTION_LEN).min(bytes.len());
    let raw = bytes[DESCRIPTION_OFFSET..end]
        .split(|byte| *byte == 0)
        .next()
        .unwrap_or_default();
    decode_text(raw).trim().to_string()
}

fn decode_text(bytes: &[u8]) -> String {
    std::str::from_utf8(bytes)
        .map(|value| value.to_string())
        .unwrap_or_else(|_| bytes.iter().map(|byte| *byte as char).collect())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| Error::InvalidRecord("truncated Perkin Elmer u16 field".to_string()))?;
    Ok(u16::from_le_bytes(value.try_into().expect("slice len")))
}

fn read_i32(bytes: &[u8], offset: usize) -> Result<i32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| Error::InvalidRecord("truncated Perkin Elmer i32 field".to_string()))?;
    Ok(i32::from_le_bytes(value.try_into().expect("slice len")))
}

fn read_f64(bytes: &[u8], offset: usize) -> Result<f64> {
    let value = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| Error::InvalidRecord("truncated Perkin Elmer f64 field".to_string()))?;
    Ok(f64::from_le_bytes(value.try_into().expect("slice len")))
}
