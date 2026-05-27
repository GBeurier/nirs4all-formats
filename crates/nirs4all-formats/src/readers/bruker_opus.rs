use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use nirs4all_formats_core::{
    AxisKind, Confidence, FormatProbe, Result, SignalType, SourceFile, SpectralArray, SpectralAxis,
    SpectralRecord,
};
use serde_json::{json, Value};

use crate::readers::util::{record_from_signals, safe_signal_name};
use crate::Reader;

const OPUS_MAGIC_NEW: &[u8; 4] = b"\n\n\xfe\xfe";
const OPUS_MAGIC_OLD: &[u8; 4] = b"\n\n\x1a\x1a";
const OPUS_HEADER_LEN: usize = 24;
const OPUS_DIRECTORY_ENTRY_LEN: usize = 12;

pub struct BrukerOpusReader;

impl Reader for BrukerOpusReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::bruker_opus"
    }

    fn sniff(&self, head: &[u8], _path: &Path) -> Option<FormatProbe> {
        if head.starts_with(OPUS_MAGIC_NEW) && looks_like_opus_header(head) {
            Some(FormatProbe::new(
                "bruker-opus",
                self.name(),
                Confidence::Definite,
                "Bruker OPUS binary header magic",
            ))
        } else if head.starts_with(OPUS_MAGIC_OLD) {
            Some(FormatProbe::new(
                "bruker-opus",
                self.name(),
                Confidence::Possible,
                "older Bruker OPUS magic recognized but not validated yet",
            ))
        } else {
            None
        }
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| nirs4all_formats_core::Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(&self, path: &Path, bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
        let source = SourceFile::from_bytes(path, bytes, "primary");
        parse_opus_bytes(bytes, path, source, self.name())
    }
}

#[derive(Clone, Debug)]
struct OpusHeader {
    version: f64,
    directory_start: usize,
    max_blocks: usize,
    num_blocks: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpusBlockType([u8; 6]);

impl OpusBlockType {
    fn from_i32(value: i32) -> Self {
        let value = value as u32;
        Self([
            (value & 0b11) as u8,
            ((value >> 2) & 0b11) as u8,
            ((value >> 4) & 0b11_1111) as u8,
            ((value >> 10) & 0b111_1111) as u8,
            ((value >> 17) & 0b11) as u8,
            ((value >> 19) & 0b111) as u8,
        ])
    }

    fn as_array(&self) -> [u8; 6] {
        self.0
    }

    fn is_file_log(&self) -> bool {
        self.0 == [0, 0, 0, 0, 0, 5]
    }

    fn is_data_status(&self) -> bool {
        self.0[2] == 1
    }

    fn is_parameter(&self) -> bool {
        self.0[2] > 0 || self.0 == [0, 0, 0, 0, 0, 1]
    }

    fn is_data(&self) -> bool {
        self.0[2] == 0 && !matches!(self.0[3], 0 | 13) && !matches!(self.0[5], 2 | 5)
    }

    fn is_compact_data(&self) -> bool {
        self.is_data() && self.0[5] == 4
    }

    fn status_matches_data(&self, data: &Self) -> bool {
        self.is_data_status()
            && self.0[0] == data.0[0]
            && self.0[1] == data.0[1]
            && self.0[3] == data.0[3]
            && self.0[4] == data.0[4]
            && self.0[5] == data.0[5]
    }

    fn label(&self) -> String {
        let labels = [
            code0_label(self.0[0]),
            code1_label(self.0[1]),
            code2_label(self.0[2]),
            code3_label(self.0[3] % 32),
            code4_label(self.0[4]),
            code5_label(self.0[5]),
        ]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
        if labels.is_empty() {
            "Undefined".to_string()
        } else {
            labels.join(" ")
        }
    }
}

#[derive(Clone, Debug)]
struct OpusBlock {
    index: usize,
    block_type: OpusBlockType,
    size: usize,
    start: usize,
}

#[derive(Clone, Debug)]
enum ParamValue {
    Int(i32),
    Float(f64),
    Text(String),
}

impl ParamValue {
    fn as_i32(&self) -> Option<i32> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(*value),
            Self::Int(value) => Some(*value as f64),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }

