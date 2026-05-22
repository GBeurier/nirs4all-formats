use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, FormatProbe, Result, SignalType, SourceFile, SpectralArray, SpectralAxis,
    SpectralRecord,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::readers::util::{
    normalize_key, parse_number, read_bytes, record_from_signals, safe_signal_name,
    text_lossy_from_bytes,
};
use crate::Reader;

pub struct JcampReader;

impl Reader for JcampReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::jcamp"
    }

    fn sniff(&self, head: &[u8], _path: &Path) -> Option<FormatProbe> {
        let text = String::from_utf8_lossy(head);
        (text.contains("##JCAMP-DX=") || text.contains("##JCAMPDX=")).then(|| {
            FormatProbe::new(
                "jcamp-dx",
                self.name(),
                Confidence::Definite,
                "JCAMP-DX labeled-data records detected",
            )
        })
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let bytes = read_bytes(path)?;
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(
        &self,
        path: &Path,
        bytes: &[u8],
    ) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let (text, source) = text_lossy_from_bytes(path, bytes);
        if is_link_jcamp(&text) {
            let outcome = parse_link_jcamp_outcome(&text)?;
            return finalize_link_outcome(outcome, &source, self.name(), bytes, &text);
        }
        let chunks = top_level_jcamp_chunks(&text);
        if chunks.len() > 1 {
            let parent_id = compute_link_parent_id(bytes);
            let total = chunks.len();
            return chunks
                .iter()
                .enumerate()
                .map(|(index, chunk)| {
                    let mut parsed = parse_jcamp_text(chunk)?;
                    let relation = infer_link_relation(&parsed.metadata, index);
                    inject_link_metadata(
                        &mut parsed.metadata,
                        &parent_id,
                        index,
                        total,
                        Some(&relation),
                    );
                    // Keep the legacy index key so downstream tools that
                    // already filter on it still work.
                    parsed
                        .metadata
                        .insert("jcamp_block_index".to_string(), json!(index));
                    jcamp_record_from_parsed(parsed, source.clone(), self.name())
                })
                .collect();
        }
        Ok(vec![jcamp_record_from_parsed(
            parse_jcamp_text(&text)?,
            source,
            self.name(),
        )?])
    }
}

/// Outcome of parsing a top-level LINK block. Same-axis links collapse
/// into a single composite record (Ocean Optics flow); heterogeneous links
/// fan out one record per child so the caller can inspect each axis.
enum LinkOutcome {
    Composite(ParsedJcamp),
    Heterogeneous(Vec<ParsedJcamp>),
}

fn finalize_link_outcome(
    outcome: LinkOutcome,
    source: &SourceFile,
    reader: &'static str,
    bytes: &[u8],
    text: &str,
) -> Result<Vec<SpectralRecord>> {
    match outcome {
        LinkOutcome::Composite(parsed) => Ok(vec![jcamp_record_from_parsed(
            parsed,
            source.clone(),
            reader,
        )?]),
        LinkOutcome::Heterogeneous(blocks) => {
            let parent_id = compute_link_parent_id(bytes);
            let total = blocks.len();
            let ocean_block_titles = link_block_titles(text);
            blocks
                .into_iter()
                .enumerate()
                .map(|(index, mut parsed)| {
                    let title_hint = ocean_block_titles.get(index).map(String::as_str);
                    let relation =
                        infer_link_relation_with_hint(&parsed.metadata, index, title_hint);
                    inject_link_metadata(
                        &mut parsed.metadata,
                        &parent_id,
                        index,
                        total,
                        Some(&relation),
                    );
                    jcamp_record_from_parsed(parsed, source.clone(), reader)
                })
                .collect()
        }
    }
}

fn compute_link_parent_id(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    format!("jcamp-{:x}", digest)
        .chars()
        .take("jcamp-".len() + 16)
        .collect()
}

fn inject_link_metadata(
    metadata: &mut BTreeMap<String, Value>,
    parent_id: &str,
    index: usize,
    total: usize,
    relation: Option<&str>,
) {
    metadata.insert("link_parent_id".to_string(), json!(parent_id));
    metadata.insert("link_index".to_string(), json!(index));
    metadata.insert("link_total".to_string(), json!(total));
    if let Some(rel) = relation {
        metadata.insert("link_relation".to_string(), json!(rel));
    }
}

/// Map a block's `##DATA TYPE` (and optional Ocean Optics title hint)
/// to a canonical link relation: sample / dark / reference / interferogram
/// / fid / unknown.
fn infer_link_relation(metadata: &BTreeMap<String, Value>, _index: usize) -> String {
    infer_link_relation_with_hint(metadata, _index, None)
}

fn infer_link_relation_with_hint(
    metadata: &BTreeMap<String, Value>,
    _index: usize,
    title_hint: Option<&str>,
) -> String {
    if let Some(hint) = title_hint {
        if let Some(relation) = link_relation_from_string(hint) {
            return relation;
        }
    }
    // LDRs live under metadata["jcamp"] (single block) and metadata["jcamp_ldr"]
    // (heterogeneous LINK child); both keep the same flat `key -> value` shape.
    for slot in ["jcamp", "jcamp_ldr"] {
        if let Some(Value::Object(ldr)) = metadata.get(slot) {
            for key in ["data_type", "title"] {
                if let Some(Value::String(value)) = ldr.get(key) {
                    if let Some(relation) = link_relation_from_string(value) {
                        return relation;
                    }
                }
            }
        }
    }
    "unknown".to_string()
}

fn link_relation_from_string(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    if lower.contains("dark") {
        Some("dark".to_string())
    } else if lower.contains("reference") || lower.contains("white") {
        Some("reference".to_string())
    } else if lower.contains("interferogram") {
        Some("interferogram".to_string())
    } else if lower.contains("nmr fid") || lower.contains("free induction") {
        Some("fid".to_string())
    } else if lower.contains("peak table") || lower.contains("peak assignments") {
        Some("peaks".to_string())
    } else if lower.contains("spectrum") || lower.contains("sample") {
        Some("sample".to_string())
    } else {
        None
    }
}

fn link_block_titles(text: &str) -> Vec<String> {
    link_block_chunks(text)
        .iter()
        .map(|chunk| {
            chunk
                .lines()
                .find_map(|line| {
                    let trimmed = line.trim();
                    if let Some(body) = trimmed.strip_prefix("##") {
                        if let Some((key, value)) = body.split_once('=') {
                            if normalize_key(key) == "title" {
                                return Some(value.trim().to_string());
                            }
                        }
                    }
                    None
                })
                .unwrap_or_default()
        })
        .collect()
}

#[derive(Clone)]
struct ParsedJcamp {
    axis: Vec<f64>,
    axis_unit: String,
    axis_kind: AxisKind,
    signals: Vec<ParsedJcampSignal>,
    metadata: BTreeMap<String, Value>,
    warnings: Vec<String>,
}

#[derive(Clone)]
struct ParsedJcampSignal {
    name: String,
    values: Vec<f64>,
    signal_type: SignalType,
    signal_unit: Option<String>,
    role: String,
}

fn jcamp_record_from_parsed(
    parsed: ParsedJcamp,
    source: SourceFile,
    reader: &str,
) -> Result<SpectralRecord> {
    let mut signals = BTreeMap::new();
    for signal in parsed.signals {
        let axis = SpectralAxis::new(
            parsed.axis.clone(),
            parsed.axis_unit.clone(),
            parsed.axis_kind.clone(),
        )?;
        let array = SpectralArray::new(
            axis,
            signal.values,
            vec!["x".to_string()],
            signal.signal_type,
            signal.signal_unit,
            signal.role,
            "file",
        )?;
        signals.insert(signal.name, array);
    }
    let dominant = dominant_signal_type(&signals);
    record_from_signals(
        "jcamp-dx",
        reader,
        source,
        signals,
        dominant,
        parsed.metadata,
        parsed.warnings,
    )
}

fn parse_jcamp_text(text: &str) -> Result<ParsedJcamp> {
    if is_link_jcamp(text) {
        return parse_link_jcamp_text(text);
    }
    if has_peak_table(text) {
        return parse_peak_table_jcamp_text(text);
    }
    if text.lines().any(|line| {
        line.trim().strip_prefix("##").is_some_and(|body| {
            normalize_key(body.split_once('=').map_or(body, |(key, _)| key)) == "ntuples"
        })
    }) {
        return parse_ntuples_jcamp_text(text);
    }

    parse_xy_jcamp_text(text)
}

fn top_level_jcamp_chunks(text: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    for raw in text.lines() {
        if current.is_empty() && !raw.trim().to_ascii_uppercase().starts_with("##TITLE=") {
            continue;
        }
        current.push(raw.to_string());
        if raw.trim().to_ascii_uppercase().starts_with("##END=") {
            if current
                .iter()
                .any(|line| line.trim().to_ascii_uppercase().starts_with("##JCAMP-DX="))
            {
                chunks.push(current.join("\n"));
            }
            current.clear();
        }
    }
    chunks
}

