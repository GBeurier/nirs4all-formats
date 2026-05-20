use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{AxisKind, Confidence, FormatProbe, Result, SignalType, SourceFile};
use serde_json::{json, Value};

use crate::readers::util::{
    safe_signal_name, signal_type_from_label, single_signal_record, SingleSignalSpec,
};
use crate::Reader;

const NEW_HEADER_LEN: usize = 512;
const OLD_HEADER_LEN: usize = 256;
const SUB_HEADER_LEN: usize = 32;
const LOG_HEADER_LEN: usize = 64;

pub struct GalacticSpcReader;

impl Reader for GalacticSpcReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::galactic_spc"
    }

    fn sniff(&self, head: &[u8], _path: &Path) -> Option<FormatProbe> {
        let version = *head.get(1)?;
        match version {
            0x4b if looks_like_new_lsb_spc(head) => Some(FormatProbe::new(
                "galactic-spc",
                self.name(),
                Confidence::Definite,
                "Galactic/Thermo GRAMS SPC new little-endian header",
            )),
            0x4d if looks_like_old_spc(head) => Some(FormatProbe::new(
                "galactic-spc",
                self.name(),
                Confidence::Likely,
                "Galactic/Thermo GRAMS SPC old little-endian header",
            )),
            0x4c => Some(FormatProbe::new(
                "galactic-spc",
                self.name(),
                Confidence::Possible,
                "Galactic/Thermo GRAMS SPC new big-endian header is recognized but not implemented",
            )),
            _ => None,
        }
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| nirs4all_io_core::Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let source = SourceFile::from_bytes(path, &bytes, "primary");
        match bytes.get(1).copied() {
            Some(0x4b) => parse_new_lsb_spc(&bytes, path, source, self.name()),
            Some(0x4d) => parse_old_lsb_spc(&bytes, path, source, self.name()),
            Some(0x4c) => Err(nirs4all_io_core::Error::InvalidRecord(
                "Galactic SPC new big-endian files are recognized but not implemented yet"
                    .to_string(),
            )),
            _ => Err(nirs4all_io_core::Error::InvalidRecord(
                "missing Galactic SPC file-version byte".to_string(),
            )),
        }
    }
}

#[derive(Clone, Debug)]
struct SpcFlags {
    raw: u8,
    tsprec: bool,
    tcgram: bool,
    tmulti: bool,
    trandm: bool,
    tordrd: bool,
    talabs: bool,
    txyxys: bool,
    txvals: bool,
}

impl SpcFlags {
    fn new(raw: u8) -> Self {
        Self {
            raw,
            tsprec: raw & 0x01 != 0,
            tcgram: raw & 0x02 != 0,
            tmulti: raw & 0x04 != 0,
            trandm: raw & 0x08 != 0,
            tordrd: raw & 0x10 != 0,
            talabs: raw & 0x20 != 0,
            txyxys: raw & 0x40 != 0,
            txvals: raw & 0x80 != 0,
        }
    }

    fn as_json(&self) -> Value {
        json!({
            "raw": self.raw,
            "tsprec_16bit_y": self.tsprec,
            "tcgram": self.tcgram,
            "tmulti": self.tmulti,
            "trandm": self.trandm,
            "tordrd": self.tordrd,
            "talabs": self.talabs,
            "txyxys": self.txyxys,
            "txvals": self.txvals,
        })
    }
}

#[derive(Clone, Debug)]
struct LabelSet {
    x: String,
    y: String,
    z: String,
}

#[derive(Clone, Debug)]
struct NewHeader {
    flags: SpcFlags,
    fexper: u8,
    fexp: u8,
    fnpts: usize,
    ffirst: f64,
    flast: f64,
    fnsub: usize,
    fxtype: u8,
    fytype: u8,
    fztype: u8,
    fdate: u32,
    fres: String,
    fsource: String,
    fcmnt: String,
    fcatxt: Vec<u8>,
    flogoff: usize,
    fprocs: u8,
    flevel: u8,
    fsampin: i16,
    ffactor: f32,
    fmethod: String,
    fzinc: f32,
    fwplanes: i32,
    fwinc: f32,
    fwtype: u8,
}

#[derive(Clone, Debug)]
struct OldHeader {
    flags: SpcFlags,
    oexp: i16,
    onpts: usize,
    ofirst: f64,
    olast: f64,
    fxtype: u8,
    fytype: u8,
    oyear: i16,
    omonth: u8,
    oday: u8,
    ohour: u8,
    ominute: u8,
    ores: String,
    ocmnt: String,
    ocatxt: Vec<u8>,
}

#[derive(Clone, Debug)]
struct SubHeader {
    subflgs: u8,
    subexp: u8,
    subindx: i16,
    subtime: f32,
    subnext: f32,
    subnois: f32,
    subnpts: usize,
    subscan: i32,
    subwlevel: f32,
}

#[derive(Clone, Debug)]
struct DirectoryEntry {
    offset: usize,
    size: usize,
    time: f32,
}

#[derive(Clone, Debug)]
struct ParsedSubfile {
    header: SubHeader,
    axis: Vec<f64>,
    values: Vec<f64>,
    directory: Option<DirectoryEntry>,
}