    fn to_json(&self) -> Value {
        match self {
            Self::Int(value) => json!(value),
            Self::Float(value) => json!(value),
            Self::Text(value) => json!(value),
        }
    }
}

type ParamMap = BTreeMap<String, ParamValue>;

#[derive(Clone, Debug)]
struct DataCandidate {
    signal_name: String,
    signal_type: SignalType,
    role: String,
    axis: Vec<f64>,
    axis_unit: String,
    axis_kind: AxisKind,
    values: Vec<f64>,
    params: ParamMap,
    data_block: OpusBlock,
    status_block: OpusBlock,
}

fn parse_opus_bytes(
    bytes: &[u8],
    path: &Path,
    source: SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    if !bytes.starts_with(OPUS_MAGIC_NEW) {
        return Err(nirs4all_formats_core::Error::InvalidRecord(
            "unsupported or missing Bruker OPUS magic".to_string(),
        ));
    }
    let header = parse_header(bytes)?;
    let blocks = parse_directory(bytes, &header)?;
    let mut warnings = Vec::new();
    let params_by_block = parse_parameter_blocks(bytes, &blocks, &mut warnings);
    let history = parse_history_blocks(bytes, &blocks);
    let candidates = parse_data_candidates(bytes, &blocks, &params_by_block, &mut warnings)?;
    if candidates.is_empty() {
        return Err(nirs4all_formats_core::Error::InvalidRecord(format!(
            "{}: no supported 1D OPUS data blocks found",
            path.display()
        )));
    }

    let mut signals = BTreeMap::new();
    let mut signal_params = BTreeMap::new();
    let mut used_names = BTreeMap::<String, usize>::new();
    let mut dominant = SignalType::Unknown;
    for candidate in candidates {
        if dominant == SignalType::Unknown || candidate.signal_type == SignalType::Absorbance {
            dominant = candidate.signal_type.clone();
        }
        let signal_name = unique_signal_name(candidate.signal_name, &mut used_names);
        let axis = SpectralAxis::new(candidate.axis, candidate.axis_unit, candidate.axis_kind)?;
        let signal = SpectralArray::new(
            axis,
            candidate.values,
            vec!["x".to_string()],
            candidate.signal_type,
            None,
            candidate.role,
            format!("block_{}", candidate.data_block.index),
        )?;
        signal_params.insert(
            signal_name.clone(),
            json!({
                "data_block_index": candidate.data_block.index,
                "data_block_type": candidate.data_block.block_type.as_array(),
                "data_block_label": candidate.data_block.block_type.label(),
                "status_block_index": candidate.status_block.index,
                "status_block_type": candidate.status_block.block_type.as_array(),
                "parameters": params_to_json(&candidate.params),
            }),
        );
        signals.insert(signal_name, signal);
    }

    let mut metadata = BTreeMap::new();
    metadata.insert(
        "bruker_opus".to_string(),
        json!({
            "version": header.version,
            "directory_start": header.directory_start,
            "max_blocks": header.max_blocks,
            "num_blocks": header.num_blocks,
            "blocks": blocks.iter().map(block_metadata).collect::<Vec<_>>(),
        }),
    );
    metadata.insert(
        "bruker_opus_signal_params".to_string(),
        json!(signal_params),
    );
    let global_params = global_params_json(&blocks, &params_by_block);
    if !global_params.is_empty() {
        metadata.insert("bruker_opus_params".to_string(), json!(global_params));
    }
    if !history.is_empty() {
        metadata.insert("bruker_opus_history".to_string(), json!(history));
    }

    let record = record_from_signals(
        "bruker-opus",
        reader,
        source,
        signals,
        dominant,
        metadata,
        warnings,
    )?;
    Ok(vec![record])
}