fn parse_xy_jcamp_text(text: &str) -> Result<ParsedJcamp> {
    if has_peak_table(text) {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "JCAMP LINK child PEAK TABLE blocks are not supported; expected XYDATA or XYPOINTS"
                .to_string(),
        ));
    }

    let mut ldr = BTreeMap::<String, String>::new();
    let mut xy_lines = Vec::new();
    let mut in_xy = false;
    let mut has_xypoints = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("$$") || line.is_empty() {
            continue;
        }
        if let Some(body) = line.strip_prefix("##") {
            if body.to_ascii_uppercase().starts_with("END") {
                break;
            }
            in_xy = false;
            if let Some((key, value)) = body.split_once('=') {
                let normalized = normalize_key(key);
                if normalized == "xydata" || normalized == "xypoints" {
                    in_xy = true;
                    has_xypoints = normalized == "xypoints";
                }
                ldr.insert(normalized, value.trim().to_string());
            }
        } else if in_xy {
            xy_lines.push(line.to_string());
        }
    }

    let yfactor = get_f64(&ldr, "yfactor").unwrap_or(1.0);
    let xfactor = get_f64(&ldr, "xfactor").unwrap_or(1.0);

    let mut warnings = Vec::new();
    let (mut axis, mut values) = if has_xypoints {
        parse_xypoints_lines(&xy_lines, yfactor)?
    } else {
        let firstx = get_f64(&ldr, "firstx").ok_or_else(|| {
            nirs4all_io_core::Error::InvalidRecord("JCAMP missing FIRSTX".to_string())
        })?;
        let deltax = get_f64(&ldr, "deltax").unwrap_or_else(|| {
            let lastx = get_f64(&ldr, "lastx").unwrap_or(firstx);
            let npoints = get_f64(&ldr, "npoints").unwrap_or(1.0).max(1.0);
            if npoints <= 1.0 {
                1.0
            } else {
                (lastx - firstx) / (npoints - 1.0)
            }
        });
        let values = decode_xy_lines_with_x_checkpoints(
            &xy_lines,
            firstx,
            deltax,
            xfactor,
            yfactor,
            "signal",
            &mut warnings,
        );
        let axis: Vec<f64> = (0..values.len())
            .map(|idx| firstx + deltax * idx as f64)
            .collect();
        (axis, values)
    };
    if values.is_empty() {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "JCAMP XYDATA contains no decoded ordinate values; supported data tables are XYDATA and XYPOINTS"
                .to_string(),
        ));
    }

    if let Some(expected) = get_f64(&ldr, "npoints").map(|value| value as usize) {
        if values.len() > expected {
            warnings.push(format!(
                "npoints_truncated: declared {expected}, decoded {}",
                values.len()
            ));
            values.truncate(expected);
            axis.truncate(expected);
        } else if values.len() < expected {
            return Err(nirs4all_io_core::Error::InvalidRecord(format!(
                "JCAMP NPOINTS mismatch: declared {expected}, parsed {}",
                values.len()
            )));
        }
    }

    let xunits = ldr.get("xunits").map(String::as_str).unwrap_or("index");
    let (axis_kind, axis_unit) = axis_kind_unit(xunits);
    let yunits = ldr.get("yunits").map(String::as_str).unwrap_or("");
    let signal_type = signal_type_from_yunits(yunits);
    let mut metadata = BTreeMap::new();
    metadata.insert("jcamp".to_string(), json!(ldr));
    Ok(ParsedJcamp {
        axis,
        axis_unit,
        axis_kind,
        signals: vec![ParsedJcampSignal {
            name: "signal".to_string(),
            values,
            signal_type,
            signal_unit: (!yunits.is_empty()).then(|| yunits.to_string()),
            role: "signal".to_string(),
        }],
        metadata,
        warnings,
    })
}

fn is_link_jcamp(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        let Some(body) = line.strip_prefix("##") else {
            return false;
        };
        let Some((key, value)) = body.split_once('=') else {
            return false;
        };
        normalize_key(key) == "data_type" && value.trim().eq_ignore_ascii_case("LINK")
    })
}

fn has_peak_table(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        let Some(body) = line.strip_prefix("##") else {
            return false;
        };
        let Some((key, value)) = body.split_once('=') else {
            return false;
        };
        let normalized = normalize_key(key);
        matches!(
            normalized.as_str(),
            "peak_table" | "peaktable" | "peak_assignments" | "peakassignments"
        ) || (normalized == "data_table" && value.to_ascii_uppercase().contains("PEAK"))
            || (matches!(normalized.as_str(), "data_type" | "datatype")
                && value.to_ascii_uppercase().contains("PEAK TABLE"))
    })
}

/// Per-peak attribute slot inside a JCAMP-DX PEAK TABLE / PEAK ASSIGNMENTS shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeakField {
    X,
    Y,
    Width,
    Multiplicity,
    Assignment,
}

impl PeakField {
    fn as_str(&self) -> &'static str {
        match self {
            PeakField::X => "x",
            PeakField::Y => "y",
            PeakField::Width => "width",
            PeakField::Multiplicity => "multiplicity",
            PeakField::Assignment => "assignment",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeakTableKind {
    Table,
    Assignments,
}

impl PeakTableKind {
    fn as_str(&self) -> &'static str {
        match self {
            PeakTableKind::Table => "peak_table",
            PeakTableKind::Assignments => "peak_assignments",
        }
    }

    fn priority(&self) -> u8 {
        // Lower is preferred. ASSIGNMENTS carry more information than TABLE.
        match self {
            PeakTableKind::Assignments => 0,
            PeakTableKind::Table => 1,
        }
    }
}

#[derive(Debug)]
struct PeakTableBlock {
    kind: PeakTableKind,
    shape_raw: String,
    fields: Vec<PeakField>,
    packed: bool,
    lines: Vec<String>,
}

#[derive(Debug, Default, Clone)]
struct Peak {
    x: f64,
    y: Option<f64>,
    width: Option<f64>,
    multiplicity: Option<f64>,
    assignment: Option<String>,
}

impl Peak {
    fn to_json(&self) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("x".to_string(), json!(self.x));
        if let Some(y) = self.y {
            map.insert("y".to_string(), json!(y));
        }
        if let Some(w) = self.width {
            map.insert("width".to_string(), json!(w));
        }
        if let Some(m) = self.multiplicity {
            map.insert("multiplicity".to_string(), json!(m));
        }
        if let Some(a) = &self.assignment {
            map.insert("assignment".to_string(), json!(a));
        }
        Value::Object(map)
    }
}

fn parse_peak_table_jcamp_text(text: &str) -> Result<ParsedJcamp> {
    let ldr = collect_ldr(text);
    let mut blocks = collect_peak_table_blocks(text);
    if blocks.is_empty() {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "JCAMP PEAK TABLE block contains no parseable shape".to_string(),
        ));
    }

    // Prefer ASSIGNMENTS (richer) when multiple peak-style blocks are present.
    blocks.sort_by_key(|block| block.kind.priority());
    let primary = blocks.remove(0);
    let extras = blocks;

    let xfactor = get_f64(&ldr, "xfactor").unwrap_or(1.0);
    let yfactor = get_f64(&ldr, "yfactor").unwrap_or(1.0);
    let mut warnings = Vec::new();

    let peaks = decode_peak_block(&primary, xfactor, yfactor, &mut warnings);
    if peaks.is_empty() {
        return Err(nirs4all_io_core::Error::InvalidRecord(format!(
            "JCAMP {} block decoded no peaks",
            primary.kind.as_str()
        )));
    }

    if let Some(expected) = get_f64(&ldr, "npoints").map(|value| value as usize) {
        if expected != peaks.len() {
            return Err(nirs4all_io_core::Error::InvalidRecord(format!(
                "JCAMP {} NPOINTS mismatch: declared {expected}, parsed {}",
                primary.kind.as_str(),
                peaks.len()
            )));
        }
    }

    let axis: Vec<f64> = peaks.iter().map(|peak| peak.x).collect();
    let values: Vec<f64> = peaks.iter().map(|peak| peak.y.unwrap_or(0.0)).collect();
    let missing_y_count = peaks.iter().filter(|peak| peak.y.is_none()).count();
    if missing_y_count > 0 {
        warnings.push(format!(
            "jcamp_peak_table_missing_y: {missing_y_count} of {} peaks lacked an ordinate; filled with 0.0",
            peaks.len()
        ));
    }

    let xunits = ldr.get("xunits").map(String::as_str).unwrap_or("index");
    let (axis_kind, axis_unit) = axis_kind_unit(xunits);
    let yunits = ldr.get("yunits").map(String::as_str).unwrap_or("");

    let mut metadata = BTreeMap::new();
    metadata.insert("jcamp".to_string(), json!(ldr));
    metadata.insert(
        "jcamp_peak_table".to_string(),
        json!({
            "kind": primary.kind.as_str(),
            "shape": primary.shape_raw,
            "fields": primary.fields.iter().map(PeakField::as_str).collect::<Vec<_>>(),
            "packed": primary.packed,
            "sparse": true,
            "peak_count": peaks.len(),
            "peaks": peaks.iter().map(Peak::to_json).collect::<Vec<_>>(),
        }),
    );
    if !extras.is_empty() {
        warnings.push(format!(
            "jcamp_peak_table_multiple_blocks: kept {}, dropped {} secondary",
            primary.kind.as_str(),
            extras.len()
        ));
        metadata.insert(
            "jcamp_peak_table_dropped".to_string(),
            json!(extras
                .iter()
                .map(|block| json!({
                    "kind": block.kind.as_str(),
                    "shape": block.shape_raw,
                    "fields": block.fields.iter().map(PeakField::as_str).collect::<Vec<_>>(),
                    "packed": block.packed,
                    "line_count": block.lines.len(),
                }))
                .collect::<Vec<_>>()),
        );
    }

    Ok(ParsedJcamp {
        axis,
        axis_unit,
        axis_kind,
        signals: vec![ParsedJcampSignal {
            name: "peak_intensity".to_string(),
            values,
            signal_type: signal_type_from_yunits(yunits),
            signal_unit: (!yunits.is_empty()).then(|| yunits.to_string()),
            role: "peak_intensity".to_string(),
        }],
        metadata,
        warnings,
    })
}