fn parse_new_lsb_spc(
    bytes: &[u8],
    path: &Path,
    source: SourceFile,
    reader: &str,
) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
    let header = parse_new_header(bytes)?;
    let labels = labels_from_new_header(&header);
    let header_metadata = new_header_metadata(&header, &labels);
    let log = parse_log_block(bytes, header.flogoff);
    let mut warnings = Vec::new();
    if header.flags.txyxys && !header.flags.txvals {
        warnings
            .push("TXYXYS is set without TXVALS; parsing as independent X subfiles".to_string());
    }
    if header.flags.trandm {
        warnings.push("random-order Z subfile flag is preserved but not reordered".to_string());
    }
    if header.flags.tcgram {
        warnings
            .push("TCGRAM compatibility flag is preserved but has no current effect".to_string());
    }

    let subfiles = parse_new_subfiles(bytes, &header, &mut warnings)?;
    records_from_subfiles(
        SpcRecordContext {
            source,
            reader,
            labels: &labels,
            fytype: header.fytype,
            header_metadata,
            log,
            warnings,
        },
        subfiles,
    )
    .map_err(|error| with_path_context(error, path))
}

fn parse_old_lsb_spc(
    bytes: &[u8],
    path: &Path,
    source: SourceFile,
    reader: &str,
) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
    let header = parse_old_header(bytes)?;
    let labels = labels_from_old_header(&header);
    let header_metadata = old_header_metadata(&header, &labels);
    let mut warnings = vec![
        "old_spc_header_limited: log blocks and old-format XY variants are not decoded yet"
            .to_string(),
    ];
    if header.flags.txyxys || header.flags.txvals {
        warnings.push("old_spc_xy_flags_ignored".to_string());
    }
    let axis = linspace(header.ofirst, header.olast, header.onpts);
    let subfiles = parse_old_subfiles(bytes, &header, &axis, &mut warnings)?;
    records_from_subfiles(
        SpcRecordContext {
            source,
            reader,
            labels: &labels,
            fytype: header.fytype,
            header_metadata,
            log: BTreeMap::new(),
            warnings,
        },
        subfiles,
    )
    .map_err(|error| with_path_context(error, path))
}

fn with_path_context(error: nirs4all_io_core::Error, path: &Path) -> nirs4all_io_core::Error {
    match error {
        nirs4all_io_core::Error::InvalidRecord(message) => {
            nirs4all_io_core::Error::InvalidRecord(format!("{}: {message}", path.display()))
        }
        other => other,
    }
}

struct SpcRecordContext<'a> {
    source: SourceFile,
    reader: &'a str,
    labels: &'a LabelSet,
    fytype: u8,
    header_metadata: Value,
    log: BTreeMap<String, Value>,
    warnings: Vec<String>,
}

fn records_from_subfiles(
    context: SpcRecordContext<'_>,
    subfiles: Vec<ParsedSubfile>,
) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
    if subfiles.is_empty() {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "Galactic SPC file contained no readable subfiles".to_string(),
        ));
    }

    let (axis_kind, axis_unit) = axis_kind_and_unit(&context.labels.x);
    let signal_type = signal_type_from_fytype(context.fytype, &context.labels.y);
    let signal_name = signal_name_from_type(&signal_type, &context.labels.y);
    let signal_unit = signal_unit_from_y_label(&context.labels.y, context.fytype);
    let role = context.labels.y.clone();
    let has_log = !context.log.is_empty();

    subfiles
        .into_iter()
        .enumerate()
        .map(|(index, subfile)| {
            let mut metadata = BTreeMap::new();
            let sample_id = log_sample_id(&context.log, index + 1)
                .unwrap_or_else(|| format!("subfile_{}", index + 1));
            metadata.insert("sample_id".to_string(), json!(sample_id));
            metadata.insert("galactic_spc".to_string(), context.header_metadata.clone());
            metadata.insert(
                "galactic_spc_subfile".to_string(),
                subfile_metadata(&subfile),
            );
            if has_log {
                metadata.insert("galactic_spc_log".to_string(), json!(context.log));
            }

            single_signal_record(
                "galactic-spc",
                context.reader,
                context.source.clone(),
                SingleSignalSpec {
                    axis_values: subfile.axis,
                    axis_unit: axis_unit.clone(),
                    axis_kind: axis_kind.clone(),
                    values: subfile.values,
                    signal_name: signal_name.clone(),
                    signal_type: signal_type.clone(),
                    signal_unit: signal_unit.clone(),
                    role: role.clone(),
                },
                BTreeMap::new(),
                metadata,
                context.warnings.clone(),
            )
        })
        .collect()
}

