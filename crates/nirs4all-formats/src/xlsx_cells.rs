//! Calamine value authority joined ordinally to a narrowly scoped XML overlay.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Cursor, Read};

use calamine::{DataRef, Reader, Xlsx};
use nirs4all_formats_core::xlsx_cells::*;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader as XmlReader;
use sha2::{Digest, Sha256};
use zip::{read::ZipFile, ZipArchive};

type Archive<'a> = ZipArchive<Cursor<&'a [u8]>>;
type PartXml<'a, 'b> = XmlReader<BufReader<ZipFile<'a, Cursor<&'b [u8]>>>>;
type DecodeResult<T> = std::result::Result<T, XlsxDecodeError>;

fn decode_error(error: impl std::fmt::Display) -> XlsxDecodeError {
    XlsxDecodeError::Decode(error.to_string())
}

fn limit(label: &'static str, maximum: u64) -> XlsxDecodeError {
    XlsxDecodeError::Limit { label, maximum }
}

fn xml_reader<R: BufRead>(reader: R) -> XmlReader<R> {
    let mut xml = XmlReader::from_reader(reader);
    let config = xml.config_mut();
    config.check_end_names = false;
    config.trim_text(false);
    config.check_comments = false;
    config.expand_empty_elements = true;
    xml
}

/// This asymmetry is deliberate: calamine normalizes cache keys, but only
/// lowercases lookup paths, falling back to the original path when absent.
fn part_xml<'a, 'b>(
    archive: &'a mut Archive<'b>,
    cache: &HashMap<String, String>,
    path: &str,
) -> DecodeResult<PartXml<'a, 'b>> {
    let key = path.to_ascii_lowercase();
    let name = cache.get(&key).map(String::as_str).unwrap_or(path);
    let part = archive.by_name(name).map_err(decode_error)?;
    Ok(xml_reader(BufReader::new(part)))
}

fn raw_attr(start: &BytesStart<'_>, name: &str) -> DecodeResult<Option<String>> {
    start
        .try_get_attribute(name)
        .map_err(decode_error)
        .map(|attr| attr.map(|attr| attr.value.into_owned()))
}

fn unescape_attr(value: &str) -> DecodeResult<String> {
    quick_xml::escape::unescape(value)
        .map(|value| value.into_owned())
        .map_err(decode_error)
}

/// Guard every reservation spelling the pinned shared-string reader can see,
/// before Xlsx::new can reserve. This retains IO's existing conservative guard.
fn unique_strings_fit(part: impl Read, size: u64) -> DecodeResult<bool> {
    let mut xml = xml_reader(BufReader::new(part));
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match xml.read_event_into(&mut buf).map_err(decode_error)? {
            Event::Start(start) if start.local_name().as_ref() == "sst" => {
                let attributes = start.attributes_raw();
                return Ok(attributes.match_indices("uniqueCount").all(|(at, key)| {
                    let mut digits = attributes[at + key.len()..]
                        .bytes()
                        .skip_while(|c| !c.is_ascii_digit())
                        .take_while(u8::is_ascii_digit);
                    let count = digits.try_fold(0_u64, |count, c| {
                        count.checked_mul(10)?.checked_add(u64::from(c - b'0'))
                    });
                    count.is_some_and(|count| count <= size)
                }));
            }
            Event::Eof => return Ok(true),
            _ => {}
        }
    }
}

fn preflight(archive: &mut Archive<'_>, limits: XlsxDecodeLimits) -> DecodeResult<u64> {
    let mut decoded = 0_u64;
    let maximum = limits.max_decoded_file_bytes.min(limits.decoded_allowance);
    for index in 0..archive.len() {
        let size = archive.by_index_raw(index).map_err(decode_error)?.size();
        decoded = decoded
            .checked_add(size)
            .ok_or_else(|| limit("decoded bytes overflow", maximum))?;
        if decoded > maximum {
            return Err(limit("decoded file/total bytes", maximum));
        }
    }
    for index in 0..archive.len() {
        let part = archive.by_index(index).map_err(decode_error)?;
        let size = part.size();
        if part
            .name()
            .map_err(decode_error)?
            .to_ascii_lowercase()
            .ends_with("sharedstrings.xml")
            && !unique_strings_fit(part, size)?
        {
            return Err(decode_error(format!(
                "the shared-strings part declares more strings than its {size} bytes can hold"
            )));
        }
        // Parts remain streamed through ZIP's bounded reader when consumed;
        // do not expand unrelated archive members or retain archive copies.
    }
    Ok(decoded)
}