fn collect_peak_table_blocks(text: &str) -> Vec<PeakTableBlock> {
    let mut blocks = Vec::<PeakTableBlock>::new();
    let mut current: Option<PeakTableBlock> = None;

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("$$") || line.is_empty() {
            continue;
        }
        if let Some(body) = line.strip_prefix("##") {
            if body.to_ascii_uppercase().starts_with("END") {
                if let Some(block) = current.take() {
                    blocks.push(block);
                }
                break;
            }
            if let Some(block) = current.take() {
                blocks.push(block);
            }
            let Some((key, value)) = body.split_once('=') else {
                continue;
            };
            let normalized = normalize_key(key);
            let value = strip_inline_comment(value).trim();
            if let Some((kind, shape_raw, fields, packed)) = peak_table_header(&normalized, value) {
                current = Some(PeakTableBlock {
                    kind,
                    shape_raw,
                    fields,
                    packed,
                    lines: Vec::new(),
                });
            }
        } else if let Some(block) = current.as_mut() {
            block.lines.push(line.to_string());
        }
    }
    if let Some(block) = current.take() {
        blocks.push(block);
    }
    blocks
}

/// Recognise a PEAK TABLE / PEAK ASSIGNMENTS / DATA TABLE header line and return
/// the structured shape parsed from its value (e.g. `(XY..XY)` -> `[X, Y]`, packed).
fn peak_table_header(
    normalized_key: &str,
    value: &str,
) -> Option<(PeakTableKind, String, Vec<PeakField>, bool)> {
    let kind = match normalized_key {
        "peak_table" | "peaktable" => PeakTableKind::Table,
        "peak_assignments" | "peakassignments" => PeakTableKind::Assignments,
        "data_table" => {
            let upper = value.to_ascii_uppercase();
            if !upper.contains("PEAK") {
                return None;
            }
            if upper.contains("ASSIGN") {
                PeakTableKind::Assignments
            } else {
                PeakTableKind::Table
            }
        }
        _ => return None,
    };
    let (fields, packed) = parse_peak_shape(value)?;
    Some((kind, value.to_string(), fields, packed))
}

/// Parse a JCAMP-DX peak-table shape token such as `(XY..XY)`, `(XYA)`, or
/// `(XYWA)`. Returns the per-peak field order and whether peaks are packed
/// (`..` syntax) into multiple peaks per line.
fn parse_peak_shape(raw: &str) -> Option<(Vec<PeakField>, bool)> {
    let trimmed = raw.trim();
    let inner = if let Some(start) = trimmed.find('(') {
        let after_open = &trimmed[start + 1..];
        if let Some(end) = after_open.find(')') {
            &after_open[..end]
        } else {
            after_open
        }
    } else {
        trimmed
    };
    let upper = inner.to_ascii_uppercase();
    let head = upper.split("..").next().unwrap_or("");
    let packed = upper.contains("..");
    let mut fields = Vec::new();
    for ch in head.chars() {
        if ch.is_whitespace() || ch == ',' {
            continue;
        }
        match ch {
            'X' => fields.push(PeakField::X),
            'Y' => fields.push(PeakField::Y),
            'W' => fields.push(PeakField::Width),
            'M' => fields.push(PeakField::Multiplicity),
            'A' => fields.push(PeakField::Assignment),
            _ => return None,
        }
    }
    if fields.is_empty() || !fields.contains(&PeakField::X) {
        return None;
    }
    Some((fields, packed))
}

fn decode_peak_block(
    block: &PeakTableBlock,
    xfactor: f64,
    yfactor: f64,
    warnings: &mut Vec<String>,
) -> Vec<Peak> {
    let has_assignment = block.fields.contains(&PeakField::Assignment);
    // Assignment text contains arbitrary characters; packing multiple peaks
    // per line with an assignment field is ambiguous, so we treat it as one
    // peak per line regardless of the `..` marker.
    let one_per_line = has_assignment || !block.packed;
    let mut peaks = Vec::new();
    let mut malformed_lines = 0usize;

    for line in &block.lines {
        if one_per_line {
            match decode_single_peak_line(line, &block.fields, xfactor, yfactor) {
                Some(peak) => peaks.push(peak),
                None => malformed_lines += 1,
            }
        } else {
            let line_peaks = decode_packed_peak_line(line, &block.fields, xfactor, yfactor);
            if line_peaks.is_empty() && line_has_numeric(line) {
                malformed_lines += 1;
            }
            peaks.extend(line_peaks);
        }
    }

    if malformed_lines > 0 {
        warnings.push(format!(
            "jcamp_peak_table_malformed_lines: {malformed_lines}"
        ));
    }
    peaks
}

fn decode_single_peak_line(
    line: &str,
    fields: &[PeakField],
    xfactor: f64,
    yfactor: f64,
) -> Option<Peak> {
    let (assignment, remainder) = extract_assignment(line);
    let numeric_tokens: Vec<f64> = remainder
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ';'))
        .filter_map(|token| {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                None
            } else {
                parse_number(trimmed)
            }
        })
        .collect();

    let mut peak = Peak::default();
    let mut have_x = false;
    let mut numeric_iter = numeric_tokens.into_iter();
    for field in fields {
        match field {
            PeakField::X => match numeric_iter.next() {
                Some(value) => {
                    peak.x = value * xfactor;
                    have_x = true;
                }
                None => return None,
            },
            PeakField::Y => {
                if let Some(value) = numeric_iter.next() {
                    peak.y = Some(value * yfactor);
                }
            }
            PeakField::Width => {
                if let Some(value) = numeric_iter.next() {
                    peak.width = Some(value * xfactor);
                }
            }
            PeakField::Multiplicity => {
                if let Some(value) = numeric_iter.next() {
                    peak.multiplicity = Some(value);
                }
            }
            PeakField::Assignment => {
                peak.assignment = assignment.clone();
            }
        }
    }
    have_x.then_some(peak)
}

fn decode_packed_peak_line(
    line: &str,
    fields: &[PeakField],
    xfactor: f64,
    yfactor: f64,
) -> Vec<Peak> {
    let numerics: Vec<f64> = line
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ';' | '(' | ')'))
        .filter_map(|token| {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                None
            } else {
                parse_number(trimmed)
            }
        })
        .collect();
    let group = fields.len();
    if group == 0 {
        return Vec::new();
    }
    let mut peaks = Vec::new();
    for chunk in numerics.chunks_exact(group) {
        let mut peak = Peak::default();
        let mut have_x = false;
        for (field, value) in fields.iter().zip(chunk.iter().copied()) {
            match field {
                PeakField::X => {
                    peak.x = value * xfactor;
                    have_x = true;
                }
                PeakField::Y => peak.y = Some(value * yfactor),
                PeakField::Width => peak.width = Some(value * xfactor),
                PeakField::Multiplicity => peak.multiplicity = Some(value),
                PeakField::Assignment => {
                    // Should never happen in packed mode (caller filters), but
                    // keep the arm defensive so adding a new field cannot
                    // silently drop data.
                }
            }
        }
        if have_x {
            peaks.push(peak);
        }
    }
    peaks
}

/// Extract the first JCAMP-DX assignment text enclosed in angle brackets and
/// return `(assignment, remainder)` where the remainder is the original line
/// with the matched `<...>` substring replaced by whitespace. JCAMP-DX 5.0
/// states assignment text is delimited by `<` and `>` and contains no nested
/// angle brackets, so we apply that rule literally and tolerate missing
/// brackets by returning `None` for the assignment.
fn extract_assignment(line: &str) -> (Option<String>, String) {
    let bytes = line.as_bytes();
    if let Some(open) = bytes.iter().position(|&b| b == b'<') {
        if let Some(rel_close) = bytes[open + 1..].iter().position(|&b| b == b'>') {
            let close = open + 1 + rel_close;
            let assignment = line[open + 1..close].trim().to_string();
            let mut remainder = String::with_capacity(line.len());
            remainder.push_str(&line[..open]);
            remainder.push(' ');
            remainder.push_str(&line[close + 1..]);
            let assignment = if assignment.is_empty() {
                None
            } else {
                Some(assignment)
            };
            return (assignment, remainder);
        }
    }
    (None, line.to_string())
}