fn parse_header(bytes: &[u8]) -> Result<OpusHeader> {
    require_len(bytes, OPUS_HEADER_LEN, "Bruker OPUS header")?;
    let directory_start = nonnegative_i32_as_usize(bytes, 12, "directory_start")?;
    let max_blocks = nonnegative_i32_as_usize(bytes, 16, "max_blocks")?;
    let num_blocks = nonnegative_i32_as_usize(bytes, 20, "num_blocks")?;
    if directory_start >= bytes.len() {
        return Err(nirs4all_formats_core::Error::InvalidRecord(
            "Bruker OPUS directory starts beyond end of file".to_string(),
        ));
    }
    if max_blocks == 0 || max_blocks > 100_000 {
        return Err(nirs4all_formats_core::Error::InvalidRecord(format!(
            "invalid Bruker OPUS max_blocks {max_blocks}"
        )));
    }
    Ok(OpusHeader {
        version: le_f64(bytes, 4)?,
        directory_start,
        max_blocks,
        num_blocks,
    })
}

fn parse_directory(bytes: &[u8], header: &OpusHeader) -> Result<Vec<OpusBlock>> {
    let byte_len = header
        .max_blocks
        .checked_mul(OPUS_DIRECTORY_ENTRY_LEN)
        .ok_or_else(|| {
            nirs4all_formats_core::Error::InvalidRecord(
                "Bruker OPUS directory byte length overflow".to_string(),
            )
        })?;
    require_len(
        bytes,
        header.directory_start + byte_len,
        "Bruker OPUS directory",
    )?;
    let mut blocks = Vec::new();
    for index in 0..header.max_blocks {
        let offset = header.directory_start + index * OPUS_DIRECTORY_ENTRY_LEN;
        let type_int = le_i32(bytes, offset)?;
        let size_words = le_i32(bytes, offset + 4)?;
        let start = le_i32(bytes, offset + 8)?;
        if start <= 0 {
            break;
        }
        if size_words < 0 {
            return Err(nirs4all_formats_core::Error::InvalidRecord(format!(
                "negative Bruker OPUS block size at directory index {index}"
            )));
        }
        let size = (size_words as usize).checked_mul(4).ok_or_else(|| {
            nirs4all_formats_core::Error::InvalidRecord(
                "Bruker OPUS block byte length overflow".to_string(),
            )
        })?;
        let start = start as usize;
        require_len(bytes, start + size, "Bruker OPUS block")?;
        blocks.push(OpusBlock {
            index,
            block_type: OpusBlockType::from_i32(type_int),
            size,
            start,
        });
    }
    Ok(blocks)
}

fn parse_parameter_blocks(
    bytes: &[u8],
    blocks: &[OpusBlock],
    warnings: &mut Vec<String>,
) -> BTreeMap<usize, ParamMap> {
    let mut out = BTreeMap::new();
    for block in blocks
        .iter()
        .filter(|block| block.block_type.is_parameter())
    {
        match parse_params(block_bytes(bytes, block)) {
            Ok(params) => {
                out.insert(block.index, params);
            }
            Err(error) => warnings.push(format!(
                "opus_parameter_block_{}_not_decoded: {error}",
                block.index
            )),
        }
    }
    out
}