fn charge_shared_copy(
    usage: &mut XlsxDecodeUsage,
    bytes: usize,
    limits: XlsxDecodeLimits,
) -> DecodeResult<()> {
    let copies = usage
        .shared_copy_bytes
        .checked_add(bytes as u64)
        .ok_or_else(|| limit("decoded bytes overflow", limits.decoded_allowance))?;
    let total = usage
        .archive_decoded_bytes
        .checked_add(copies)
        .ok_or_else(|| limit("decoded bytes overflow", limits.decoded_allowance))?;
    if total > limits.max_decoded_file_bytes {
        return Err(limit("decoded file bytes", limits.max_decoded_file_bytes));
    }
    if total > limits.decoded_allowance {
        return Err(limit("decoded file/total bytes", limits.decoded_allowance));
    }
    usage.shared_copy_bytes = copies;
    Ok(())
}

fn relationships(
    archive: &mut Archive<'_>,
    cache: &HashMap<String, String>,
    path: &str,
) -> DecodeResult<Vec<HashMap<String, String>>> {
    let mut xml = part_xml(archive, cache, path)?;
    let mut buf = Vec::new();
    let mut relationships = Vec::new();
    loop {
        buf.clear();
        match xml.read_event_into(&mut buf).map_err(decode_error)? {
            Event::Start(start) if start.local_name().as_ref() == "Relationship" => {
                let mut attrs = HashMap::new();
                for attr in start.attributes().with_checks(false) {
                    let attr = attr.map_err(decode_error)?;
                    attrs.insert(attr.key.as_ref().to_owned(), attr.value.into_owned());
                }
                relationships.push(attrs);
            }
            Event::End(end) if end.local_name().as_ref() == "Relationships" => {
                return Ok(relationships);
            }
            Event::Eof => return Err(decode_error("unexpected EOF in Relationships")),
            _ => {}
        }
    }
}

fn sheet_paths(
    archive: &mut Archive<'_>,
    cache: &HashMap<String, String>,
) -> DecodeResult<Vec<(String, String)>> {
    let package = relationships(archive, cache, "_rels/.rels")?;
    let mut target = None;
    for rel in package {
        if rel
            .get("Type")
            .is_some_and(|kind| kind.ends_with("/relationships/officeDocument"))
        {
            if let Some(value) = rel.get("Target") {
                target = Some(unescape_attr(value)?);
            }
        }
    }
    let target = target.ok_or_else(|| decode_error("officeDocument relationship not found"))?;
    let xl_path = target
        .rfind('/')
        .map(|i| &target[..=i])
        .map(|path| path.strip_prefix('/').unwrap_or(path))
        .unwrap_or("");
    let rels = relationships(archive, cache, &format!("{xl_path}_rels/workbook.xml.rels"))?;
    let mut targets = HashMap::new();
    for rel in rels {
        if let Some(id) = rel.get("Id") {
            targets.insert(id.clone(), rel.get("Target").cloned().unwrap_or_default());
        }
    }
    let path = format!("{xl_path}workbook.xml");
    // Calamine admits a missing workbook part with no sheets.
    let key = path.to_ascii_lowercase();
    let zip_name = cache.get(&key).map(String::as_str).unwrap_or(&path);
    if archive.index_for_name(zip_name).is_none() {
        return Ok(Vec::new());
    }
    let mut xml = part_xml(archive, cache, &path)?;
    let mut buf = Vec::new();
    let mut sheets = Vec::new();
    loop {
        buf.clear();
        match xml.read_event_into(&mut buf).map_err(decode_error)? {
            Event::Start(start) if start.local_name().as_ref() == "sheet" => {
                let mut name = String::new();
                let mut id = String::new();
                for attr in start.attributes().with_checks(false) {
                    let attr = attr.map_err(decode_error)?;
                    match attr.key.as_ref() {
                        "name" => name = unescape_attr(&attr.value)?,
                        key if key.rsplit(':').next() == Some("id") => {
                            id = attr.value.into_owned();
                        }
                        _ => {}
                    }
                }
                let target = targets
                    .get(&id)
                    .ok_or_else(|| decode_error("sheet relationship not found"))?;
                let path = target
                    .strip_prefix('/')
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("{xl_path}{target}"));
                sheets.push((name, path));
            }
            Event::End(end) if end.local_name().as_ref() == "workbook" => return Ok(sheets),
            Event::Eof => return Err(decode_error("unexpected EOF in workbook")),
            _ => {}
        }
    }
}

