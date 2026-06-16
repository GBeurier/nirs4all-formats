use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_formats_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralRecord,
};
use serde_json::{json, Value};

use crate::readers::util::{safe_signal_name, single_signal_record, SingleSignalSpec};
use crate::Reader;

// --- File header (fixed little-endian fields) ---
const VERSION_OFF: usize = 0x00; // u16: 2 = .cal (with constituents), 1 = .nir (spectra only)
const SAMPLE_COUNT_OFF: usize = 0x02; // u16
const POINT_COUNT_OFF: usize = 0x06; // u16
const CONSTITUENT_COUNT_OFF: usize = 0x08; // u16
const MASTER_NO_OFF: usize = 0x59; // ASCII master/serial number
const MODEL_OFF: usize = 0x82; // ASCII instrument model
const SEGMENT_COUNT_OFF: usize = 0xA0; // u16
const SEGMENT_POINTS_OFF: usize = 0xA2; // u16 per segment, stride 2
const SEGMENT_START_OFF: usize = 0xCC; // f32 per segment, stride 4
const SEGMENT_STEP_OFF: usize = 0xE8; // f32 per segment, stride 4
const SEGMENT_END_OFF: usize = 0x104; // f32 per segment, stride 4
const CONSTITUENT_NAME_OFF: usize = 0x180; // 16-byte NUL-padded names, 32 slots
const CONSTITUENT_NAME_LEN: usize = 16;

// --- Per-sample record layout (fixed stride from 0x380) ---
const DATA_START: usize = 0x380; // first sample block (constant for .cal and .nir)
const RECORD_HEADER_LEN: usize = 0x100; // metadata header, then the spectrum
const SAMPLE_NUMBER_LEN: usize = 18; // NUL-padded sample-number field before product code
const PRODUCT_CODE_OFF: usize = SAMPLE_NUMBER_LEN; // i32 within the metadata header
const TIMESTAMP_OFF: usize = 214; // u32 unix timestamp within the metadata header
const SPECTRUM_GAP: usize = 24; // compact-layout zero bytes between spectrum and values
const CONSTITUENT_REGION_LEN: usize = 128; // 32 f32 slots reserved per record
const MAX_SEGMENTS: usize = 7; // segment arrays are 7 slots wide before they overlap
const MAX_CONSTITUENTS: usize = 32; // constituent value region capacity

const VERSION_CAL: u16 = 2;
const VERSION_NIR: u16 = 1;

const SIG_ISISCAN: &[u8] = b"ISIscan";
const SIG_NIRSYSTEMS: &[u8] = b"NIRSystems";
// FOSS DS-series bench analysers (DS2500, DS3 F, ...) emit the same native
// container but carry no ISIscan/NIRSystems identity string; they are pinned by
// the instrument model prefix at `MODEL_OFF` (e.g. "NIRS DS2500", "NIRS DS3 F").
const SIG_DS_MODEL: &[u8] = b"NIRS DS";

pub struct FossWinisiReader;