fn parse_data_candidates(
    bytes: &[u8],
    blocks: &[OpusBlock],
    params_by_block: &BTreeMap<usize, ParamMap>,
    warnings: &mut Vec<String>,
) -> Result<Vec<DataCandidate>> {
    let statuses = blocks
        .iter()
        .filter(|block| block.block_type.is_data_status())
        .collect::<Vec<_>>();
    let mut used_statuses = BTreeSet::new();
    let mut out = Vec::new();

    let mut data_blocks = blocks
        .iter()
        .filter(|block| block.block_type.is_data())
        .collect::<Vec<_>>();
    data_blocks.sort_by_key(|block| std::cmp::Reverse(block.start));

    for data_block in data_blocks {
        let matches = statuses
            .iter()
            .copied()
            .filter(|status| {
                status
                    .block_type
                    .status_matches_data(&data_block.block_type)
            })
            .collect::<Vec<_>>();
        if matches.is_empty() {
            warnings.push(format!(
                "opus_data_block_{}_has_no_matching_status_block",
                data_block.index
            ));
            continue;
        }
        let Some((status_block, params, values)) = select_status_match(
            bytes,
            data_block,
            &matches,
            params_by_block,
            &mut used_statuses,
        )?
        else {
            warnings.push(format!(
                "opus_data_block_{}_could_not_be_matched_unambiguously",
                data_block.index
            ));
            continue;
        };
        let npt = params_i32(params, "NPT")
            .unwrap_or(values.len() as i32)
            .max(0) as usize;
        if npt == 0 {
            warnings.push(format!(
                "opus_data_block_{}_declares_zero_points",
                data_block.index
            ));
            continue;
        }
        let fxv = params_f64(params, "FXV").unwrap_or(0.0);
        let lxv = params_f64(params, "LXV").unwrap_or((npt.saturating_sub(1)) as f64);
        let dxu = params
            .get("DXU")
            .and_then(ParamValue::as_str)
            .unwrap_or("PNT");
        let (axis_kind, axis_unit) = axis_kind_and_unit(dxu);
        let values = values.into_iter().take(npt).collect::<Vec<_>>();
        if values.len() != npt {
            warnings.push(format!(
                "opus_data_block_{}_shorter_than_declared_npt",
                data_block.index
            ));
            continue;
        }
        let key = signal_key(&data_block.block_type);
        let signal_type = signal_type_from_block(&data_block.block_type);
        out.push(DataCandidate {
            signal_name: key,
            signal_type,
            role: data_block.block_type.label(),
            axis: linspace(fxv, lxv, npt),
            axis_unit,
            axis_kind,
            values,
            params: params.clone(),
            data_block: data_block.clone(),
            status_block: status_block.clone(),
        });
        used_statuses.insert(status_block.index);
    }
    Ok(out)
}

fn select_status_match<'a>(
    bytes: &[u8],
    data_block: &OpusBlock,
    matches: &[&'a OpusBlock],
    params_by_block: &'a BTreeMap<usize, ParamMap>,
    used_statuses: &mut BTreeSet<usize>,
) -> Result<Option<(&'a OpusBlock, &'a ParamMap, Vec<f64>)>> {
    let mut parsed = Vec::new();
    for status in matches {
        let Some(params) = params_by_block.get(&status.index) else {
            continue;
        };
        let values = parse_scaled_data(bytes, data_block, params)?;
        parsed.push((*status, params, values));
    }
    if parsed.is_empty() {
        return Ok(None);
    }

    let matched_by_minmax = parsed
        .iter()
        .filter(|(_, params, values)| minmax_matches(params, values))
        .collect::<Vec<_>>();
    if matched_by_minmax.len() == 1 {
        let (status, params, values) = matched_by_minmax[0];
        return Ok(Some((status, params, values.clone())));
    }

    if let Some((status, params, values)) = parsed
        .iter()
        .find(|(status, _, _)| !used_statuses.contains(&status.index))
    {
        return Ok(Some((status, params, values.clone())));
    }

    let (status, params, values) = parsed.remove(0);
    Ok(Some((status, params, values)))
}

fn parse_scaled_data(bytes: &[u8], block: &OpusBlock, params: &ParamMap) -> Result<Vec<f64>> {
    let dpf = params_i32(params, "DPF").unwrap_or(1);
    let csf = params_f64(params, "CSF").unwrap_or(1.0);
    let mut values = match dpf {
        2 => parse_i32_array(block_bytes(bytes, block))?,
        _ => parse_f32_array(block_bytes(bytes, block))?,
    };
    if block.block_type.is_compact_data() {
        let npt = params_i32(params, "NPT").unwrap_or(values.len() as i32);
        if npt > 0 && (npt as usize) < values.len() {
            values = values[values.len() - npt as usize..].to_vec();
        }
    }
    values.iter_mut().for_each(|value| *value *= csf);
    Ok(values)
}