#[derive(Default)]
struct OverlayCell {
    reference: Option<String>,
    cell_type: Option<String>,
    formula: bool,
    token: Option<String>,
}

struct OverlayReader<R> {
    xml: XmlReader<R>,
    buf: Vec<u8>,
    done: bool,
}

impl<R: BufRead> OverlayReader<R> {
    fn new(mut xml: XmlReader<R>) -> DecodeResult<Self> {
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match xml.read_event_into(&mut buf).map_err(decode_error)? {
                Event::Start(start) if start.local_name().as_ref() == "sheetData" => break,
                Event::Eof => return Err(decode_error("unexpected EOF before sheetData")),
                _ => {}
            }
        }
        Ok(Self {
            xml,
            buf,
            done: false,
        })
    }

    fn next_record(&mut self) -> DecodeResult<Option<OverlayCell>> {
        if self.done {
            return Ok(None);
        }
        loop {
            self.buf.clear();
            match self
                .xml
                .read_event_into(&mut self.buf)
                .map_err(decode_error)?
            {
                Event::Start(start) if start.local_name().as_ref() == "c" => {
                    let record = OverlayCell {
                        reference: raw_attr(&start, "r")?,
                        cell_type: raw_attr(&start, "t")?,
                        ..OverlayCell::default()
                    };
                    return self.read_cell(record).map(Some);
                }
                Event::End(end) if end.local_name().as_ref() == "sheetData" => {
                    self.done = true;
                    return Ok(None);
                }
                Event::Eof => return Err(decode_error("unexpected EOF in sheetData overlay")),
                _ => {}
            }
        }
    }

    fn read_cell(&mut self, mut record: OverlayCell) -> DecodeResult<OverlayCell> {
        let numeric = matches!(record.cell_type.as_deref(), None | Some("n"));
        let mut depth = 0_usize;
        let mut last_value = false;
        let mut events = 0_usize;
        let mut plain_text = false;
        loop {
            self.buf.clear();
            match self
                .xml
                .read_event_into(&mut self.buf)
                .map_err(decode_error)?
            {
                Event::Start(start) => {
                    if depth == 0 {
                        let name = start.local_name();
                        last_value = name.as_ref() == "v";
                        record.formula |= name.as_ref() == "f";
                        record.token = None;
                        events = 0;
                        plain_text = false;
                    } else if last_value {
                        events += 1;
                        plain_text = false;
                    }
                    depth += 1;
                }
                Event::End(end) if depth == 0 && end.local_name().as_ref() == "c" => {
                    if numeric && last_value && events != 0 && !(events == 1 && plain_text) {
                        return Err(decode_error("ambiguous numeric XML content: expected exactly one Text event in the last v child"));
                    }
                    return Ok(record);
                }
                Event::End(_) => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or_else(|| decode_error("unexpected cell overlay end tag"))?;
                }
                Event::Text(text) if depth == 1 && last_value => {
                    events += 1;
                    plain_text = events == 1;
                    if numeric && events == 1 {
                        // Raw content: never unescape, concatenate, or reparse as f64.
                        record.token = Some(text.as_ref().to_owned());
                    }
                }
                Event::Eof => return Err(decode_error("unexpected EOF in cell overlay")),
                _ if depth > 0 && last_value => {
                    events += 1;
                    plain_text = false;
                }
                _ => {}
            }
        }
    }
}