fn parse_new_header(bytes: &[u8]) -> Result<NewHeader> {
    require_len(bytes, NEW_HEADER_LEN, "Galactic SPC new header")?;
    if bytes[1] != 0x4b {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "not a new little-endian Galactic SPC header".to_string(),
        ));
    }
    let fnpts = nonnegative_i32_as_usize(bytes, 4, "fnpts")?;
    let fnsub_raw = le_i32(bytes, 24)?;
    let fnsub = if fnsub_raw <= 0 {
        1
    } else {
        fnsub_raw as usize
    };
    Ok(NewHeader {
        flags: SpcFlags::new(bytes[0]),
        fexper: bytes[2],
        fexp: bytes[3],
        fnpts,
        ffirst: le_f64(bytes, 8)?,
        flast: le_f64(bytes, 16)?,
        fnsub,
        fxtype: bytes[28],
        fytype: bytes[29],
        fztype: bytes[30],
        fdate: le_u32(bytes, 32)?,
        fres: clean_ascii(&bytes[36..45]),
        fsource: clean_ascii(&bytes[45..54]),
        fcmnt: clean_ascii(&bytes[88..218]),
        fcatxt: bytes[218..248].to_vec(),
        flogoff: nonnegative_i32_as_usize(bytes, 248, "flogoff")?,
        fprocs: bytes[256],
        flevel: bytes[257],
        fsampin: le_i16(bytes, 258)?,
        ffactor: le_f32(bytes, 260)?,
        fmethod: clean_ascii(&bytes[264..312]),
        fzinc: le_f32(bytes, 312)?,
        fwplanes: le_i32(bytes, 316)?,
        fwinc: le_f32(bytes, 320)?,
        fwtype: bytes[324],
    })
}

fn parse_old_header(bytes: &[u8]) -> Result<OldHeader> {
    require_len(bytes, OLD_HEADER_LEN, "Galactic SPC old header")?;
    if bytes[1] != 0x4d {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "not an old little-endian Galactic SPC header".to_string(),
        ));
    }
    let onpts_float = le_f32(bytes, 4)?;
    if !onpts_float.is_finite() || onpts_float <= 0.0 {
        return Err(nirs4all_io_core::Error::InvalidRecord(format!(
            "invalid old SPC point count {onpts_float}"
        )));
    }
    Ok(OldHeader {
        flags: SpcFlags::new(bytes[0]),
        oexp: le_i16(bytes, 2)?,
        onpts: onpts_float.round() as usize,
        ofirst: le_f32(bytes, 8)? as f64,
        olast: le_f32(bytes, 12)? as f64,
        fxtype: bytes[16],
        fytype: bytes[17],
        oyear: le_i16(bytes, 18)?,
        omonth: bytes[20],
        oday: bytes[21],
        ohour: bytes[22],
        ominute: bytes[23],
        ores: clean_ascii(&bytes[24..32]),
        ocmnt: clean_ascii(&bytes[64..194]),
        ocatxt: bytes[194..224].to_vec(),
    })
}

fn parse_new_subfiles(
    bytes: &[u8],
    header: &NewHeader,
    warnings: &mut Vec<String>,
) -> Result<Vec<ParsedSubfile>> {
    if header.flags.txyxys {
        if let Some(entries) = parse_directory(bytes, header) {
            return entries
                .into_iter()
                .map(|entry| {
                    let end = entry.offset.checked_add(entry.size).ok_or_else(|| {
                        nirs4all_io_core::Error::InvalidRecord(
                            "SPC subfile directory offset overflow".to_string(),
                        )
                    })?;
                    if end > bytes.len() {
                        return Err(nirs4all_io_core::Error::InvalidRecord(format!(
                            "SPC subfile directory entry exceeds file length: {}..{} > {}",
                            entry.offset,
                            end,
                            bytes.len()
                        )));
                    }
                    parse_new_subfile(
                        &bytes[entry.offset..end],
                        header,
                        None,
                        Some(entry),
                        warnings,
                    )
                })
                .collect();
        }

        let mut out = Vec::with_capacity(header.fnsub);
        let mut offset = NEW_HEADER_LEN;
        for _ in 0..header.fnsub {
            let header_bytes = bytes.get(offset..offset + SUB_HEADER_LEN).ok_or_else(|| {
                nirs4all_io_core::Error::InvalidRecord(
                    "SPC TXYXYS subfile header truncated".to_string(),
                )
            })?;
            let sub_header = parse_subheader(header_bytes)?;
            let pts = sub_header.subnpts;
            let exp = effective_new_exp(header, &sub_header, warnings);
            let y_bytes = y_value_byte_len(pts, exp, header.flags.tsprec)?;
            let size = SUB_HEADER_LEN + 4 * pts + y_bytes;
            let end = offset.checked_add(size).ok_or_else(|| {
                nirs4all_io_core::Error::InvalidRecord("SPC subfile offset overflow".to_string())
            })?;
            let data = bytes.get(offset..end).ok_or_else(|| {
                nirs4all_io_core::Error::InvalidRecord(
                    "SPC TXYXYS subfile data truncated".to_string(),
                )
            })?;
            out.push(parse_new_subfile(data, header, None, None, warnings)?);
            offset = end;
        }
        return Ok(out);
    }

    if header.fnpts == 0 {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "SPC header declares zero data points".to_string(),
        ));
    }

    let (axis, mut offset) = if header.flags.txvals {
        let x_start = NEW_HEADER_LEN;
        let x_end = x_start + 4 * header.fnpts;
        require_len(bytes, x_end, "SPC explicit X array")?;
        (parse_f32_array(bytes, x_start, header.fnpts)?, x_end)
    } else {
        (
            linspace(header.ffirst, header.flast, header.fnpts),
            NEW_HEADER_LEN,
        )
    };

    let mut out = Vec::with_capacity(header.fnsub);
    for _ in 0..header.fnsub {
        let sub_header =
            parse_subheader(bytes.get(offset..offset + SUB_HEADER_LEN).ok_or_else(|| {
                nirs4all_io_core::Error::InvalidRecord("SPC subfile header truncated".to_string())
            })?)?;
        let exp = effective_new_exp(header, &sub_header, warnings);
        let y_bytes = y_value_byte_len(header.fnpts, exp, header.flags.tsprec)?;
        let size = SUB_HEADER_LEN + y_bytes;
        let end = offset.checked_add(size).ok_or_else(|| {
            nirs4all_io_core::Error::InvalidRecord("SPC subfile offset overflow".to_string())
        })?;
        let data = bytes.get(offset..end).ok_or_else(|| {
            nirs4all_io_core::Error::InvalidRecord("SPC subfile data truncated".to_string())
        })?;
        out.push(parse_new_subfile(
            data,
            header,
            Some(&axis),
            None,
            warnings,
        )?);
        offset = end;
    }
    Ok(out)
}