fn line_has_numeric(line: &str) -> bool {
    line.split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ';' | '(' | ')'))
        .any(|token| parse_number(token.trim()).is_some())
}

fn parse_link_jcamp_text(text: &str) -> Result<ParsedJcamp> {
    match parse_link_jcamp_outcome(text)? {
        LinkOutcome::Composite(parsed) => Ok(parsed),
        LinkOutcome::Heterogeneous(_) => Err(nirs4all_io_core::Error::InvalidRecord(
            "JCAMP LINK child blocks have incompatible axes".to_string(),
        )),
    }
}

fn parse_link_jcamp_outcome(text: &str) -> Result<LinkOutcome> {
    let chunks = link_block_chunks(text);
    if chunks.is_empty() {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "JCAMP LINK contains no child blocks".to_string(),
        ));
    }

    let mut composite_axis: Option<(Vec<f64>, String, AxisKind)> = None;
    let mut signals = Vec::<ParsedJcampSignal>::new();
    let mut block_metadata = Vec::<BTreeMap<String, String>>::new();
    let mut composite_warnings = Vec::<String>::new();
    let mut child_blocks = Vec::<ParsedJcamp>::new();
    let mut heterogeneous = false;

    for (index, chunk) in chunks.iter().enumerate() {
        let ldr = collect_ldr(chunk);
        let parsed = parse_xy_jcamp_text(chunk)?;
        if parsed.signals.len() != 1 {
            return Err(nirs4all_io_core::Error::InvalidRecord(
                "JCAMP LINK child block did not decode to one signal".to_string(),
            ));
        }
        let mut next_parsed = ParsedJcamp {
            axis: parsed.axis.clone(),
            axis_unit: parsed.axis_unit.clone(),
            axis_kind: parsed.axis_kind.clone(),
            signals: parsed.signals.clone(),
            metadata: parsed.metadata.clone(),
            warnings: parsed.warnings.clone(),
        };
        child_blocks.push(parsed);

        if heterogeneous {
            // Fan-out branch: keep each child verbatim.
            block_metadata.push(ldr);
            continue;
        }

        match composite_axis.as_ref() {
            None => {
                composite_axis = Some((
                    next_parsed.axis.clone(),
                    next_parsed.axis_unit.clone(),
                    next_parsed.axis_kind.clone(),
                ));
            }
            Some((axis, _, _)) if same_axis(axis, &next_parsed.axis) => {}
            _ => {
                // Switch to fan-out mode.
                heterogeneous = true;
                composite_axis = None;
                signals.clear();
                composite_warnings.clear();
                block_metadata.push(ldr);
                continue;
            }
        }
        composite_warnings.append(&mut next_parsed.warnings);

        let mut signal = next_parsed.signals.into_iter().next().expect("checked");
        let name = link_signal_name(&ldr, index);
        let (signal_type, unit) = link_signal_type_and_unit(&name, &ldr);
        signal.name = name.clone();
        signal.role = name;
        signal.signal_type = signal_type;
        signal.signal_unit = unit;
        signals.push(signal);
        block_metadata.push(ldr);
    }

    if heterogeneous {
        // Re-attach LDR onto each child's metadata under `jcamp_ldr` so
        // callers can still inspect the parsed labelled-data records.
        let blocks = child_blocks
            .into_iter()
            .zip(block_metadata)
            .map(|(mut block, ldr)| {
                block.metadata.insert("jcamp_ldr".to_string(), json!(ldr));
                block
            })
            .collect();
        return Ok(LinkOutcome::Heterogeneous(blocks));
    }

    let (axis, axis_unit, axis_kind) =
        composite_axis.expect("composite path keeps an axis when not heterogeneous");

    if let Some((processed, undefined_count)) = compute_ocean_link_transmittance(&signals) {
        if undefined_count > 0 {
            composite_warnings.push(format!(
                "jcamp_link_processed_zero_denominator: {undefined_count} points set to 0"
            ));
        }
        signals.push(ParsedJcampSignal {
            name: "processed".to_string(),
            values: processed,
            signal_type: SignalType::Transmittance,
            signal_unit: Some("%".to_string()),
            role: "processed".to_string(),
        });
    }

    let mut metadata = BTreeMap::new();
    metadata.insert(
        "jcamp_link".to_string(),
        json!({
            "block_count": block_metadata.len(),
            "blocks": block_metadata,
        }),
    );
    Ok(LinkOutcome::Composite(ParsedJcamp {
        axis,
        axis_unit,
        axis_kind,
        signals,
        metadata,
        warnings: composite_warnings,
    }))
}

fn link_block_chunks(text: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    for raw in text.lines() {
        current.push(raw.to_string());
        if raw.trim().to_ascii_uppercase().starts_with("##END=") {
            if current
                .iter()
                .any(|line| line.trim().to_ascii_uppercase().starts_with("##JCAMP-DX="))
            {
                chunks.push(current.join("\n"));
            }
            current.clear();
        }
    }
    chunks
}

fn collect_ldr(text: &str) -> BTreeMap<String, String> {
    let mut ldr = BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        let Some(body) = line.strip_prefix("##") else {
            continue;
        };
        let Some((key, value)) = body.split_once('=') else {
            continue;
        };
        ldr.insert(
            normalize_key(key),
            strip_inline_comment(value).trim().to_string(),
        );
    }
    ldr
}

fn same_axis(left: &[f64], right: &[f64]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(a, b)| (a - b).abs() <= a.abs().max(b.abs()).max(1.0) * 1e-6)
}

fn link_signal_name(ldr: &BTreeMap<String, String>, index: usize) -> String {
    let title = ldr.get("title").map(String::as_str).unwrap_or("");
    let sample_description = ldr
        .get("sample_description")
        .map(String::as_str)
        .unwrap_or("");
    let origin = ldr.get("origin").map(String::as_str).unwrap_or("");
    let combined = format!("{title}\n{sample_description}").to_ascii_uppercase();
    if combined.contains("DARK") {
        "dark_reference".to_string()
    } else if combined.contains("REFERENCE") {
        "white_reference".to_string()
    } else if combined.contains("PROCESSED") && origin.eq_ignore_ascii_case("OCEANOPTICS EXPORT") {
        "sample".to_string()
    } else if combined.contains("PROCESSED") {
        "processed".to_string()
    } else {
        format!("signal_{}", index + 1)
    }
}

fn link_signal_type_and_unit(
    name: &str,
    ldr: &BTreeMap<String, String>,
) -> (SignalType, Option<String>) {
    if matches!(name, "sample" | "dark_reference" | "white_reference") {
        return (SignalType::RawCounts, None);
    }
    let yunits = ldr.get("yunits").map(String::as_str).unwrap_or("");
    let signal_type = signal_type_from_yunits(yunits);
    let unit = if yunits.trim().is_empty() {
        None
    } else {
        Some(yunits.to_string())
    };
    (signal_type, unit)
}

fn compute_ocean_link_transmittance(signals: &[ParsedJcampSignal]) -> Option<(Vec<f64>, usize)> {
    let sample = signals.iter().find(|signal| signal.name == "sample")?;
    let dark = signals
        .iter()
        .find(|signal| signal.name == "dark_reference")?;
    let white = signals
        .iter()
        .find(|signal| signal.name == "white_reference")?;
    if sample.values.len() != dark.values.len() || sample.values.len() != white.values.len() {
        return None;
    }

    let mut undefined_count = 0usize;
    let processed = sample
        .values
        .iter()
        .zip(&dark.values)
        .zip(&white.values)
        .map(|((sample, dark), white)| {
            let denominator = white - dark;
            if denominator.abs() <= f64::EPSILON {
                undefined_count += 1;
                0.0
            } else {
                (sample - dark) / denominator * 100.0
            }
        })
        .collect();
    Some((processed, undefined_count))
}

struct NtuplePage {
    descriptor: String,
    data_symbol: Option<String>,
    lines: Vec<String>,
}