/// Validate explicit positions only. Missing positions remain calamine's job.
fn explicit_position(reference: &str) -> DecodeResult<(u32, u32)> {
    let mut bytes = reference.bytes().peekable();
    let mut col = 0_u32;
    while let Some(c) = bytes.next_if(u8::is_ascii_alphabetic) {
        col = col
            .checked_mul(26)
            .and_then(|col| col.checked_add(u32::from(c.to_ascii_uppercase() - b'A') + 1))
            .ok_or_else(|| decode_error("cell reference column overflow"))?;
    }
    let mut row = 0_u32;
    for c in bytes {
        if !c.is_ascii_digit() {
            return Err(decode_error("invalid explicit cell reference"));
        }
        row = row
            .checked_mul(10)
            .and_then(|row| row.checked_add(u32::from(c - b'0')))
            .ok_or_else(|| decode_error("cell reference row overflow"))?;
    }
    Ok((
        row.checked_sub(1)
            .ok_or_else(|| decode_error("missing cell reference row"))?,
        col.checked_sub(1)
            .ok_or_else(|| decode_error("missing cell reference column"))?,
    ))
}

fn integer_spelling(token: &str) -> bool {
    let digits = token
        .strip_prefix('+')
        .or_else(|| token.strip_prefix('-'))
        .unwrap_or(token);
    !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit())
}

fn numeric_token(record: &mut OverlayCell) -> DecodeResult<String> {
    record
        .token
        .take()
        .filter(|token| !token.is_empty())
        .ok_or_else(|| decode_error("numeric cell has no plain XML token"))
}

fn render_date(value: &calamine::ExcelDateTime, epoch_1904: bool) -> Option<String> {
    // Mirror calamine's epoch and leap-day adjustments only to guard its
    // unchecked Duration::milliseconds(ms.round() as i64) construction.
    let serial = value.as_f64();
    let days = if epoch_1904 { serial + 1462.0 } else { serial };
    let days = if days >= 60.0 { days } else { days + 1.0 };
    let milliseconds = (days * 86_400_000.0).round();
    // Chrono rejects i64::MIN milliseconds. Strict float bounds also reject
    // NaN/infinities and saturation: i64::MAX as f64 rounds up to 2^63.
    // Every representable calendar date is well inside these duration bounds;
    // let calamine's checked date addition handle the narrower calendar range.
    if milliseconds > i64::MIN as f64 && milliseconds < i64::MAX as f64 {
        value.as_datetime().map(|value| value.to_string())
    } else {
        None
    }
}

fn typed_value(
    value: &DataRef<'_>,
    record: &mut OverlayCell,
    epoch_1904: bool,
    usage: &mut XlsxDecodeUsage,
    limits: XlsxDecodeLimits,
) -> DecodeResult<XlsxCellValue> {
    Ok(match value {
        DataRef::Float(value) => {
            let token = numeric_token(record)?;
            if integer_spelling(&token) {
                XlsxCellValue::IntegerLexeme { token }
            } else {
                XlsxCellValue::Float64 {
                    bits: value.to_bits(),
                    token,
                }
            }
        }
        // The pinned XLSX reader produces Float/DateTime for numeric XML.
        // Refuse a changed dependency contract instead of narrowing an integer.
        DataRef::Int(_) => return Err(decode_error("unexpected calamine XLSX Int result")),
        DataRef::DateTime(value) => XlsxCellValue::Date {
            kind: if value.is_duration() {
                XlsxDateKind::TimeDelta
            } else {
                XlsxDateKind::DateTime
            },
            serial_bits: value.as_f64().to_bits(),
            token: numeric_token(record)?,
            epoch_1904,
            rendered: render_date(value, epoch_1904),
        },
        DataRef::SharedString(text) => {
            charge_shared_copy(usage, text.len(), limits)?;
            XlsxCellValue::Text((*text).to_owned())
        }
        DataRef::String(text) | DataRef::DateTimeIso(text) | DataRef::DurationIso(text) => {
            XlsxCellValue::Text(text.clone())
        }
        DataRef::Bool(value) => XlsxCellValue::Bool(*value),
        DataRef::Empty => XlsxCellValue::Empty,
        DataRef::Error(error) => XlsxCellValue::Error(error.to_string()),
    })
}