fn parse_old_subfiles(
    bytes: &[u8],
    header: &OldHeader,
    axis: &[f64],
    warnings: &mut Vec<String>,
) -> Result<Vec<ParsedSubfile>> {
    let mut out = Vec::new();
    let mut offset = OLD_HEADER_LEN - SUB_HEADER_LEN;
    while offset + SUB_HEADER_LEN <= bytes.len() {
        let sub_header = parse_subheader(&bytes[offset..offset + SUB_HEADER_LEN])?;
        let pts = if sub_header.subnpts == 0 {
            header.onpts
        } else {
            sub_header.subnpts
        };
        if pts == 0 || pts > 10_000_000 {
            break;
        }
        let size = SUB_HEADER_LEN + 4 * pts;
        let Some(end) = offset.checked_add(size) else {
            break;
        };
        if end > bytes.len() {
            break;
        }
        let exp = effective_old_exp(header, &sub_header);
        let values = parse_old_y_values(&bytes[offset + SUB_HEADER_LEN..end], pts, exp)?;
        out.push(ParsedSubfile {
            header: sub_header,
            axis: axis.to_vec(),
            values,
            directory: None,
        });
        offset = end;
    }
    if out.is_empty() {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "old Galactic SPC file contained no readable subfiles".to_string(),
        ));
    }
    if offset < bytes.len() {
        warnings.push(format!(
            "old_spc_trailing_bytes_not_decoded: {} bytes",
            bytes.len() - offset
        ));
    }
    Ok(out)
}

fn parse_new_subfile(
    data: &[u8],
    header: &NewHeader,
    common_axis: Option<&[f64]>,
    directory: Option<DirectoryEntry>,
    warnings: &mut Vec<String>,
) -> Result<ParsedSubfile> {
    require_len(data, SUB_HEADER_LEN, "SPC subfile header")?;
    let sub_header = parse_subheader(&data[..SUB_HEADER_LEN])?;
    let exp = effective_new_exp(header, &sub_header, warnings);
    let points = if header.flags.txyxys {
        sub_header.subnpts
    } else {
        header.fnpts
    };
    if points == 0 {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "SPC subfile declares zero points".to_string(),
        ));
    }

    let (axis, y_offset) = if header.flags.txyxys {
        let x_start = SUB_HEADER_LEN;
        let x_end = x_start + 4 * points;
        require_len(data, x_end, "SPC TXYXYS X array")?;
        (parse_scaled_x_array(data, x_start, points, exp)?, x_end)
    } else {
        (
            common_axis
                .ok_or_else(|| {
                    nirs4all_io_core::Error::InvalidRecord(
                        "SPC common axis missing for subfile".to_string(),
                    )
                })?
                .to_vec(),
            SUB_HEADER_LEN,
        )
    };
    let values = parse_y_values(data, y_offset, points, exp, header.flags.tsprec)?;
    Ok(ParsedSubfile {
        header: sub_header,
        axis,
        values,
        directory,
    })
}

fn parse_directory(bytes: &[u8], header: &NewHeader) -> Option<Vec<DirectoryEntry>> {
    if !header.flags.txyxys || header.fnpts == 0 {
        return None;
    }
    let directory_start = header.fnpts;
    let directory_len = header.fnsub.checked_mul(12)?;
    if directory_start < NEW_HEADER_LEN || directory_start.checked_add(directory_len)? > bytes.len()
    {
        return None;
    }
    let mut entries = Vec::with_capacity(header.fnsub);
    for index in 0..header.fnsub {
        let offset = directory_start + index * 12;
        let sub_offset = le_i32(bytes, offset).ok()? as isize;
        let sub_size = le_i32(bytes, offset + 4).ok()? as isize;
        if sub_offset < 0 || sub_size <= 0 {
            return None;
        }
        entries.push(DirectoryEntry {
            offset: sub_offset as usize,
            size: sub_size as usize,
            time: le_f32(bytes, offset + 8).ok()?,
        });
    }
    Some(entries)
}