impl Reader for FossWinisiReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::foss_winisi"
    }

    fn sniff(&self, head: &[u8], _path: &Path) -> Option<FormatProbe> {
        let version = read_u16(head, VERSION_OFF).ok()?;
        if version != VERSION_CAL && version != VERSION_NIR {
            return None;
        }
        // Distinguish from BUCHI NIRCal (which also uses the `.nir` extension)
        // purely by binary signature: ISIscan/WinISI files carry an ASCII
        // identity string in the header, never the "NIRCAL Project File" magic.
        // FOSS DS-series benches omit that string but pin themselves with a
        // "NIRS DS..." instrument model at MODEL_OFF instead.
        let window = &head[..head.len().min(256)];
        let ds_series = is_ds_series(head);
        if !ds_series && !contains(window, SIG_ISISCAN) && !contains(window, SIG_NIRSYSTEMS) {
            return None;
        }
        Some(FormatProbe::new(
            resolve_format_key(ds_series, version),
            self.name(),
            Confidence::Definite,
            if ds_series {
                "FOSS DS-series native header (NIRS DS model)"
            } else {
                "FOSS NIRSystems / ISIscan / WinISI native header"
            },
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
        let source = SourceFile::from_bytes(path, bytes, "primary");
        parse_foss_winisi(bytes, source, self.name())
    }
}

struct Header {
    version: u16,
    format_key: &'static str,
    sample_count: usize,
    point_count: usize,
    constituent_count: usize,
    axis: Vec<f64>,
    constituent_names: Vec<String>,
    master_no: String,
    instrument_model: String,
}

fn parse_foss_winisi(
    bytes: &[u8],
    source: SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    let header = parse_header(bytes)?;
    let stride = detect_record_stride(bytes, &header)?;

    let mut records = Vec::with_capacity(header.sample_count);
    for index in 0..header.sample_count {
        let base = DATA_START
            .checked_add(stride.checked_mul(index).ok_or_else(overflow)?)
            .ok_or_else(overflow)?;
        records.push(parse_record(
            bytes, &header, base, stride, index, &source, reader,
        )?);
    }
    Ok(records)
}

fn parse_header(bytes: &[u8]) -> Result<Header> {
    let version = read_u16(bytes, VERSION_OFF)?;
    if version != VERSION_CAL && version != VERSION_NIR {
        return Err(Error::InvalidRecord(format!(
            "unsupported FOSS WinISI version word {version}"
        )));
    }
    let sample_count = read_u16(bytes, SAMPLE_COUNT_OFF)? as usize;
    if sample_count == 0 {
        return Err(Error::InvalidRecord(
            "FOSS WinISI file declares zero samples".to_string(),
        ));
    }
    let point_count = read_u16(bytes, POINT_COUNT_OFF)? as usize;
    if point_count == 0 {
        return Err(Error::InvalidRecord(
            "FOSS WinISI file declares zero data points".to_string(),
        ));
    }
    let constituent_count = read_u16(bytes, CONSTITUENT_COUNT_OFF)? as usize;
    if constituent_count > MAX_CONSTITUENTS {
        return Err(Error::InvalidRecord(format!(
            "FOSS WinISI declares {constituent_count} constituents, exceeding the {MAX_CONSTITUENTS} reserved slots"
        )));
    }
    if version == VERSION_NIR && constituent_count != 0 {
        return Err(Error::InvalidRecord(
            "FOSS WinISI .nir spectra file declares constituents".to_string(),
        ));
    }

    let axis = parse_axis(bytes, point_count)?;
    let constituent_names = parse_constituent_names(bytes, constituent_count);
    let master_no = ascii_field(bytes, MASTER_NO_OFF, 16);
    let instrument_model = ascii_field(bytes, MODEL_OFF, 24);
    let format_key = resolve_format_key(is_ds_series(bytes), version);

    Ok(Header {
        version,
        format_key,
        sample_count,
        point_count,
        constituent_count,
        axis,
        constituent_names,
        master_no,
        instrument_model,
    })
}

fn parse_axis(bytes: &[u8], point_count: usize) -> Result<Vec<f64>> {
    let segment_count = read_u16(bytes, SEGMENT_COUNT_OFF)? as usize;
    if segment_count == 0 || segment_count > MAX_SEGMENTS {
        return Err(Error::InvalidRecord(format!(
            "FOSS WinISI segment count {segment_count} is out of range"
        )));
    }

    let mut axis = Vec::with_capacity(point_count);
    let mut total = 0usize;
    for segment in 0..segment_count {
        let points = read_u16(bytes, SEGMENT_POINTS_OFF + segment * 2)? as usize;
        let start = read_f32(bytes, SEGMENT_START_OFF + segment * 4)? as f64;
        let step = read_f32(bytes, SEGMENT_STEP_OFF + segment * 4)? as f64;
        let end = read_f32(bytes, SEGMENT_END_OFF + segment * 4)? as f64;
        if !start.is_finite() || !step.is_finite() || !end.is_finite() {
            return Err(Error::InvalidRecord(
                "FOSS WinISI segment table contains non-finite values".to_string(),
            ));
        }
        for point in 0..points {
            axis.push(start + step * point as f64);
        }
        total += points;
    }

    if total != point_count {
        return Err(Error::InvalidRecord(format!(
            "FOSS WinISI segment points sum to {total}, expected {point_count}"
        )));
    }
    Ok(axis)
}

fn parse_constituent_names(bytes: &[u8], constituent_count: usize) -> Vec<String> {
    let mut names = Vec::with_capacity(constituent_count);
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for index in 0..constituent_count {
        let offset = CONSTITUENT_NAME_OFF + index * CONSTITUENT_NAME_LEN;
        let raw = ascii_field(bytes, offset, CONSTITUENT_NAME_LEN);
        let name = if raw.is_empty() {
            format!("constituent_{}", index + 1)
        } else {
            raw
        };
        let count = seen.entry(name.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            names.push(format!("{name}_{count}"));
        } else {
            names.push(name);
        }
    }
    names
}

fn parse_record(
    bytes: &[u8],
    header: &Header,
    base: usize,
    stride: usize,
    index: usize,
    source: &SourceFile,
    reader: &str,
) -> Result<SpectralRecord> {
    let spectrum_off = base + RECORD_HEADER_LEN;
    let values = read_f32_vec(bytes, spectrum_off, header.point_count)?;

    let constituent_off = base
        .checked_add(stride)
        .and_then(|end| end.checked_sub(CONSTITUENT_REGION_LEN))
        .ok_or_else(overflow)?;
    let targets = parse_targets(bytes, header, constituent_off)?;

    let sample_number = ascii_field(bytes, base, SAMPLE_NUMBER_LEN);
    let product_code = read_i32(bytes, base + PRODUCT_CODE_OFF).ok();
    let timestamp = read_u32(bytes, base + TIMESTAMP_OFF).ok();

    let mut metadata = BTreeMap::new();
    metadata.insert("record_index".to_string(), json!(index));
    if !sample_number.is_empty() {
        metadata.insert("sample_number".to_string(), json!(sample_number));
    }
    if let Some(code) = product_code {
        metadata.insert("product_code".to_string(), json!(code));
    }
    if let Some(ts) = timestamp.filter(|value| *value != 0) {
        metadata.insert("acquired_unix_time".to_string(), json!(ts));
        if let Some(formatted) = format_unix_utc(ts) {
            metadata.insert("acquired_utc".to_string(), json!(formatted));
        }
    }
    if !header.master_no.is_empty() {
        metadata.insert("master_number".to_string(), json!(header.master_no));
    }
    if !header.instrument_model.is_empty() {
        metadata.insert(
            "instrument_model".to_string(),
            json!(header.instrument_model),
        );
    }
    metadata.insert("file_kind".to_string(), json!(header.format_key));
    metadata.insert(
        "constituent_count".to_string(),
        json!(header.constituent_count),
    );
    metadata.insert("version_word".to_string(), json!(header.version));

    single_signal_record(
        header.format_key,
        reader,
        source.clone(),
        SingleSignalSpec {
            axis_values: header.axis.clone(),
            axis_unit: "nm".to_string(),
            axis_kind: AxisKind::Wavelength,
            values,
            signal_name: "absorbance".to_string(),
            signal_type: SignalType::Absorbance,
            signal_unit: None,
            role: "absorbance".to_string(),
        },
        targets,
        metadata,
        vec!["foss_winisi_reverse_engineered_blocks".to_string()],
    )
}

fn parse_targets(
    bytes: &[u8],
    header: &Header,
    constituent_off: usize,
) -> Result<BTreeMap<String, Value>> {
    let mut targets = BTreeMap::new();
    for (index, name) in header.constituent_names.iter().enumerate() {
        let value = read_f32(bytes, constituent_off + index * 4)? as f64;
        let entry = if value.is_finite() {
            json!(value)
        } else {
            Value::Null
        };
        targets.insert(
            safe_signal_name(name, &format!("constituent_{}", index + 1)),
            entry,
        );
    }
    Ok(targets)
}

fn record_stride(point_count: usize) -> usize {
    RECORD_HEADER_LEN + point_count * 4 + SPECTRUM_GAP + CONSTITUENT_REGION_LEN
}

fn aligned_record_stride(point_count: usize) -> usize {
    align_up(RECORD_HEADER_LEN + point_count * 4, CONSTITUENT_REGION_LEN) + CONSTITUENT_REGION_LEN
}

fn align_up(value: usize, alignment: usize) -> usize {
    debug_assert!(alignment.is_power_of_two());
    (value + alignment - 1) & !(alignment - 1)
}

fn detect_record_stride(bytes: &[u8], header: &Header) -> Result<usize> {
    let compact = record_stride(header.point_count);
    let aligned = aligned_record_stride(header.point_count);
    let minimum_payload = RECORD_HEADER_LEN
        .checked_add(header.point_count.checked_mul(4).ok_or_else(overflow)?)
        .ok_or_else(overflow)?;

    let mut candidates = vec![compact, aligned];
    if let Some(data_len) = bytes.len().checked_sub(DATA_START) {
        if data_len % header.sample_count == 0 {
            candidates.push(data_len / header.sample_count);
        }
    }
    candidates.sort_unstable();
    candidates.dedup();

    let mut best = None;
    for stride in candidates {
        let minimum_stride = minimum_payload
            .checked_add(CONSTITUENT_REGION_LEN)
            .ok_or_else(overflow)?;
        if stride < minimum_stride || !stride_fits(bytes.len(), header.sample_count, stride)? {
            continue;
        }
        let score = stride_score(bytes, header.sample_count, stride);
        match best {
            None => best = Some((score, stride)),
            Some((best_score, best_stride))
                if score > best_score || (score == best_score && stride < best_stride) =>
            {
                best = Some((score, stride));
            }
            _ => {}
        }
    }

    if let Some((score, stride)) = best {
        if score > 0 {
            return Ok(stride);
        }
    }
    if stride_fits(bytes.len(), header.sample_count, compact)? {
        return Ok(compact);
    }
    Err(Error::InvalidRecord(
        "FOSS WinISI sample blocks extend past end of file".to_string(),
    ))
}

fn stride_fits(file_len: usize, sample_count: usize, stride: usize) -> Result<bool> {
    let last_base = DATA_START
        .checked_add(
            stride
                .checked_mul(sample_count.saturating_sub(1))
                .ok_or_else(overflow)?,
        )
        .ok_or_else(overflow)?;
    Ok(last_base
        .checked_add(stride)
        .is_some_and(|end| end <= file_len))
}

fn stride_score(bytes: &[u8], sample_count: usize, stride: usize) -> usize {
    (0..sample_count)
        .map(|index| {
            let Some(base) = DATA_START.checked_add(stride.saturating_mul(index)) else {
                return 0;
            };
            record_start_score(bytes, base)
        })
        .sum()
}

fn record_start_score(bytes: &[u8], base: usize) -> usize {
    let Some(first) = bytes.get(base).copied() else {
        return 0;
    };
    let mut score = 0;
    if first.is_ascii_alphanumeric() && ascii_field_is_plausible(bytes, base, 32) {
        score += 2;
    }
    if read_i32(bytes, base + PRODUCT_CODE_OFF)
        .ok()
        .is_some_and(|value| (-1_000_000..=1_000_000).contains(&value))
    {
        score += 1;
    }
    if read_u32(bytes, base + TIMESTAMP_OFF)
        .ok()
        .is_some_and(|value| value == 0 || (946_684_800..=4_102_444_800).contains(&value))
    {
        score += 1;
    }
    score
}

fn ascii_field_is_plausible(bytes: &[u8], offset: usize, max_len: usize) -> bool {
    let end = offset.saturating_add(max_len).min(bytes.len());
    let Some(slice) = bytes.get(offset..end) else {
        return false;
    };
    let field = slice.split(|byte| *byte == 0).next().unwrap_or(slice);
    !field.is_empty()
        && field
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

/// FOSS DS-series benches (DS2500, DS3 F, ...) write the same container as the
/// ISIscan/WinISI files but carry a "NIRS DS..." model string at `MODEL_OFF`
/// instead of the ISIscan/NIRSystems identity. The prefix never collides with
/// the "NIRSystems 6500" model of the WinISI files.
fn is_ds_series(bytes: &[u8]) -> bool {
    bytes
        .get(MODEL_OFF..MODEL_OFF + SIG_DS_MODEL.len())
        .is_some_and(|slice| slice == SIG_DS_MODEL)
}

fn resolve_format_key(ds_series: bool, version: u16) -> &'static str {
    match (ds_series, version == VERSION_CAL) {
        (true, true) => "foss-ds-cal",
        (true, false) => "foss-ds-nir",
        (false, true) => "foss-winisi-cal",
        (false, false) => "foss-winisi-nir",
    }
}

fn overflow() -> Error {
    Error::InvalidRecord("FOSS WinISI record offset overflows usize".to_string())
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn ascii_field(bytes: &[u8], offset: usize, max_len: usize) -> String {
    let end = offset.saturating_add(max_len).min(bytes.len());
    let Some(slice) = bytes.get(offset..end) else {
        return String::new();
    };
    let trimmed = slice.split(|byte| *byte == 0).next().unwrap_or(slice);
    decode_text(trimmed).trim().to_string()
}

fn decode_text(bytes: &[u8]) -> String {
    std::str::from_utf8(bytes)
        .map(|value| value.to_string())
        .unwrap_or_else(|_| bytes.iter().map(|byte| *byte as char).collect())
}

fn read_f32_vec(bytes: &[u8], offset: usize, len: usize) -> Result<Vec<f64>> {
    let byte_len = len.checked_mul(4).ok_or_else(overflow)?;
    let end = offset.checked_add(byte_len).ok_or_else(overflow)?;
    if end > bytes.len() {
        return Err(Error::InvalidRecord(
            "FOSS WinISI spectrum extends past end of file".to_string(),
        ));
    }
    (0..len)
        .map(|index| Ok(read_f32(bytes, offset + index * 4)? as f64))
        .collect()
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| Error::InvalidRecord("truncated FOSS WinISI u16 field".to_string()))?;
    Ok(u16::from_le_bytes(value.try_into().expect("slice len")))
}

fn read_i32(bytes: &[u8], offset: usize) -> Result<i32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| Error::InvalidRecord("truncated FOSS WinISI i32 field".to_string()))?;
    Ok(i32::from_le_bytes(value.try_into().expect("slice len")))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| Error::InvalidRecord("truncated FOSS WinISI u32 field".to_string()))?;
    Ok(u32::from_le_bytes(value.try_into().expect("slice len")))
}