/// Decode unencrypted XLSX/XLSM bytes into physical sparse cells.
///
/// This operation assigns no roles, NA policy, integer storage domain or dense
/// table shape. It preserves numeric lexemes alongside calamine's value kinds;
/// callers must reject unknown operation/version before mapping the carrier.
pub fn decode_xlsx_cells(
    bytes: &[u8],
    source_name: &str,
    limits: XlsxDecodeLimits,
) -> DecodeResult<DecodedWorkbook> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(decode_error)?;
    let mut usage = XlsxDecodeUsage {
        archive_decoded_bytes: preflight(&mut archive, limits)?,
        shared_copy_bytes: 0,
    };
    let mut cache = HashMap::new();
    for name in archive.file_names() {
        let name = name.map_err(decode_error)?.into_owned();
        cache.insert(name.replace('\\', "/").to_ascii_lowercase(), name);
    }
    let mut workbook: Xlsx<_> = Xlsx::new(Cursor::new(bytes)).map_err(decode_error)?;
    // C5: worksheet overlay/resolution begins only after calamine initialization.
    let paths = sheet_paths(&mut archive, &cache)?;
    let sheet_names = workbook.sheet_names();
    if paths.iter().map(|(name, _)| name).ne(sheet_names.iter()) {
        return Err(decode_error(
            "ordered overlay sheet names differ from calamine",
        ));
    }
    let epoch_1904 = workbook.has_1904_epoch();
    let mut sheets = Vec::new();
    for (name, path) in paths {
        let mut reader = workbook
            .worksheet_cells_reader(&name)
            .map_err(decode_error)?;
        let mut overlay = OverlayReader::new(part_xml(&mut archive, &cache, &path)?)?;
        let mut cells = Vec::new();
        let mut positions = HashSet::new();
        let mut extent: Option<XlsxUsedExtent> = None;
        loop {
            let record = overlay.next_record()?;
            let cell = reader.next_cell().map_err(decode_error)?;
            let (mut record, cell) = match (record, cell) {
                (None, None) => break,
                (Some(record), Some(cell)) => (record, cell),
                _ => {
                    return Err(decode_error(format!(
                        "worksheet '{name}' overlay/calamine cell count mismatch"
                    )));
                }
            };
            let position = cell.get_position();
            if let Some(reference) = &record.reference {
                if explicit_position(reference)? != position {
                    return Err(decode_error(format!(
                        "worksheet '{name}' explicit cell reference disagrees with calamine"
                    )));
                }
            }
            if !positions.insert(position) {
                return Err(decode_error(format!(
                    "worksheet '{name}' duplicate cell coordinate {position:?}"
                )));
            }
            let value = typed_value(
                cell.get_value(),
                &mut record,
                epoch_1904,
                &mut usage,
                limits,
            )?;
            if value != XlsxCellValue::Empty {
                let height = u64::from(position.0) + 1;
                let width = u64::from(position.1) + 1;
                extent = Some(match extent {
                    Some(old) => XlsxUsedExtent {
                        height: old.height.max(height),
                        width: old.width.max(width),
                    },
                    None => XlsxUsedExtent { height, width },
                });
            }
            cells.push(DecodedCell {
                position,
                value,
                origin: if record.formula {
                    XlsxCellOrigin::FormulaCache
                } else {
                    XlsxCellOrigin::Literal
                },
                reference: record.reference,
                cell_type: record.cell_type,
            });
        }
        sheets.push(DecodedSheet {
            name,
            cells,
            used_extent: extent,
        });
    }
    Ok(DecodedWorkbook {
        operation: XLSX_CELLS_OPERATION.to_owned(),
        version: XLSX_CELLS_VERSION,
        source_name: source_name.to_owned(),
        source_sha256: Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        sheet_names,
        sheets,
        usage,
    })
}