fn parse_subheader(bytes: &[u8]) -> Result<SubHeader> {
    require_len(bytes, SUB_HEADER_LEN, "SPC subfile header")?;
    let subnpts = le_i32(bytes, 16)?;
    Ok(SubHeader {
        subflgs: bytes[0],
        subexp: bytes[1],
        subindx: le_i16(bytes, 2)?,
        subtime: le_f32(bytes, 4)?,
        subnext: le_f32(bytes, 8)?,
        subnois: le_f32(bytes, 12)?,
        subnpts: if subnpts < 0 { 0 } else { subnpts as usize },
        subscan: le_i32(bytes, 20)?,
        subwlevel: le_f32(bytes, 24)?,
    })
}

fn parse_y_values(
    bytes: &[u8],
    offset: usize,
    count: usize,
    exp: EffectiveExp,
    tsprec: bool,
) -> Result<Vec<f64>> {
    match exp {
        EffectiveExp::Float32 => parse_f32_array(bytes, offset, count),
        EffectiveExp::Integer(exponent) if tsprec => {
            require_len(bytes, offset + count * 2, "SPC 16-bit Y array")?;
            (0..count)
                .map(|index| {
                    le_i16(bytes, offset + index * 2)
                        .map(|value| (value as f64) * 2_f64.powi(exponent - 16))
                })
                .collect()
        }
        EffectiveExp::Integer(exponent) => {
            require_len(bytes, offset + count * 4, "SPC 32-bit Y array")?;
            (0..count)
                .map(|index| {
                    le_i32(bytes, offset + index * 4)
                        .map(|value| (value as f64) * 2_f64.powi(exponent - 32))
                })
                .collect()
        }
    }
}

fn parse_scaled_x_array(
    bytes: &[u8],
    offset: usize,
    count: usize,
    exp: EffectiveExp,
) -> Result<Vec<f64>> {
    match exp {
        EffectiveExp::Float32 => parse_f32_array(bytes, offset, count),
        EffectiveExp::Integer(exponent) => {
            require_len(bytes, offset + count * 4, "SPC TXYXYS X array")?;
            (0..count)
                .map(|index| {
                    le_i32(bytes, offset + index * 4)
                        .map(|value| (value as f64) * 2_f64.powi(exponent - 32))
                })
                .collect()
        }
    }
}

fn parse_old_y_values(bytes: &[u8], count: usize, exp: EffectiveExp) -> Result<Vec<f64>> {
    require_len(bytes, count * 4, "old SPC Y array")?;
    match exp {
        EffectiveExp::Float32 => (0..count)
            .map(|index| le_f32(bytes, index * 4).map(f64::from))
            .collect(),
        EffectiveExp::Integer(exponent) => (0..count)
            .map(|index| {
                let offset = index * 4;
                let raw = (bytes[offset + 1] as u32) << 24
                    | (bytes[offset] as u32) << 16
                    | (bytes[offset + 3] as u32) << 8
                    | (bytes[offset + 2] as u32);
                let signed = raw as i32;
                Ok((signed as f64) * 2_f64.powi(exponent - 32))
            })
            .collect(),
    }
}

fn parse_f32_array(bytes: &[u8], offset: usize, count: usize) -> Result<Vec<f64>> {
    require_len(bytes, offset + count * 4, "SPC float32 array")?;
    (0..count)
        .map(|index| le_f32(bytes, offset + index * 4).map(f64::from))
        .collect()
}

#[derive(Copy, Clone, Debug)]
enum EffectiveExp {
    Float32,
    Integer(i32),
}

fn effective_new_exp(
    header: &NewHeader,
    sub_header: &SubHeader,
    warnings: &mut Vec<String>,
) -> EffectiveExp {
    let raw = if header.flags.tmulti {
        sub_header.subexp
    } else {
        header.fexp
    };
    effective_exp_from_byte(raw, warnings)
}

fn effective_old_exp(header: &OldHeader, sub_header: &SubHeader) -> EffectiveExp {
    if sub_header.subexp == 128 {
        EffectiveExp::Float32
    } else if sub_header.subexp > 0 && sub_header.subexp < 128 {
        EffectiveExp::Integer(sub_header.subexp as i32)
    } else {
        EffectiveExp::Integer(header.oexp as i32)
    }
}

fn effective_exp_from_byte(raw: u8, warnings: &mut Vec<String>) -> EffectiveExp {
    if raw == 128 {
        EffectiveExp::Float32
    } else if raw < 128 {
        EffectiveExp::Integer(raw as i32)
    } else {
        let warning = format!("invalid_spc_integer_exponent_{raw}_treated_as_0");
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
        EffectiveExp::Integer(0)
    }
}