fn parse_ntuples_jcamp_text(text: &str) -> Result<ParsedJcamp> {
    let mut ldr = BTreeMap::<String, String>::new();
    let mut pages = Vec::<NtuplePage>::new();
    let mut current_page: Option<NtuplePage> = None;
    let mut in_data = false;

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("$$") || line.is_empty() {
            continue;
        }
        if let Some(body) = line.strip_prefix("##") {
            let upper = body.to_ascii_uppercase();
            if upper.starts_with("END") {
                if let Some(page) = current_page.take() {
                    if !page.lines.is_empty() {
                        pages.push(page);
                    }
                }
                if upper.starts_with("END=") {
                    break;
                }
                in_data = false;
                continue;
            }

            in_data = false;
            if let Some((key, value)) = body.split_once('=') {
                let normalized = normalize_key(key);
                let value = strip_inline_comment(value).trim().to_string();
                if normalized == "page" {
                    if let Some(page) = current_page.take() {
                        if !page.lines.is_empty() {
                            pages.push(page);
                        }
                    }
                    current_page = Some(NtuplePage {
                        descriptor: value.clone(),
                        data_symbol: None,
                        lines: Vec::new(),
                    });
                } else if normalized == "data_table" {
                    if current_page.is_none() {
                        current_page = Some(NtuplePage {
                            descriptor: String::new(),
                            data_symbol: None,
                            lines: Vec::new(),
                        });
                    }
                    if let Some(page) = current_page.as_mut() {
                        page.data_symbol = data_symbol_from_table(&value);
                    }
                    in_data = true;
                }
                ldr.insert(normalized, value);
            }
        } else if in_data {
            if let Some(page) = current_page.as_mut() {
                page.lines.push(line.to_string());
            }
        }
    }
    if let Some(page) = current_page.take() {
        if !page.lines.is_empty() {
            pages.push(page);
        }
    }

    if pages.is_empty() {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "JCAMP NTUPLES contains no DATA TABLE pages".to_string(),
        ));
    }

    let var_names = ldr_list(&ldr, "var_name");
    let symbols = ldr_list(&ldr, "symbol");
    let var_types = ldr_list(&ldr, "var_type");
    let var_dims = ldr_list(&ldr, "var_dim");
    let units = ldr_list(&ldr, "units");
    let first = ldr_list(&ldr, "first");
    let last = ldr_list(&ldr, "last");
    let factors = ldr_list(&ldr, "factor");

    let x_index = var_types
        .iter()
        .position(|value| value.to_ascii_uppercase().contains("INDEPENDENT"))
        .or_else(|| {
            symbols
                .iter()
                .position(|symbol| symbol.eq_ignore_ascii_case("X"))
        })
        .unwrap_or(0);

    let npoints = parse_list_number(&var_dims, x_index)
        .map(|value| value as usize)
        .unwrap_or_else(|| {
            pages
                .iter()
                .map(|page| page.lines.len())
                .max()
                .unwrap_or(1)
                .max(1)
        });
    let firstx = parse_list_number(&first, x_index).ok_or_else(|| {
        nirs4all_io_core::Error::InvalidRecord("JCAMP NTUPLES missing FIRST axis".to_string())
    })?;
    let lastx = parse_list_number(&last, x_index).unwrap_or(firstx + npoints as f64 - 1.0);
    let step = if npoints <= 1 {
        1.0
    } else {
        (lastx - firstx) / (npoints - 1) as f64
    };
    let axis: Vec<f64> = (0..npoints).map(|idx| firstx + step * idx as f64).collect();

    let xunit_raw = units.get(x_index).map(String::as_str).unwrap_or("index");
    let (axis_kind, axis_unit) = axis_kind_unit(xunit_raw);

    let mut warnings = Vec::new();
    let mut parsed_signals = Vec::new();
    for (page_index, page) in pages.iter().enumerate() {
        let Some(signal_index) = ntuple_page_signal_index(page, &symbols, &var_types, page_index)
        else {
            warnings.push(format!("jcamp_ntuples_unmapped_page: {}", page.descriptor));
            continue;
        };
        let yfactor = parse_list_number(&factors, signal_index).unwrap_or(1.0);
        let mut values = decode_xy_lines(
            &page.lines,
            yfactor,
            page.data_symbol.as_deref().unwrap_or("page"),
            &mut warnings,
        );
        if values.len() > npoints {
            warnings.push(format!(
                "jcamp_ntuples_npoints_truncated: page {} declared {npoints}, decoded {}",
                page.data_symbol.as_deref().unwrap_or("?"),
                values.len()
            ));
            values.truncate(npoints);
        } else if values.len() < npoints {
            warnings.push(format!(
                "jcamp_ntuples_npoints_mismatch: page {} declared {npoints}, parsed {}",
                page.data_symbol.as_deref().unwrap_or("?"),
                values.len()
            ));
        }
        if values.is_empty() {
            continue;
        }

        let signal_name = var_names
            .get(signal_index)
            .map(|name| ntuple_signal_name(name, symbols.get(signal_index).map(String::as_str)))
            .unwrap_or_else(|| format!("signal_{}", page_index + 1));
        let signal_unit = units
            .get(signal_index)
            .map(String::as_str)
            .filter(|unit| !unit.trim().is_empty())
            .map(|unit| unit.to_string());
        parsed_signals.push(ParsedJcampSignal {
            name: signal_name.clone(),
            values,
            signal_type: signal_type_from_yunits(
                units
                    .get(signal_index)
                    .map(String::as_str)
                    .unwrap_or(signal_name.as_str()),
            ),
            signal_unit,
            role: signal_name,
        });
    }

    if parsed_signals.is_empty() {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "JCAMP NTUPLES contains no decoded signal pages".to_string(),
        ));
    }

    let mut metadata = BTreeMap::new();
    metadata.insert("jcamp".to_string(), json!(ldr));
    Ok(ParsedJcamp {
        axis,
        axis_unit,
        axis_kind,
        signals: parsed_signals,
        metadata,
        warnings,
    })
}

fn decode_xy_lines(
    xy_lines: &[String],
    yfactor: f64,
    context: &str,
    warnings: &mut Vec<String>,
) -> Vec<f64> {
    let mut values = Vec::new();
    let mut last_raw_y: Option<f64> = None;
    for line in xy_lines {
        let has_dif = xy_line_has_dif(line);
        let mut line_values = values_from_xy_line(line);
        if has_dif && last_raw_y.is_some() && !line_values.is_empty() {
            let checkpoint = line_values.remove(0);
            if let Some(previous) = last_raw_y {
                let tolerance = previous.abs().max(1.0) * 1e-6;
                if (checkpoint - previous).abs() > tolerance {
                    warnings.push(format!(
                        "jcamp_dif_checkpoint_mismatch: {context} previous {previous}, checkpoint {checkpoint}"
                    ));
                }
            }
        }
        for raw_y in line_values {
            last_raw_y = Some(raw_y);
            values.push(raw_y * yfactor);
        }
    }
    values
}

fn decode_xy_lines_with_x_checkpoints(
    xy_lines: &[String],
    firstx: f64,
    deltax: f64,
    xfactor: f64,
    yfactor: f64,
    context: &str,
    warnings: &mut Vec<String>,
) -> Vec<f64> {
    let mut values = Vec::new();
    let mut last_raw_y: Option<f64> = None;
    let mut x_scale_mode = None;
    let mut previous_x_checkpoint = None;
    let mut checked_x = 0usize;
    let mut mismatched_x = 0usize;
    let mut first_mismatch = None;

    for line in xy_lines {
        let line_x = xy_line_start_x(line);
        let has_dif = xy_line_has_dif(line);
        let mut line_values = values_from_xy_line(line);
        if has_dif && last_raw_y.is_some() && !line_values.is_empty() {
            let checkpoint = line_values.remove(0);
            if let Some(previous) = last_raw_y {
                let tolerance = previous.abs().max(1.0) * 1e-6;
                if (checkpoint - previous).abs() > tolerance {
                    warnings.push(format!(
                        "jcamp_dif_checkpoint_mismatch: {context} previous {previous}, checkpoint {checkpoint}"
                    ));
                }
            }
        }
        let emitted_count = line_values.len();
        if let Some(line_x) = line_x {
            checked_x += 1;
            let expected_x = expected_x_checkpoints(firstx, deltax, previous_x_checkpoint);
            let evaluation =
                evaluate_x_checkpoint(line_x, xfactor, x_scale_mode, expected_x, deltax);
            x_scale_mode.get_or_insert(evaluation.scale);
            if !evaluation.matched {
                mismatched_x += 1;
                let closest_expected_x = closest_expected_x(evaluation.physical_x, expected_x);
                first_mismatch.get_or_insert((line_x, closest_expected_x));
            }
            previous_x_checkpoint = Some((evaluation.physical_x, emitted_count));
        }
        for raw_y in line_values {
            last_raw_y = Some(raw_y);
            values.push(raw_y * yfactor);
        }
    }

    if mismatched_x > 0 {
        let (line_x, expected_x) = first_mismatch.expect("mismatch count implies example");
        let abs_delta = (line_x - expected_x).abs();
        let scale = expected_x.abs().max(line_x.abs()).max(f64::MIN_POSITIVE);
        let rel_delta = abs_delta / scale;
        warnings.push(format!(
            "jcamp_xydata_x_checkpoint_drift: {context} {mismatched_x}/{checked_x} line starts mismatched; \
first line_x {line_x}, expected {expected_x}, abs={abs_delta:.6e}, rel={rel_delta:.6e}"
        ));
    }

    values
}

fn xy_line_start_x(line: &str) -> Option<f64> {
    let (raw_x, _) = split_xy_line(line)?;
    parse_number(raw_x)
}

fn expected_x_checkpoints(
    firstx: f64,
    deltax: f64,
    previous_x_checkpoint: Option<(f64, usize)>,
) -> [Option<f64>; 2] {
    if let Some((previous_x, previous_count)) = previous_x_checkpoint {
        [
            Some(previous_x + deltax * previous_count as f64),
            (previous_count > 0).then(|| previous_x + deltax * (previous_count - 1) as f64),
        ]
    } else {
        [Some(firstx), None]
    }
}