fn read_f32(bytes: &[u8], offset: usize) -> Result<f32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| Error::InvalidRecord("truncated FOSS WinISI f32 field".to_string()))?;
    Ok(f32::from_le_bytes(value.try_into().expect("slice len")))
}

/// Format a unix timestamp as `YYYY-MM-DD HH:MM:SS` UTC without pulling in a
/// date-time dependency, using Howard Hinnant's civil-from-days algorithm.
fn format_unix_utc(timestamp: u32) -> Option<String> {
    let secs = timestamp as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let hour = rem / 3600;
    let minute = (rem % 3600) / 60;
    let second = rem % 60;

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };

    Some(format!(
        "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_known_timestamp() {
        // 1209998195 == 2008-05-05 14:36:35 UTC (yamtot sample 1).
        assert_eq!(
            format_unix_utc(1_209_998_195).as_deref(),
            Some("2008-05-05 14:36:35")
        );
    }

    #[test]
    fn record_stride_matches_real_layout() {
        // The common 1050-point layout lands on the same stride with both
        // compact padding and 128-byte alignment.
        assert_eq!(record_stride(1050), 4608);
        assert_eq!(aligned_record_stride(1050), 4608);
        // DS3 F real files align the post-spectrum region to 128 bytes:
        // 256 + 2800 + 16 + 128 == 3200. Older synthetic fixtures use the
        // compact 24-byte gap and therefore remain at 3208.
        assert_eq!(record_stride(700), 3208);
        assert_eq!(aligned_record_stride(700), 3200);
    }

    #[test]
    fn detects_aligned_ds3f_stride_from_record_headers() {
        let bytes = stride_fixture(700, 3, aligned_record_stride(700));
        let header = parse_header(&bytes).expect("header");

        assert_eq!(detect_record_stride(&bytes, &header).unwrap(), 3200);
    }

    #[test]
    fn keeps_compact_stride_from_record_headers() {
        let bytes = stride_fixture(50, 3, record_stride(50));
        let header = parse_header(&bytes).expect("header");

        assert_eq!(detect_record_stride(&bytes, &header).unwrap(), 608);
    }

    #[test]
    fn ds_series_model_is_recognised() {
        let mut head = vec![0u8; MODEL_OFF + 16];
        head[MODEL_OFF..MODEL_OFF + 11].copy_from_slice(b"NIRS DS2500");
        assert!(is_ds_series(&head));

        head[MODEL_OFF..MODEL_OFF + 11].copy_from_slice(b"NIRSystems6");
        assert!(!is_ds_series(&head));
    }

    #[test]
    fn format_key_separates_family_and_kind() {
        assert_eq!(resolve_format_key(true, VERSION_NIR), "foss-ds-nir");
        assert_eq!(resolve_format_key(true, VERSION_CAL), "foss-ds-cal");
        assert_eq!(resolve_format_key(false, VERSION_NIR), "foss-winisi-nir");
        assert_eq!(resolve_format_key(false, VERSION_CAL), "foss-winisi-cal");
    }

    #[test]
    fn truncated_header_is_rejected() {
        assert!(matches!(
            parse_header(&[0x02, 0x00, 0x01]),
            Err(Error::InvalidRecord(_))
        ));
    }

    fn stride_fixture(point_count: usize, sample_count: usize, stride: usize) -> Vec<u8> {
        let len = DATA_START + stride * sample_count;
        let mut bytes = vec![0u8; len];
        write_u16(&mut bytes, VERSION_OFF, VERSION_NIR);
        write_u16(&mut bytes, SAMPLE_COUNT_OFF, sample_count as u16);
        write_u16(&mut bytes, POINT_COUNT_OFF, point_count as u16);
        write_u16(&mut bytes, CONSTITUENT_COUNT_OFF, 0);
        bytes[MODEL_OFF..MODEL_OFF + SIG_DS_MODEL.len()].copy_from_slice(SIG_DS_MODEL);
        write_u16(&mut bytes, SEGMENT_COUNT_OFF, 1);
        write_u16(&mut bytes, SEGMENT_POINTS_OFF, point_count as u16);
        write_f32(&mut bytes, SEGMENT_START_OFF, 1100.0);
        write_f32(&mut bytes, SEGMENT_STEP_OFF, 2.0);
        write_f32(
            &mut bytes,
            SEGMENT_END_OFF,
            1100.0 + 2.0 * (point_count.saturating_sub(1)) as f32,
        );

        for index in 0..sample_count {
            let base = DATA_START + stride * index;
            let sample = format!("SYN{index:04}-00");
            bytes[base..base + sample.len()].copy_from_slice(sample.as_bytes());
            bytes[base + PRODUCT_CODE_OFF..base + PRODUCT_CODE_OFF + 4]
                .copy_from_slice(&(index as i32).to_le_bytes());
            bytes[base + TIMESTAMP_OFF..base + TIMESTAMP_OFF + 4]
                .copy_from_slice(&(1_700_000_000u32 + index as u32).to_le_bytes());
        }
        bytes
    }

    fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_f32(bytes: &mut [u8], offset: usize, value: f32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}