fn y_value_byte_len(count: usize, exp: EffectiveExp, tsprec: bool) -> Result<usize> {
    let bytes_per_value = match exp {
        EffectiveExp::Float32 => 4,
        EffectiveExp::Integer(_) if tsprec => 2,
        EffectiveExp::Integer(_) => 4,
    };
    count.checked_mul(bytes_per_value).ok_or_else(|| {
        nirs4all_io_core::Error::InvalidRecord("SPC data byte length overflow".to_string())
    })
}

fn labels_from_new_header(header: &NewHeader) -> LabelSet {
    labels_from_fields(
        header.flags.talabs,
        &header.fcatxt,
        header.fxtype,
        header.fytype,
        header.fztype,
    )
}

fn labels_from_old_header(header: &OldHeader) -> LabelSet {
    labels_from_fields(
        header.flags.talabs,
        &header.ocatxt,
        header.fxtype,
        header.fytype,
        0,
    )
}

fn labels_from_fields(talabs: bool, fcatxt: &[u8], fxtype: u8, fytype: u8, fztype: u8) -> LabelSet {
    let mut labels = LabelSet {
        x: x_label_from_code(fxtype).to_string(),
        y: y_label_from_code(fytype).to_string(),
        z: x_label_from_code(fztype).to_string(),
    };
    if talabs {
        let custom = split_null_labels(fcatxt);
        if let Some(label) = custom.first().filter(|value| !value.is_empty()) {
            labels.x = label.clone();
        }
        if let Some(label) = custom.get(1).filter(|value| !value.is_empty()) {
            labels.y = label.clone();
        }
        if let Some(label) = custom.get(2).filter(|value| !value.is_empty()) {
            labels.z = label.clone();
        }
    }
    labels
}

fn new_header_metadata(header: &NewHeader, labels: &LabelSet) -> Value {
    json!({
        "version": "new_lsb_0x4b",
        "flags": header.flags.as_json(),
        "experiment_type": experiment_type_label(header.fexper),
        "fexper": header.fexper,
        "fexp": header.fexp,
        "fnpts": header.fnpts,
        "ffirst": header.ffirst,
        "flast": header.flast,
        "fnsub": header.fnsub,
        "x_type": header.fxtype,
        "y_type": header.fytype,
        "z_type": header.fztype,
        "x_label": labels.x,
        "y_label": labels.y,
        "z_label": labels.z,
        "fdate_raw": header.fdate,
        "resolution": header.fres,
        "source": header.fsource,
        "comment": header.fcmnt,
        "flogoff": header.flogoff,
        "fprocs": header.fprocs,
        "flevel": header.flevel,
        "fsampin": header.fsampin,
        "ffactor": header.ffactor,
        "method": header.fmethod,
        "fzinc": header.fzinc,
        "fwplanes": header.fwplanes,
        "fwinc": header.fwinc,
        "fwtype": header.fwtype,
    })
}

fn old_header_metadata(header: &OldHeader, labels: &LabelSet) -> Value {
    json!({
        "version": "old_lsb_0x4d",
        "flags": header.flags.as_json(),
        "oexp": header.oexp,
        "onpts": header.onpts,
        "ofirst": header.ofirst,
        "olast": header.olast,
        "x_type": header.fxtype,
        "y_type": header.fytype,
        "x_label": labels.x,
        "y_label": labels.y,
        "z_label": labels.z,
        "year": header.oyear,
        "month": header.omonth,
        "day": header.oday,
        "hour": header.ohour,
        "minute": header.ominute,
        "resolution": header.ores,
        "comment": header.ocmnt,
    })
}

fn subfile_metadata(subfile: &ParsedSubfile) -> Value {
    let directory = subfile.directory.as_ref().map(|entry| {
        json!({
            "offset": entry.offset,
            "size": entry.size,
            "time": entry.time,
        })
    });
    json!({
        "subflgs": subfile.header.subflgs,
        "subexp": subfile.header.subexp,
        "subindx": subfile.header.subindx,
        "subtime": subfile.header.subtime,
        "subnext": subfile.header.subnext,
        "subnois": subfile.header.subnois,
        "subnpts": subfile.header.subnpts,
        "subscan": subfile.header.subscan,
        "subwlevel": subfile.header.subwlevel,
        "directory": directory,
    })
}

fn parse_log_block(bytes: &[u8], offset: usize) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    if offset == 0 || offset + LOG_HEADER_LEN > bytes.len() {
        return out;
    }
    let Ok(logsizd) = nonnegative_i32_as_usize(bytes, offset, "logsizd") else {
        return out;
    };
    let Ok(logtxto) = nonnegative_i32_as_usize(bytes, offset + 8, "logtxto") else {
        return out;
    };
    let start = offset + logtxto;
    let Some(declared_end) = start.checked_add(logsizd) else {
        return out;
    };
    if start >= bytes.len() {
        return out;
    }
    let end = declared_end.min(bytes.len());
    let text = String::from_utf8_lossy(&bytes[start..end]).replace('\r', "");
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        out.insert(key.to_string(), json!(value.trim()));
    }
    out
}