fn evaluate_x_checkpoint(
    line_x: f64,
    xfactor: f64,
    scale_mode: Option<XCheckpointScale>,
    expected_x: [Option<f64>; 2],
    deltax: f64,
) -> XCheckpointEvaluation {
    if let Some(scale_mode) = scale_mode {
        return evaluate_x_checkpoint_scale(line_x, xfactor, scale_mode, expected_x, deltax);
    }

    let raw =
        evaluate_x_checkpoint_scale(line_x, xfactor, XCheckpointScale::Raw, expected_x, deltax);
    if xfactor == 1.0 {
        return raw;
    }

    let scaled = evaluate_x_checkpoint_scale(
        line_x,
        xfactor,
        XCheckpointScale::ScaledByXfactor,
        expected_x,
        deltax,
    );
    if scaled.error < raw.error {
        scaled
    } else {
        raw
    }
}

fn evaluate_x_checkpoint_scale(
    line_x: f64,
    xfactor: f64,
    scale: XCheckpointScale,
    expected_x: [Option<f64>; 2],
    deltax: f64,
) -> XCheckpointEvaluation {
    let physical_x = match scale {
        XCheckpointScale::Raw => line_x,
        XCheckpointScale::ScaledByXfactor => line_x * xfactor,
    };
    let error = expected_x
        .iter()
        .flatten()
        .map(|expected| (physical_x - *expected).abs())
        .fold(f64::INFINITY, f64::min);
    let matched = expected_x
        .iter()
        .flatten()
        .any(|expected| nearly_equal_x(physical_x, *expected, deltax));
    XCheckpointEvaluation {
        physical_x,
        scale,
        error,
        matched,
    }
}

fn closest_expected_x(physical_x: f64, expected_x: [Option<f64>; 2]) -> f64 {
    expected_x
        .iter()
        .flatten()
        .min_by(|left, right| {
            let left_error = (physical_x - **left).abs();
            let right_error = (physical_x - **right).abs();
            left_error.total_cmp(&right_error)
        })
        .copied()
        .unwrap_or(physical_x)
}