fn minmax_matches(params: &ParamMap, values: &[f64]) -> bool {
    let npt = params_i32(params, "NPT")
        .unwrap_or(values.len() as i32)
        .max(0) as usize;
    if npt == 0 || values.len() < npt {
        return false;
    }
    let Some(expected_min) = params_f64(params, "MNY") else {
        return true;
    };
    let Some(expected_max) = params_f64(params, "MXY") else {
        return true;
    };
    let (actual_min, actual_max) = values[..npt]
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
            (min.min(*value), max.max(*value))
        });
    close_enough(actual_min, expected_min) && close_enough(actual_max, expected_max)
}

fn close_enough(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-5_f64.max(expected.abs() * 1e-5)
}

fn parse_params(bytes: &[u8]) -> Result<ParamMap> {
    let mut loc = 0;
    let mut out = ParamMap::new();
    while loc + 8 <= bytes.len() {
        let key = std::str::from_utf8(&bytes[loc..loc + 3])
            .unwrap_or_default()
            .to_ascii_uppercase();
        if key == "END" {
            break;
        }
        let dtype = le_i16(bytes, loc + 4)?;
        let size_words = le_i16(bytes, loc + 6)?;
        if size_words <= 0 {
            break;
        }
        let value_bytes = (size_words as usize).checked_mul(2).ok_or_else(|| {
            nirs4all_formats_core::Error::InvalidRecord(
                "Bruker OPUS parameter byte length overflow".to_string(),
            )
        })?;
        let next = loc + 8 + value_bytes;
        require_len(bytes, next, "Bruker OPUS parameter value")?;
        let value = match dtype {
            0 => ParamValue::Int(le_i32(bytes, loc + 8)?),
            1 => ParamValue::Float(le_f64(bytes, loc + 8)?),
            _ => ParamValue::Text(clean_latin1(&bytes[loc + 8..loc + 8 + value_bytes])),
        };
        out.insert(key, value);
        loc = next;
    }
    Ok(out)
}

fn parse_history_blocks(bytes: &[u8], blocks: &[OpusBlock]) -> Vec<String> {
    blocks
        .iter()
        .filter(|block| block.block_type.is_file_log())
        .flat_map(|block| {
            block_bytes(bytes, block)
                .split(|byte| *byte == 0)
                .map(clean_latin1)
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn block_bytes<'a>(bytes: &'a [u8], block: &OpusBlock) -> &'a [u8] {
    &bytes[block.start..block.start + block.size]
}

fn parse_f32_array(bytes: &[u8]) -> Result<Vec<f64>> {
    if !bytes.len().is_multiple_of(4) {
        return Err(nirs4all_formats_core::Error::InvalidRecord(
            "Bruker OPUS float32 data block length is not divisible by 4".to_string(),
        ));
    }
    (0..bytes.len() / 4)
        .map(|index| le_f32(bytes, index * 4).map(f64::from))
        .collect()
}

fn parse_i32_array(bytes: &[u8]) -> Result<Vec<f64>> {
    if !bytes.len().is_multiple_of(4) {
        return Err(nirs4all_formats_core::Error::InvalidRecord(
            "Bruker OPUS int32 data block length is not divisible by 4".to_string(),
        ));
    }
    (0..bytes.len() / 4)
        .map(|index| le_i32(bytes, index * 4).map(|value| value as f64))
        .collect()
}

fn block_metadata(block: &OpusBlock) -> Value {
    json!({
        "index": block.index,
        "type": block.block_type.as_array(),
        "label": block.block_type.label(),
        "size": block.size,
        "start": block.start,
    })
}

fn global_params_json(
    blocks: &[OpusBlock],
    params_by_block: &BTreeMap<usize, ParamMap>,
) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for block in blocks
        .iter()
        .filter(|block| block.block_type.is_parameter() && !block.block_type.is_data_status())
    {
        if let Some(params) = params_by_block.get(&block.index) {
            out.insert(
                format!(
                    "block_{}_{}",
                    block.index,
                    safe_signal_name(&block.block_type.label(), "params")
                ),
                json!({
                    "type": block.block_type.as_array(),
                    "label": block.block_type.label(),
                    "parameters": params_to_json(params),
                }),
            );
        }
    }
    out
}