fn log_sample_id(log: &BTreeMap<String, Value>, one_based_index: usize) -> Option<String> {
    let key = format!("SUBFILE{one_based_index}");
    let value = log
        .iter()
        .find(|(candidate, _)| candidate.trim().eq_ignore_ascii_case(&key))?
        .1
        .as_str()?;
    quoted_tail(value)
        .or_else(|| Some(value.trim().to_string()))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn quoted_tail(value: &str) -> Option<String> {
    let start = value.rfind('"')?;
    let before = &value[..start];
    let start = before.rfind('"')?;
    Some(before[start + 1..].to_string())
}

fn axis_kind_and_unit(label: &str) -> (AxisKind, String) {
    let lower = label.to_ascii_lowercase();
    if lower.contains("wavenumber") || lower.contains("raman shift") || lower.contains("cm-1") {
        (AxisKind::Wavenumber, "cm-1".to_string())
    } else if lower.contains("nanometer") || lower == "nm" || lower.contains("(nm)") {
        (AxisKind::Wavelength, "nm".to_string())
    } else if lower.contains("micrometer") || lower.contains("um") {
        (AxisKind::Wavelength, "um".to_string())
    } else if lower.contains("hertz") || lower.contains("hz") {
        (AxisKind::Frequency, unit_from_label(label))
    } else if lower.contains("minute") {
        (AxisKind::Index, "min".to_string())
    } else if lower.contains("second") {
        (AxisKind::Index, "s".to_string())
    } else if lower.contains("ev") {
        (AxisKind::Index, "eV".to_string())
    } else {
        (AxisKind::Index, unit_from_label(label))
    }
}

fn unit_from_label(label: &str) -> String {
    let lower = label.to_ascii_lowercase();
    if lower.contains("khz") {
        "kHz".to_string()
    } else if lower.contains("mhz") {
        "MHz".to_string()
    } else if lower.contains("ghz") {
        "GHz".to_string()
    } else if lower.contains("hz") {
        "Hz".to_string()
    } else if lower.contains("ppm") {
        "ppm".to_string()
    } else if lower.contains("m/z") {
        "m/z".to_string()
    } else if lower.contains("degree") {
        "deg".to_string()
    } else if lower.contains("temperature (f)") {
        "degF".to_string()
    } else if lower.contains("temperature (c)") {
        "degC".to_string()
    } else if lower.contains("temperature (k)") {
        "K".to_string()
    } else {
        "index".to_string()
    }
}

fn signal_type_from_fytype(fytype: u8, label: &str) -> SignalType {
    match fytype {
        1 => SignalType::Interferogram,
        2 => SignalType::Absorbance,
        3 => SignalType::KubelkaMunk,
        4 => SignalType::RawCounts,
        10 => SignalType::Absorbance,
        12 | 13 => SignalType::RawCounts,
        128 => SignalType::Transmittance,
        129 => SignalType::Reflectance,
        130 => SignalType::SingleBeam,
        _ => {
            let inferred = signal_type_from_label(label);
            if inferred == SignalType::Unknown && label.to_ascii_lowercase().contains("intensity") {
                SignalType::RawCounts
            } else {
                inferred
            }
        }
    }
}

fn signal_name_from_type(signal_type: &SignalType, label: &str) -> String {
    match signal_type {
        SignalType::Absorbance => "absorbance".to_string(),
        SignalType::Reflectance => "reflectance".to_string(),
        SignalType::Transmittance => "transmittance".to_string(),
        SignalType::Radiance => "radiance".to_string(),
        SignalType::Irradiance => "irradiance".to_string(),
        SignalType::RawCounts => safe_signal_name(label, "intensity"),
        SignalType::SingleBeam => "single_beam".to_string(),
        SignalType::Interferogram => "interferogram".to_string(),
        SignalType::KubelkaMunk => "kubelka_munk".to_string(),
        SignalType::Derivative => "derivative".to_string(),
        SignalType::Preprocessed => "preprocessed".to_string(),
        SignalType::Unknown => safe_signal_name(label, "signal"),
    }
}

fn signal_unit_from_y_label(label: &str, fytype: u8) -> Option<String> {
    if fytype == 11 || label.to_ascii_lowercase().contains("percent") {
        Some("%".to_string())
    } else {
        None
    }
}

fn x_label_from_code(code: u8) -> &'static str {
    match code {
        0 => "Arbitrary",
        1 => "Wavenumber (cm-1)",
        2 => "Micrometers (um)",
        3 => "Nanometers (nm)",
        4 => "Seconds",
        5 => "Minutes",
        6 => "Hertz (Hz)",
        7 => "Kilohertz (KHz)",
        8 => "Megahertz (MHz)",
        9 => "Mass (M/z)",
        10 => "Parts per million (PPM)",
        11 => "Days",
        12 => "Years",
        13 => "Raman Shift (cm-1)",
        14 => "eV",
        16 => "Diode Number",
        17 => "Channel",
        18 => "Degrees",
        19 => "Temperature (F)",
        20 => "Temperature (C)",
        21 => "Temperature (K)",
        22 => "Data Points",
        23 => "Milliseconds (mSec)",
        24 => "Microseconds (uSec)",
        25 => "Nanoseconds (nSec)",
        26 => "Gigahertz (GHz)",
        27 => "Centimeters (cm)",
        28 => "Meters (m)",
        29 => "Millimeters (mm)",
        30 => "Hours",
        _ => "Unknown",
    }
}