fn nearly_equal_x(left: f64, right: f64, deltax: f64) -> bool {
    let relative = right.abs().max(left.abs()).max(1.0) * 1e-5;
    let line_rounding = deltax.abs() * 2.0;
    (left - right).abs() <= relative.max(line_rounding)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum XCheckpointScale {
    Raw,
    ScaledByXfactor,
}

#[derive(Clone, Copy)]
struct XCheckpointEvaluation {
    physical_x: f64,
    scale: XCheckpointScale,
    error: f64,
    matched: bool,
}

fn get_f64(map: &BTreeMap<String, String>, key: &str) -> Option<f64> {
    map.get(key).and_then(|value| parse_number(value))
}

fn ldr_list(map: &BTreeMap<String, String>, key: &str) -> Vec<String> {
    map.get(key)
        .map(|value| {
            value
                .split(',')
                .map(|part| part.trim().trim_matches('"').to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn parse_list_number(values: &[String], index: usize) -> Option<f64> {
    values.get(index).and_then(|value| parse_number(value))
}

fn strip_inline_comment(value: &str) -> &str {
    value.split_once("$$").map_or(value, |(head, _)| head)
}

fn data_symbol_from_table(value: &str) -> Option<String> {
    let start = value.find("++(")? + 3;
    let mut symbol = String::new();
    for ch in value[start..].chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            symbol.push(ch);
        } else if !symbol.is_empty() {
            break;
        }
    }
    (!symbol.is_empty()).then_some(symbol)
}

fn ntuple_page_signal_index(
    page: &NtuplePage,
    symbols: &[String],
    var_types: &[String],
    page_index: usize,
) -> Option<usize> {
    if let Some(symbol) = page.data_symbol.as_deref() {
        if let Some(index) = symbols
            .iter()
            .position(|candidate| candidate.eq_ignore_ascii_case(symbol))
        {
            return Some(index);
        }
    }
    var_types
        .iter()
        .enumerate()
        .filter(|(_, var_type)| {
            let upper = var_type.to_ascii_uppercase();
            !upper.contains("INDEPENDENT") && !upper.contains("PAGE")
        })
        .map(|(index, _)| index)
        .nth(page_index)
}

fn ntuple_signal_name(var_name: &str, symbol: Option<&str>) -> String {
    let lower = var_name.to_ascii_lowercase();
    if lower.contains("real") {
        "real".to_string()
    } else if lower.contains("imag") {
        "imaginary".to_string()
    } else {
        safe_signal_name(var_name, symbol.unwrap_or("signal"))
    }
}

fn values_from_xy_line(line: &str) -> Vec<f64> {
    let Some((_, rest)) = split_xy_line(line) else {
        return Vec::new();
    };
    decode_asdf_values(rest)
}

fn parse_xypoints_lines(lines: &[String], yfactor: f64) -> Result<(Vec<f64>, Vec<f64>)> {
    let mut axis = Vec::new();
    let mut values = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with("$$") {
            continue;
        }
        let fields = if line.contains(',') {
            line.split(',').collect::<Vec<_>>()
        } else {
            line.split_whitespace().collect::<Vec<_>>()
        };
        if fields.len() < 2 {
            continue;
        }
        let Some(x) = parse_number(fields[0]) else {
            continue;
        };
        let Some(y) = parse_number(fields[1]) else {
            continue;
        };
        axis.push(x);
        values.push(y * yfactor);
    }
    if axis.is_empty() {
        return Err(nirs4all_io_core::Error::InvalidRecord(
            "JCAMP XYPOINTS contains no numeric XY pairs".to_string(),
        ));
    }
    Ok((axis, values))
}

fn xy_line_has_dif(line: &str) -> bool {
    let Some((_, rest)) = split_xy_line(line) else {
        return false;
    };
    rest.chars().any(|ch| dif_digit(ch).is_some())
}

fn split_xy_line(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    if let Some(first) = line.split_whitespace().next() {
        if parse_number(first).is_some() {
            return Some((first, line[first.len()..].trim_start()));
        }
    }

    let mut end = 0usize;
    for (index, ch) in line.char_indices() {
        if index == 0 && matches!(ch, '+' | '-') {
            end = ch.len_utf8();
            continue;
        }
        if ch.is_ascii_digit() || matches!(ch, '.') {
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }
    (end > 0).then(|| line.split_at(end))
}

fn decode_asdf_values(input: &str) -> Vec<f64> {
    let mut values = Vec::new();
    let mut current: Option<i64> = None;
    let mut last_difference: i64 = 0;
    let mut index = 0usize;
    let bytes = input.as_bytes();

    while index < bytes.len() {
        let ch = input[index..].chars().next().expect("valid char boundary");
        if ch.is_whitespace() || ch == ',' {
            index += ch.len_utf8();
            continue;
        }

        if let Some(first_digit) = sqz_digit(ch) {
            let (number, next) = asdf_number(input, index, first_digit);
            current = Some(number);
            values.push(number as f64);
            index = next;
        } else if let Some(first_digit) = dif_digit(ch) {
            let (difference, next) = asdf_number(input, index, first_digit);
            let next_value = current.unwrap_or(0) + difference;
            current = Some(next_value);
            last_difference = difference;
            values.push(next_value as f64);
            index = next;
        } else if let Some(first_digit) = dup_digit(ch) {
            let (count, next) = asdf_unsigned_number(input, index, first_digit);
            if let Some(mut value) = current {
                for _ in 0..count.saturating_sub(1) {
                    value += last_difference;
                    values.push(value as f64);
                }
                current = Some(value);
            }
            index = next;
        } else if ch.is_ascii_digit() || matches!(ch, '+' | '-' | '.') {
            let (token, next) = ordinary_number_token(input, index);
            if let Some(value) = parse_number(token) {
                let int_value = value as i64;
                current = Some(int_value);
                values.push(value);
            }
            index = next;
        } else {
            index += ch.len_utf8();
        }
    }

    values
}

#[cfg(test)]
fn numbers_from_token(token: &str) -> Vec<f64> {
    if parse_number(token).is_some() {
        return vec![parse_number(token).expect("checked")];
    }
    let mut out = Vec::new();
    let mut start = 0usize;
    let bytes = token.as_bytes();
    for index in 1..bytes.len() {
        if bytes[index] == b'+' || bytes[index] == b'-' {
            if let Some(value) = parse_number(&token[start..index]) {
                out.push(value);
            }
            start = index;
        }
    }
    if start < token.len() {
        if let Some(value) = parse_number(&token[start..]) {
            out.push(value);
        }
    }
    out
}

fn asdf_number(input: &str, offset: usize, first_digit: i64) -> (i64, usize) {
    let (digits, next) = following_digits(input, offset);
    let sign = if first_digit < 0 { -1 } else { 1 };
    let magnitude =
        first_digit.abs() * 10_i64.pow(digits.len() as u32) + digits.parse::<i64>().unwrap_or(0);
    (sign * magnitude, next)
}

fn asdf_unsigned_number(input: &str, offset: usize, first_digit: i64) -> (usize, usize) {
    let (digits, next) = following_digits(input, offset);
    let value = first_digit * 10_i64.pow(digits.len() as u32) + digits.parse::<i64>().unwrap_or(0);
    (value.max(0) as usize, next)
}

fn following_digits(input: &str, offset: usize) -> (&str, usize) {
    let start = offset + input[offset..].chars().next().expect("char").len_utf8();
    let mut end = start;
    for (relative, ch) in input[start..].char_indices() {
        if ch.is_ascii_digit() {
            end = start + relative + ch.len_utf8();
        } else {
            break;
        }
    }
    (&input[start..end], end)
}

fn ordinary_number_token(input: &str, offset: usize) -> (&str, usize) {
    let mut end = offset;
    let mut seen_exponent = false;
    let mut previous_was_exponent = false;
    for (relative, ch) in input[offset..].char_indices() {
        if relative == 0 && matches!(ch, '+' | '-') {
            end = offset + ch.len_utf8();
            continue;
        }
        if ch.is_ascii_digit() || matches!(ch, '.') {
            end = offset + relative + ch.len_utf8();
            previous_was_exponent = false;
        } else if !seen_exponent && matches!(ch, 'E' | 'e') {
            seen_exponent = true;
            previous_was_exponent = true;
            end = offset + relative + ch.len_utf8();
        } else if previous_was_exponent && matches!(ch, '+' | '-') {
            previous_was_exponent = false;
            end = offset + relative + ch.len_utf8();
        } else {
            break;
        }
    }
    (&input[offset..end], end)
}

fn sqz_digit(ch: char) -> Option<i64> {
    match ch {
        '@' => Some(0),
        'A'..='I' => Some(ch as i64 - 'A' as i64 + 1),
        'a'..='i' => Some(-(ch as i64 - 'a' as i64 + 1)),
        _ => None,
    }
}

fn dif_digit(ch: char) -> Option<i64> {
    match ch {
        '%' => Some(0),
        'J'..='R' => Some(ch as i64 - 'J' as i64 + 1),
        'j'..='r' => Some(-(ch as i64 - 'j' as i64 + 1)),
        _ => None,
    }
}

fn dup_digit(ch: char) -> Option<i64> {
    match ch {
        'S'..='Z' => Some(ch as i64 - 'S' as i64 + 1),
        's' => Some(9),
        _ => None,
    }
}

fn axis_kind_unit(raw: &str) -> (AxisKind, String) {
    let upper = raw.trim().to_ascii_uppercase();
    if upper.contains("1/CM") || upper.contains("CM-1") || upper.contains("WAVENUMBER") {
        (AxisKind::Wavenumber, "cm-1".to_string())
    } else if upper.contains("MICROM") || upper == "UM" {
        (AxisKind::Wavelength, "um".to_string())
    } else if upper.contains("NM") || upper.contains("NANOM") {
        (AxisKind::Wavelength, "nm".to_string())
    } else if upper.contains("HZ") {
        (AxisKind::Frequency, "hz".to_string())
    } else if upper.contains("EV") {
        (AxisKind::Energy, "eV".to_string())
    } else if upper.contains("SECOND") || upper == "S" {
        (AxisKind::Time, "s".to_string())
    } else {
        (AxisKind::Index, "index".to_string())
    }
}

fn signal_type_from_yunits(raw: &str) -> SignalType {
    let upper = raw.trim().to_ascii_uppercase();
    if upper.contains("ABS") {
        SignalType::Absorbance
    } else if upper.contains("TRANSM") {
        SignalType::Transmittance
    } else if upper.contains("REFLECT") {
        SignalType::Reflectance
    } else {
        SignalType::Unknown
    }
}

fn dominant_signal_type(signals: &BTreeMap<String, SpectralArray>) -> SignalType {
    for preferred in [
        SignalType::Absorbance,
        SignalType::Reflectance,
        SignalType::Transmittance,
        SignalType::Irradiance,
    ] {
        if signals
            .values()
            .any(|signal| signal.signal_type == preferred)
        {
            return preferred;
        }
    }
    signals
        .values()
        .next()
        .map(|signal| signal.signal_type.clone())
        .unwrap_or(SignalType::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_signed_packed_affn_tokens() {
        assert_eq!(
            numbers_from_token("+10160+10159-12"),
            vec![10160.0, 10159.0, -12.0]
        );
    }

    #[test]
    fn decodes_squeeze_difference_and_duplicate_values() {
        assert_eq!(
            decode_asdf_values("C1276%Sj05Sl3"),
            vec![31_276.0, 31_276.0, 31_171.0, 31_138.0]
        );
        assert_eq!(
            decode_asdf_values("B254931p506547"),
            vec![2_254_931.0, -5_251_616.0]
        );
    }

    #[test]
    fn parses_packed_peak_table_xy_pairs_with_yfactor() {
        let parsed = parse_jcamp_text(
            "##JCAMP-DX=5.00
##DATA TYPE=INFRARED PEAK TABLE
##XUNITS=1/CM
##YUNITS=ABSORBANCE
##YFACTOR=0.5
##NPOINTS=4
##PEAK TABLE=(XY..XY)
4000.0,2.0; 3500.0,4.0
3000.0,6.0
2500.0,8.0
##END=
",
        )
        .expect("parse peak table");

        assert_eq!(parsed.axis, vec![4000.0, 3500.0, 3000.0, 2500.0]);
        assert_eq!(parsed.signals.len(), 1);
        assert_eq!(parsed.signals[0].name, "peak_intensity");
        assert_eq!(parsed.signals[0].values, vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(parsed.axis_unit, "cm-1");
        assert_eq!(parsed.axis_kind, AxisKind::Wavenumber);
        assert_eq!(parsed.signals[0].signal_type, SignalType::Absorbance);

        let table_meta = parsed
            .metadata
            .get("jcamp_peak_table")
            .expect("jcamp_peak_table metadata");
        assert_eq!(table_meta["kind"], json!("peak_table"));
        assert_eq!(table_meta["packed"], json!(true));
        assert_eq!(table_meta["fields"], json!(["x", "y"]));
        assert_eq!(table_meta["peak_count"], json!(4));
        let peaks = table_meta["peaks"].as_array().expect("peaks array");
        assert_eq!(peaks.len(), 4);
        assert_eq!(peaks[0]["x"], json!(4000.0));
        assert_eq!(peaks[0]["y"], json!(1.0));
    }

    #[test]
    fn parses_data_table_peak_table_shape_with_suffix() {
        let parsed = parse_jcamp_text(
            "##JCAMP-DX=5.00
##DATA TYPE=INFRARED PEAK TABLE
##XUNITS=1/CM
##YUNITS=ABSORBANCE
##NPOINTS=2
##DATA TABLE=(XY..XY), PEAK TABLE
3300,0.4; 1700,0.9
##END=
",
        )
        .expect("parse data table peak header");

        assert_eq!(parsed.axis, vec![3300.0, 1700.0]);
        assert_eq!(parsed.signals[0].values, vec![0.4, 0.9]);
        let table_meta = parsed
            .metadata
            .get("jcamp_peak_table")
            .expect("jcamp_peak_table metadata");
        assert_eq!(table_meta["kind"], json!("peak_table"));
        assert_eq!(table_meta["fields"], json!(["x", "y"]));
        assert_eq!(table_meta["packed"], json!(true));
    }

    #[test]
    fn parses_peak_assignments_with_angle_bracket_text() {
        let parsed = parse_jcamp_text(
            "##JCAMP-DX=5.00
##DATA TYPE=INFRARED PEAK ASSIGNMENTS
##XUNITS=1/CM
##YUNITS=ABSORBANCE
##NPOINTS=3
##PEAK ASSIGNMENTS=(XYA)
3300, 0.42 <O-H stretch, broad>
2950, 0.18 <C-H stretch>
1650, 0.85 <C=C stretch>
##END=
",
        )
        .expect("parse peak assignments");

        assert_eq!(parsed.axis, vec![3300.0, 2950.0, 1650.0]);
        assert_eq!(parsed.signals[0].values, vec![0.42, 0.18, 0.85]);

        let table_meta = parsed
            .metadata
            .get("jcamp_peak_table")
            .expect("jcamp_peak_table metadata");
        assert_eq!(table_meta["kind"], json!("peak_assignments"));
        assert_eq!(table_meta["packed"], json!(false));
        assert_eq!(table_meta["fields"], json!(["x", "y", "assignment"]));
        let peaks = table_meta["peaks"].as_array().expect("peaks array");
        assert_eq!(peaks[0]["assignment"], json!("O-H stretch, broad"));
        assert_eq!(peaks[1]["assignment"], json!("C-H stretch"));
        assert_eq!(peaks[2]["assignment"], json!("C=C stretch"));
    }

    #[test]
    fn parses_peak_assignments_xywa_with_width_and_multiplicity_uses_xfactor() {
        let parsed = parse_jcamp_text(
            "##JCAMP-DX=5.00
##DATA TYPE=NMR PEAK ASSIGNMENTS
##XUNITS=HZ
##YUNITS=ARBITRARY
##XFACTOR=2.0
##YFACTOR=10.0
##NPOINTS=2
##PEAK ASSIGNMENTS=(XYWA)
100.0 5.0 0.5 <peak alpha>
200.0 2.5 1.0 <peak beta>
##END=
",
        )
        .expect("parse XYWA assignments");

        assert_eq!(parsed.axis, vec![200.0, 400.0]);
        assert_eq!(parsed.signals[0].values, vec![50.0, 25.0]);

        let table_meta = parsed
            .metadata
            .get("jcamp_peak_table")
            .expect("jcamp_peak_table metadata");
        assert_eq!(
            table_meta["fields"],
            json!(["x", "y", "width", "assignment"])
        );
        let peaks = table_meta["peaks"].as_array().expect("peaks array");
        assert_eq!(peaks[0]["width"], json!(1.0));
        assert_eq!(peaks[1]["width"], json!(2.0));
        assert_eq!(peaks[0]["assignment"], json!("peak alpha"));
    }

    #[test]
    fn parses_peak_table_xyw_packed_triples() {
        let parsed = parse_jcamp_text(
            "##JCAMP-DX=5.00
##DATA TYPE=INFRARED PEAK TABLE
##XUNITS=1/CM
##YUNITS=ABSORBANCE
##NPOINTS=2
##PEAK TABLE=(XYW..XYW)
3300 0.4 25; 1700 0.9 12
##END=
",
        )
        .expect("parse XYW packed");

        assert_eq!(parsed.axis, vec![3300.0, 1700.0]);
        let table_meta = parsed.metadata.get("jcamp_peak_table").expect("metadata");
        assert_eq!(table_meta["fields"], json!(["x", "y", "width"]));
        let peaks = table_meta["peaks"].as_array().expect("peaks");
        assert_eq!(peaks[0]["width"], json!(25.0));
        assert_eq!(peaks[1]["width"], json!(12.0));
    }

    #[test]
    fn prefers_peak_assignments_when_both_blocks_are_present() {
        let parsed = parse_jcamp_text(
            "##JCAMP-DX=5.00
##DATA TYPE=INFRARED PEAK ASSIGNMENTS
##XUNITS=1/CM
##YUNITS=ABSORBANCE
##NPOINTS=2
##PEAK TABLE=(XY..XY)
3300, 0.40; 1700, 0.90
##PEAK ASSIGNMENTS=(XYA)
3300, 0.42 <O-H stretch>
1700, 0.91 <C=C stretch>
##END=
",
        )
        .expect("parse with both tables");

        assert_eq!(parsed.signals[0].values, vec![0.42, 0.91]);
        let table_meta = parsed.metadata.get("jcamp_peak_table").expect("metadata");
        assert_eq!(table_meta["kind"], json!("peak_assignments"));
        let dropped = parsed
            .metadata
            .get("jcamp_peak_table_dropped")
            .expect("dropped metadata")
            .as_array()
            .expect("array");
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0]["kind"], json!("peak_table"));
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.contains("jcamp_peak_table_multiple_blocks")));
    }

    #[test]
    fn rejects_peak_table_with_count_mismatch() {
        let message = invalid_record_message(parse_jcamp_text(
            "##JCAMP-DX=5.00
##DATA TYPE=INFRARED PEAK TABLE
##XUNITS=1/CM
##YUNITS=ABSORBANCE
##NPOINTS=4
##PEAK TABLE=(XY..XY)
4000.0,1.0
##END=
",
        ));

        assert!(
            message.contains("peak_table NPOINTS mismatch"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn rejects_link_child_blocks_with_incompatible_axes_in_strict_parse() {
        // `parse_jcamp_text` is the strict composite path: any axis
        // mismatch raises an InvalidRecord error.
        let message = invalid_record_message(parse_jcamp_text(
            "##JCAMP-DX=5.00
##DATA TYPE=LINK
##TITLE=linked spectra
##JCAMP-DX=5.00
##TITLE=first child
##XUNITS=nm
##YUNITS=Absorbance
##FIRSTX=100
##DELTAX=1
##NPOINTS=3
##XYDATA=(X++(Y..Y))
100 1 2 3
##END=
##JCAMP-DX=5.00
##TITLE=second child
##XUNITS=nm
##YUNITS=Absorbance
##FIRSTX=101
##DELTAX=1
##NPOINTS=3
##XYDATA=(X++(Y..Y))
101 4 5 6
##END=
",
        ));

        assert!(message.contains("JCAMP LINK child blocks have incompatible axes"));
    }

    #[test]
    fn heterogeneous_link_fans_out_one_record_per_child() {
        // Through the Reader::read_bytes entry point the heterogeneous
        // LINK now emits one record per child with link_* metadata.
        let reader = super::JcampReader;
        let text = b"##JCAMP-DX=5.00
##DATA TYPE=LINK
##TITLE=linked spectra
##JCAMP-DX=5.00
##TITLE=first sample
##DATA TYPE=INFRARED SPECTRUM
##XUNITS=nm
##YUNITS=Absorbance
##FIRSTX=100
##DELTAX=1
##NPOINTS=3
##XYDATA=(X++(Y..Y))
100 1 2 3
##END=
##JCAMP-DX=5.00
##TITLE=second reference
##DATA TYPE=INFRARED REFERENCE SPECTRUM
##XUNITS=nm
##YUNITS=Reflectance
##FIRSTX=101
##DELTAX=2
##NPOINTS=3
##XYDATA=(X++(Y..Y))
101 4 5 6
##END=
";
        let records = reader
            .read_bytes(Path::new("synthetic_link.jdx"), text)
            .expect("read heterogeneous link");
        assert_eq!(records.len(), 2);
        let total = records[0].metadata.get("link_total").expect("link_total");
        assert_eq!(total, &json!(2));
        assert_eq!(
            records[0].metadata.get("link_index").expect("link_index"),
            &json!(0)
        );
        assert_eq!(
            records[1].metadata.get("link_index").expect("link_index"),
            &json!(1)
        );
        let parent0 = records[0]
            .metadata
            .get("link_parent_id")
            .expect("link_parent_id")
            .as_str()
            .expect("string");
        let parent1 = records[1]
            .metadata
            .get("link_parent_id")
            .expect("link_parent_id")
            .as_str()
            .expect("string");
        assert_eq!(parent0, parent1);
        assert!(parent0.starts_with("jcamp-"));
        assert_eq!(
            records[0].metadata.get("link_relation").expect("relation"),
            &json!("sample")
        );
        assert_eq!(
            records[1].metadata.get("link_relation").expect("relation"),
            &json!("reference")
        );
    }

    #[test]
    fn ocean_optics_link_stays_composite_with_link_metadata_absent() {
        // Same-axis LINK collapses into one record; the link_* metadata
        // only ships when there is more than one record to identify.
        let reader = super::JcampReader;
        let text = b"##JCAMP-DX=5.00
##DATA TYPE=LINK
##TITLE=composite
##JCAMP-DX=5.00
##TITLE=sample
##XUNITS=nm
##YUNITS=Absorbance
##FIRSTX=100
##DELTAX=1
##NPOINTS=3
##XYDATA=(X++(Y..Y))
100 1 2 3
##END=
##JCAMP-DX=5.00
##TITLE=reference
##XUNITS=nm
##YUNITS=Absorbance
##FIRSTX=100
##DELTAX=1
##NPOINTS=3
##XYDATA=(X++(Y..Y))
100 10 20 30
##END=
";
        let records = reader
            .read_bytes(Path::new("synthetic_composite.jdx"), text)
            .expect("read composite link");
        assert_eq!(records.len(), 1);
        assert!(!records[0].metadata.contains_key("link_index"));
        assert!(!records[0].metadata.contains_key("link_parent_id"));
    }

    #[test]
    fn rejects_xydata_with_fewer_points_than_declared() {
        let message = invalid_record_message(parse_jcamp_text(
            "##JCAMP-DX=5.00
##DATA TYPE=INFRARED SPECTRUM
##XUNITS=nm
##YUNITS=Absorbance
##FIRSTX=100
##DELTAX=1
##NPOINTS=4
##XYDATA=(X++(Y..Y))
100 1 2 3
##END=
",
        ));

        assert!(message.contains("JCAMP NPOINTS mismatch: declared 4, parsed 3"));
    }

    #[test]
    fn warns_on_xydata_line_x_checkpoint_mismatch() {
        let parsed = parse_jcamp_text(
            "##JCAMP-DX=5.00
##DATA TYPE=INFRARED SPECTRUM
##XUNITS=nm
##YUNITS=Absorbance
##FIRSTX=100
##DELTAX=1
##NPOINTS=4
##XYDATA=(X++(Y..Y))
100 1 2
105 3 4
##END=
",
        )
        .expect("parse checkpoint mismatch fixture");

        assert_eq!(parsed.axis, vec![100.0, 101.0, 102.0, 103.0]);
        assert_eq!(parsed.signals[0].values, vec![1.0, 2.0, 3.0, 4.0]);
        let drift_warning = parsed
            .warnings
            .iter()
            .find(|warning| warning.contains("jcamp_xydata_x_checkpoint_drift"))
            .expect("drift warning emitted");
        assert!(
            drift_warning.contains("abs=") && drift_warning.contains("rel="),
            "drift warning missing abs/rel deltas: {drift_warning}"
        );
    }

    fn invalid_record_message(result: Result<ParsedJcamp>) -> String {
        match result {
            Err(nirs4all_io_core::Error::InvalidRecord(message)) => message,
            Err(error) => panic!("expected InvalidRecord, got {error:?}"),
            Ok(_) => panic!("expected InvalidRecord, got parsed JCAMP"),
        }
    }
}