fn params_to_json(params: &ParamMap) -> BTreeMap<String, Value> {
    params
        .iter()
        .map(|(key, value)| (key.clone(), value.to_json()))
        .collect()
}

fn params_i32(params: &ParamMap, key: &str) -> Option<i32> {
    params.get(key).and_then(ParamValue::as_i32)
}

fn params_f64(params: &ParamMap, key: &str) -> Option<f64> {
    params.get(key).and_then(ParamValue::as_f64)
}

fn unique_signal_name(name: String, used: &mut BTreeMap<String, usize>) -> String {
    let count = used.entry(name.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        name
    } else {
        format!("{name}_{count}")
    }
}

fn signal_key(block_type: &OpusBlockType) -> String {
    let channels = block_type.0[3] / 32 + 1;
    let suffix = if channels > 1 {
        format!("_{channels}ch")
    } else {
        String::new()
    };
    let role_prefix = match block_type.0[1] {
        1 => "sample_",
        2 => "reference_",
        _ => "",
    };
    let base = match block_type.0[3] % 32 {
        1 => format!("{role_prefix}spectrum"),
        2 => format!("{role_prefix}interferogram"),
        3 => format!("{role_prefix}phase"),
        4 => "absorbance".to_string(),
        5 => "transmittance".to_string(),
        6 => "kubelka_munk".to_string(),
        7 => "trace".to_string(),
        10 => "raman".to_string(),
        11 => "emission".to_string(),
        12 => "reflectance".to_string(),
        14 => "power".to_string(),
        15 => "log_reflectance".to_string(),
        16 => "atr".to_string(),
        17 => "photoacoustic".to_string(),
        22 => "match".to_string(),
        other => format!("signal_{other}"),
    };
    format!("{base}{suffix}")
}

fn signal_type_from_block(block_type: &OpusBlockType) -> SignalType {
    match block_type.0[3] % 32 {
        1 => SignalType::SingleBeam,
        2 => SignalType::Interferogram,
        4 | 19 => SignalType::Absorbance,
        5 | 18 => SignalType::Transmittance,
        6 => SignalType::KubelkaMunk,
        12 => SignalType::Reflectance,
        22 => SignalType::Preprocessed,
        _ => SignalType::Unknown,
    }
}

fn axis_kind_and_unit(dxu: &str) -> (AxisKind, String) {
    match dxu.to_ascii_uppercase().as_str() {
        "WN" => (AxisKind::Wavenumber, "cm-1".to_string()),
        "MI" => (AxisKind::Wavelength, "um".to_string()),
        "PNT" => (AxisKind::Index, "index".to_string()),
        "MIN" => (AxisKind::Time, "min".to_string()),
        "LGW" => (AxisKind::Wavenumber, "log_cm-1".to_string()),
        _ => (AxisKind::Index, "index".to_string()),
    }
}

fn linspace(first: f64, last: f64, count: usize) -> Vec<f64> {
    if count <= 1 {
        return vec![first];
    }
    let step = (last - first) / (count - 1) as f64;
    (0..count)
        .map(|index| first + step * index as f64)
        .collect()
}

fn clean_latin1(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    bytes[..end]
        .iter()
        .map(|byte| char::from(*byte))
        .collect::<String>()
        .trim()
        .to_string()
}

fn code0_label(code: u8) -> &'static str {
    match code {
        1 => "Real Part of Complex Data",
        2 => "Imaginary Part of Complex Data",
        _ => "",
    }
}

fn code1_label(code: u8) -> &'static str {
    match code {
        1 => "Sample",
        2 => "Reference",
        _ => "",
    }
}