fn y_label_from_code(code: u8) -> &'static str {
    match code {
        0 => "Arbitrary Intensity",
        1 => "Interferogram",
        2 => "Absorbance",
        3 => "Kubelka-Munk",
        4 => "Counts",
        5 => "Volts",
        6 => "Degrees",
        7 => "Milliamps",
        8 => "Millimeters",
        9 => "Millivolts",
        10 => "Log(1/R)",
        11 => "Percent",
        12 => "Intensity",
        13 => "Relative Intensity",
        14 => "Energy",
        16 => "Decibel",
        19 => "Temperature (F)",
        20 => "Temperature (C)",
        21 => "Temperature (K)",
        22 => "Index of Refraction [N]",
        23 => "Extinction Coeff. [K]",
        24 => "Real",
        25 => "Imaginary",
        26 => "Complex",
        128 => "Transmission",
        129 => "Reflectance",
        130 => "Single Beam",
        131 => "Emission",
        _ => "Unknown",
    }
}

fn experiment_type_label(code: u8) -> &'static str {
    match code {
        0 => "General SPC",
        1 => "Gas Chromatogram",
        2 => "General Chromatogram",
        3 => "HPLC Chromatogram",
        4 => "FT-IR, FT-NIR, FT-Raman Spectrum or Igram",
        5 => "NIR Spectrum",
        6 => "UV-VIS Spectrum",
        7 => "X-ray Diffraction Spectrum",
        8 => "Mass Spectrum",
        9 => "NMR Spectrum or FID",
        10 => "Raman Spectrum",
        11 => "Fluorescence Spectrum",
        12 => "Atomic Spectrum",
        13 => "Chromatography Diode Array Spectra",
        _ => "Unknown",
    }
}

fn split_null_labels(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|byte| *byte == 0)
        .map(clean_ascii)
        .collect::<Vec<_>>()
}

fn clean_ascii(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end])
        .replace('\0', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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

fn looks_like_new_lsb_spc(head: &[u8]) -> bool {
    if head.len() < NEW_HEADER_LEN || head[1] != 0x4b {
        return false;
    }
    let fnpts = le_i32(head, 4).ok();
    let ffirst = le_f64(head, 8).ok();
    let flast = le_f64(head, 16).ok();
    let fnsub = le_i32(head, 24).ok();
    matches!((fnpts, ffirst, flast, fnsub), (Some(points), Some(first), Some(last), Some(subs))
        if (0..500_000_000).contains(&points)
            && (0..5_000_000).contains(&subs)
            && first.is_finite()
            && last.is_finite())
}

fn looks_like_old_spc(head: &[u8]) -> bool {
    if head.len() < OLD_HEADER_LEN || head[1] != 0x4d {
        return false;
    }
    let points = le_f32(head, 4).ok();
    let first = le_f32(head, 8).ok();
    let last = le_f32(head, 12).ok();
    matches!((points, first, last), (Some(points), Some(first), Some(last))
        if points.is_finite()
            && (0.0..500_000_000.0).contains(&points)
            && points != 0.0
            && first.is_finite()
            && last.is_finite())
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

fn nonnegative_i32_as_usize(bytes: &[u8], offset: usize, name: &str) -> Result<usize> {
    let value = le_i32(bytes, offset)?;
    if value < 0 {
        return Err(nirs4all_io_core::Error::InvalidRecord(format!(
            "negative SPC {name} value {value}"
        )));
    }
    Ok(value as usize)
}

fn le_i16(bytes: &[u8], offset: usize) -> Result<i16> {
    let data = bytes.get(offset..offset + 2).ok_or_else(|| {
        nirs4all_io_core::Error::InvalidRecord("SPC i16 field truncated".to_string())
    })?;
    Ok(i16::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn le_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let data = bytes.get(offset..offset + 4).ok_or_else(|| {
        nirs4all_io_core::Error::InvalidRecord("SPC u32 field truncated".to_string())
    })?;
    Ok(u32::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn le_i32(bytes: &[u8], offset: usize) -> Result<i32> {
    let data = bytes.get(offset..offset + 4).ok_or_else(|| {
        nirs4all_io_core::Error::InvalidRecord("SPC i32 field truncated".to_string())
    })?;
    Ok(i32::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn le_f32(bytes: &[u8], offset: usize) -> Result<f32> {
    let data = bytes.get(offset..offset + 4).ok_or_else(|| {
        nirs4all_io_core::Error::InvalidRecord("SPC f32 field truncated".to_string())
    })?;
    Ok(f32::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

fn le_f64(bytes: &[u8], offset: usize) -> Result<f64> {
    let data = bytes.get(offset..offset + 8).ok_or_else(|| {
        nirs4all_io_core::Error::InvalidRecord("SPC f64 field truncated".to_string())
    })?;
    Ok(f64::from_le_bytes(
        data.try_into().expect("slice length checked"),
    ))
}