fn code2_label(code: u8) -> &'static str {
    match code {
        1 => "Data Parameters",
        2 => "Instrument Parameters",
        3 => "Acquisition Parameters",
        4 => "Fourier Transform Parameters",
        5 => "Plot and Display Parameters",
        6 => "Optical Parameters",
        7 => "GC Parameters",
        8 => "Library Search Parameters",
        9 => "Communication Parameters",
        10 => "Sample Origin Parameters",
        11 => "Lab and Process Parameters",
        _ => "",
    }
}

fn code3_label(code: u8) -> &'static str {
    match code {
        1 => "Spectrum",
        2 => "Interferogram",
        3 => "Phase",
        4 => "Absorbance",
        5 => "Transmittance",
        6 => "Kubelka-Munk",
        7 => "Trace",
        8 => "GC Interferogram Series",
        9 => "GC Spectrum Series",
        10 => "Raman",
        11 => "Emission",
        12 => "Reflectance",
        13 => "Directory",
        14 => "Power",
        15 => "Log Reflectance",
        16 => "ATR",
        17 => "Photoacoustic",
        18 => "Arithmetic Transmittance",
        19 => "Arithmetic Absorbance",
        22 => "Match",
        _ => "",
    }
}

fn code4_label(code: u8) -> &'static str {
    match code {
        1 => "First Derivative",
        2 => "Second Derivative",
        3 => "N-th Derivative",
        _ => "",
    }
}

fn code5_label(code: u8) -> &'static str {
    match code {
        1 => "Compound Information",
        2 => "Series",
        3 => "Molecular Structure",
        4 => "Compact",
        5 => "History/Report",
        _ => "",
    }
}

fn looks_like_opus_header(head: &[u8]) -> bool {
    if head.len() < OPUS_HEADER_LEN {
        return false;
    }
    let directory_start = le_i32(head, 12).ok();
    let max_blocks = le_i32(head, 16).ok();
    let num_blocks = le_i32(head, 20).ok();
    matches!((directory_start, max_blocks, num_blocks), (Some(start), Some(max), Some(num))
        if start >= OPUS_HEADER_LEN as i32
            && (1..100_000).contains(&max)
            && (0..=max).contains(&num))
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

fn nonnegative_i32_as_usize(bytes: &[u8], offset: usize, name: &str) -> Result<usize> {
    let value = le_i32(bytes, offset)?;
    if value < 0 {
        return Err(nirs4all_formats_core::Error::InvalidRecord(format!(
            "negative OPUS {name} value {value}"
        )));
    }
    Ok(value as usize)
}

fn le_i16(bytes: &[u8], offset: usize) -> Result<i16> {
    let data = bytes.get(offset..offset + 2).ok_or_else(|| {
        nirs4all_formats_core::Error::InvalidRecord("OPUS i16 field truncated".to_string())
    })?;
    Ok(i16::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn le_i32(bytes: &[u8], offset: usize) -> Result<i32> {
    let data = bytes.get(offset..offset + 4).ok_or_else(|| {
        nirs4all_formats_core::Error::InvalidRecord("OPUS i32 field truncated".to_string())
    })?;
    Ok(i32::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn le_f32(bytes: &[u8], offset: usize) -> Result<f32> {
    let data = bytes.get(offset..offset + 4).ok_or_else(|| {
        nirs4all_formats_core::Error::InvalidRecord("OPUS f32 field truncated".to_string())
    })?;
    Ok(f32::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn le_f64(bytes: &[u8], offset: usize) -> Result<f64> {
    let data = bytes.get(offset..offset + 8).ok_or_else(|| {
        nirs4all_formats_core::Error::InvalidRecord("OPUS f64 field truncated".to_string())
    })?;
    Ok(f64::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_minutes_axis_is_typed_as_time() {
        assert_eq!(
            axis_kind_and_unit("MIN"),
            (AxisKind::Time, "min".to_string())
        );
        assert_eq!(
            axis_kind_and_unit("PNT"),
            (AxisKind::Index, "index".to_string())
        );
    }
}
