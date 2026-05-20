use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralArray,
    SpectralAxis, SpectralRecord,
};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader as XmlReader;
use serde_json::{json, Value};

use crate::readers::util::{normalize_key, parse_number, provenance, read_text_lossy};
use crate::Reader;

const XML_FORMAT: &str = "horiba-jobinyvon-xml";
const TEXT_FORMAT: &str = "horiba-labspec-text";

pub struct HoribaLabSpecReader;

impl Reader for HoribaLabSpecReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::horiba_labspec"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let text = String::from_utf8_lossy(head);

        if ext == "xml" && text.contains("<LSX_Data") && text.contains("<LSX_Tree") {
            return Some(FormatProbe::new(
                XML_FORMAT,
                self.name(),
                Confidence::Definite,
                "Horiba/JobinYvon LabSpec LSX XML detected",
            ));
        }

        if ext == "txt" && looks_like_labspec_text(&text) {
            return Some(FormatProbe::new(
                TEXT_FORMAT,
                self.name(),
                Confidence::Definite,
                "Horiba LabSpec text export header detected",
            ));
        }

        None
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let (text, source) = read_text_lossy(path)?;
        if text.contains("<LSX_Data") && text.contains("<LSX_Tree") {
            read_lsx_xml(&text, source, self.name())
        } else if looks_like_labspec_text(&text) {
            read_labspec_text(&text, source, self.name())
        } else {
            Err(Error::UnsupportedFormat {
                path: path.to_path_buf(),
            })
        }
    }
}

#[derive(Clone, Debug, Default)]
struct XmlNode {
    name: String,
    id: Option<String>,
    index: Option<usize>,
    text: String,
    children: Vec<XmlNode>,
}

#[derive(Clone)]
struct AxisSpec {
    label: String,
    unit: String,
    values: Vec<f64>,
}

struct TextLine {
    leading_tabs: usize,
    numbers: Vec<f64>,
}

struct ParsedTextHeader {
    pairs: Vec<(String, String)>,
    axis_types: BTreeMap<usize, String>,
    axis_units: BTreeMap<usize, String>,
}

struct TextRow {
    values: Vec<f64>,
    metadata: BTreeMap<String, Value>,
}

struct IntensityRecordInput<'a> {
    format: &'a str,
    reader_name: &'a str,
    source: SourceFile,
    axis_values: Vec<f64>,
    axis_unit: String,
    axis_kind: AxisKind,
    values: Vec<f64>,
    signal_unit: Option<String>,
    metadata: BTreeMap<String, Value>,
    warnings: Vec<String>,
}

fn looks_like_labspec_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let has_labram = lower.contains("#instrument=\tlabram") || lower.contains("#instrument=labram");
    (lower.contains("#acq. time")
        && (lower.contains("#axis") || has_labram || lower.contains("#laser")))
        || (has_labram && lower.contains("#spectro") && lower.contains("#laser"))
}

fn read_lsx_xml(text: &str, source: SourceFile, reader_name: &str) -> Result<Vec<SpectralRecord>> {
    let root = parse_xml(text)?;
    let tree = find_first(&root, |node| node.name == "LSX_Tree").ok_or_else(|| {
        Error::InvalidRecord("Horiba XML contains no LSX_Tree dataset".to_string())
    })?;
    let dataset_type = direct_child_text_by_id(tree, "0x6D707974");
    let dataset_name = direct_child_text_by_id(tree, "0x6C7469D9");
    let axes = extract_xml_axes(tree);
    let spectral_axis = axes
        .iter()
        .find(|axis| {
            axis.label.eq_ignore_ascii_case("Spectr")
                || axis.label.to_ascii_lowercase().contains("spect")
        })
        .filter(|axis| !axis.values.is_empty())
        .ok_or_else(|| {
            Error::InvalidRecord("Horiba XML contains no spectral axis values".to_string())
        })?;
    let intensity_axis = axes
        .iter()
        .find(|axis| axis.label.eq_ignore_ascii_case("Intens"));
    let rows = extract_xml_matrix_rows(&root)?;
    let mut warnings = Vec::new();
    let (axis_kind, axis_unit) = spectral_axis_kind_unit(&spectral_axis.unit, &mut warnings);
    let signal_unit = intensity_axis.map(|axis| normalize_unit(&axis.unit));
    let mut metadata = xml_base_metadata(tree, dataset_type, dataset_name);
    metadata.insert("container".to_string(), json!("jobinyvon_xml"));
    metadata.insert("spectrum_count".to_string(), json!(rows.len()));
    metadata.insert(
        "spectral_point_count".to_string(),
        json!(spectral_axis.values.len()),
    );

    let row_count = rows.len();
    let mut out = Vec::new();
    for (index, values) in rows.into_iter().enumerate() {
        if values.len() != spectral_axis.values.len() {
            return Err(Error::InvalidRecord(format!(
                "Horiba XML matrix row {index} has {} points but axis has {}",
                values.len(),
                spectral_axis.values.len()
            )));
        }
        let mut record_metadata = metadata.clone();
        record_metadata.insert("spectrum_index".to_string(), json!(index));
        add_xml_spatial_metadata(&mut record_metadata, index, row_count, &axes);
        out.push(build_intensity_record(IntensityRecordInput {
            format: XML_FORMAT,
            reader_name,
            source: source.clone(),
            axis_values: spectral_axis.values.clone(),
            axis_unit: axis_unit.clone(),
            axis_kind: axis_kind.clone(),
            values,
            signal_unit: signal_unit.clone(),
            metadata: record_metadata,
            warnings: warnings.clone(),
        })?);
    }

    Ok(out)
}

fn read_labspec_text(
    text: &str,
    source: SourceFile,
    reader_name: &str,
) -> Result<Vec<SpectralRecord>> {
    let (header, lines) = parse_labspec_text(text);
    if lines.is_empty() {
        return Err(Error::InvalidRecord(
            "Horiba LabSpec text export contains no numeric data".to_string(),
        ));
    }

    let mut metadata = metadata_from_header(&header);
    metadata.insert("container".to_string(), json!("labspec_text"));
    let mut warnings = Vec::new();
    let signal_unit = header.axis_units.get(&0).map(|unit| normalize_unit(unit));
    let axis_unit = infer_text_axis_unit(&header, &mut warnings);
    let (axis_kind, axis_unit) = spectral_axis_kind_unit(&axis_unit, &mut warnings);

    let (axis_values, rows, layout) = parse_text_rows(&header, &lines)?;
    metadata.insert("axis_layout".to_string(), json!(layout));
    metadata.insert("spectrum_count".to_string(), json!(rows.len()));
    metadata.insert("spectral_point_count".to_string(), json!(axis_values.len()));

    let mut out = Vec::new();
    for (index, row) in rows.into_iter().enumerate() {
        if row.values.len() != axis_values.len() {
            return Err(Error::InvalidRecord(format!(
                "Horiba LabSpec text row {index} has {} points but axis has {}",
                row.values.len(),
                axis_values.len()
            )));
        }
        let mut record_metadata = metadata.clone();
        record_metadata.insert("spectrum_index".to_string(), json!(index));
        record_metadata.extend(row.metadata);
        out.push(build_intensity_record(IntensityRecordInput {
            format: TEXT_FORMAT,
            reader_name,
            source: source.clone(),
            axis_values: axis_values.clone(),
            axis_unit: axis_unit.clone(),
            axis_kind: axis_kind.clone(),
            values: row.values,
            signal_unit: signal_unit.clone(),
            metadata: record_metadata,
            warnings: warnings.clone(),
        })?);
    }

    Ok(out)
}

fn parse_xml(text: &str) -> Result<XmlNode> {
    let mut reader = XmlReader::from_str(text);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<XmlNode>::new();
    let mut root = XmlNode {
        name: "document".to_string(),
        ..XmlNode::default()
    };

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => stack.push(node_from_start(&event)),
            Ok(Event::Empty(event)) => {
                let node = node_from_start(&event);
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    root.children.push(node);
                }
            }
            Ok(Event::Text(event)) => {
                let text = event.decode().map_err(|error| {
                    Error::InvalidRecord(format!("Horiba XML text error: {error}"))
                })?;
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&text);
                }
            }
            Ok(Event::CData(event)) => {
                let text = event.decode().map_err(|error| {
                    Error::InvalidRecord(format!("Horiba XML CDATA error: {error}"))
                })?;
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&text);
                }
            }
            Ok(Event::End(_)) => {
                let Some(node) = stack.pop() else {
                    return Err(Error::InvalidRecord(
                        "Horiba XML has an unmatched closing tag".to_string(),
                    ));
                };
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    root.children.push(node);
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(Error::InvalidRecord(format!(
                    "Horiba XML parse error: {error}"
                )));
            }
            _ => {}
        }
    }

    if !stack.is_empty() {
        return Err(Error::InvalidRecord(
            "Horiba XML ended before all tags were closed".to_string(),
        ));
    }

    Ok(root)
}

fn node_from_start(event: &BytesStart<'_>) -> XmlNode {
    XmlNode {
        name: tag_name(event),
        id: attr_value(event, "ID"),
        index: attr_value(event, "Index").and_then(|value| value.parse::<usize>().ok()),
        text: String::new(),
        children: Vec::new(),
    }
}

fn tag_name(event: &BytesStart<'_>) -> String {
    local_name(event.name().as_ref())
}

fn local_name(name: &[u8]) -> String {
    let local = name
        .iter()
        .rposition(|byte| *byte == b':')
        .map_or(name, |index| &name[index + 1..]);
    String::from_utf8_lossy(local).into_owned()
}

fn attr_value(event: &BytesStart<'_>, key: &str) -> Option<String> {
    event
        .attributes()
        .flatten()
        .find(|attr| attr.key.as_ref() == key.as_bytes())
        .map(|attr| String::from_utf8_lossy(attr.value.as_ref()).into_owned())
}

fn find_first(node: &XmlNode, predicate: impl Fn(&XmlNode) -> bool + Copy) -> Option<&XmlNode> {
    if predicate(node) {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| find_first(child, predicate))
}

fn direct_child_by_id<'a>(node: &'a XmlNode, id: &str) -> Option<&'a XmlNode> {
    node.children.iter().find(|child| {
        child
            .id
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(id))
    })
}

fn direct_child_text_by_id(node: &XmlNode, id: &str) -> Option<String> {
    direct_child_by_id(node, id)
        .map(|child| child.text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn find_first_by_id<'a>(node: &'a XmlNode, id: &str) -> Option<&'a XmlNode> {
    find_first(node, |candidate| {
        candidate
            .id
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(id))
    })
}

fn extract_xml_axes(tree: &XmlNode) -> Vec<AxisSpec> {
    let Some(axis_root) = find_first_by_id(tree, "0x7B697861") else {
        return Vec::new();
    };

    axis_root
        .children
        .iter()
        .filter(|node| node.name == "LSX")
        .filter_map(|axis_node| {
            let label = direct_child_text_by_id(axis_node, "0x6D707974")?;
            let unit = direct_child_text_by_id(axis_node, "0x7C696E75").unwrap_or_default();
            let values = direct_child_by_id(axis_node, "0x7D6CD4DB")
                .map(|node| parse_numbers(&node.text))
                .unwrap_or_default();
            Some(AxisSpec {
                label,
                unit,
                values,
            })
        })
        .collect()
}

fn extract_xml_matrix_rows(root: &XmlNode) -> Result<Vec<Vec<f64>>> {
    let matrix = find_first(root, |node| node.name == "LSX_Matrix").ok_or_else(|| {
        Error::InvalidRecord("Horiba XML contains no LSX_Matrix signal payload".to_string())
    })?;
    let mut indexed = matrix
        .children
        .iter()
        .filter(|node| node.name == "LSX_Row")
        .map(|row| (row.index.unwrap_or(usize::MAX), parse_numbers(&row.text)))
        .collect::<Vec<_>>();
    indexed.sort_by_key(|(index, _)| *index);
    let rows = indexed
        .into_iter()
        .map(|(_, values)| values)
        .filter(|values| !values.is_empty())
        .collect::<Vec<_>>();

    if rows.is_empty() {
        let values = parse_numbers(&matrix.text);
        if values.is_empty() {
            return Err(Error::InvalidRecord(
                "Horiba XML signal matrix is empty".to_string(),
            ));
        }
        return Ok(vec![values]);
    }

    Ok(rows)
}

fn xml_base_metadata(
    tree: &XmlNode,
    dataset_type: Option<String>,
    dataset_name: Option<String>,
) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    if let Some(value) = dataset_type {
        metadata.insert("dataset_type".to_string(), json!(value));
    }
    if let Some(value) = dataset_name {
        metadata.insert("dataset_name".to_string(), json!(value));
    }

    let mut vendor = BTreeMap::new();
    collect_xml_metadata_pairs(tree, &mut vendor);
    add_promoted_vendor_fields(&mut metadata, &vendor);
    if !vendor.is_empty() {
        metadata.insert("vendor".to_string(), json!(vendor));
    }
    metadata
}

fn collect_xml_metadata_pairs(node: &XmlNode, out: &mut BTreeMap<String, Value>) {
    if let (Some(name), Some(value)) = (
        direct_child_text_by_id(node, "0x6D6D616E"),
        direct_child_text_by_id(node, "0x7D6C61DB"),
    ) {
        out.insert(normalize_key(&name), json!(value.trim()));
    }
    for child in &node.children {
        collect_xml_metadata_pairs(child, out);
    }
}

fn add_promoted_vendor_fields(
    metadata: &mut BTreeMap<String, Value>,
    vendor: &BTreeMap<String, Value>,
) {
    for key in [
        "instrument",
        "detector",
        "objective",
        "grating",
        "laser",
        "laser_nm",
        "acquired",
        "date",
        "title",
        "sample",
    ] {
        if let Some(value) = vendor.get(key) {
            metadata.insert(key.to_string(), value.clone());
        }
    }
}

fn add_xml_spatial_metadata(
    metadata: &mut BTreeMap<String, Value>,
    index: usize,
    row_count: usize,
    axes: &[AxisSpec],
) {
    let x_axis = axes
        .iter()
        .find(|axis| axis.label.eq_ignore_ascii_case("X") && !axis.values.is_empty());
    let y_axis = axes
        .iter()
        .find(|axis| axis.label.eq_ignore_ascii_case("Y") && !axis.values.is_empty());

    match (x_axis, y_axis) {
        (Some(x_axis), Some(y_axis)) if x_axis.values.len() * y_axis.values.len() == row_count => {
            let x_index = index % x_axis.values.len();
            let y_index = index / x_axis.values.len();
            insert_spatial(metadata, "x", x_axis.values[x_index], &x_axis.unit);
            insert_spatial(metadata, "y", y_axis.values[y_index], &y_axis.unit);
        }
        (Some(x_axis), Some(y_axis)) if y_axis.values.len() == row_count => {
            insert_spatial(metadata, "x", x_axis.values[0], &x_axis.unit);
            insert_spatial(metadata, "y", y_axis.values[index], &y_axis.unit);
        }
        (Some(x_axis), Some(y_axis)) if x_axis.values.len() == row_count => {
            insert_spatial(metadata, "x", x_axis.values[index], &x_axis.unit);
            insert_spatial(metadata, "y", y_axis.values[0], &y_axis.unit);
        }
        (Some(axis), None) if axis.values.len() == row_count => {
            insert_spatial(metadata, "x", axis.values[index], &axis.unit);
        }
        (None, Some(axis)) if axis.values.len() == row_count => {
            insert_spatial(metadata, "y", axis.values[index], &axis.unit);
        }
        _ => {}
    }
}

fn parse_labspec_text(text: &str) -> (ParsedTextHeader, Vec<TextLine>) {
    let mut header = ParsedTextHeader {
        pairs: Vec::new(),
        axis_types: BTreeMap::new(),
        axis_units: BTreeMap::new(),
    };
    let mut rows = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(header_line) = trimmed.strip_prefix('#') {
            if let Some((key, value)) = header_line.split_once('=') {
                let key = key.trim().to_string();
                let value = value.trim().to_string();
                if let Some(index) = axis_index(&key, "AxisType") {
                    header.axis_types.insert(index, value.clone());
                } else if let Some(index) = axis_index(&key, "AxisUnit") {
                    header.axis_units.insert(index, value.clone());
                }
                header.pairs.push((key, value));
            }
            continue;
        }

        let numbers = split_numbers(line);
        if !numbers.is_empty() {
            rows.push(TextLine {
                leading_tabs: line.chars().take_while(|ch| *ch == '\t').count(),
                numbers,
            });
        }
    }

    (header, rows)
}

fn axis_index(key: &str, prefix: &str) -> Option<usize> {
    key.strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('['))
        .and_then(|rest| rest.split_once(']'))
        .and_then(|(index, _)| index.parse::<usize>().ok())
}

fn metadata_from_header(header: &ParsedTextHeader) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    let mut vendor = BTreeMap::new();
    for (key, value) in &header.pairs {
        vendor.insert(normalize_key(key), json!(value.trim()));
    }
    add_promoted_vendor_fields(&mut metadata, &vendor);
    promote_text_spatial_header(&mut metadata, &vendor);
    metadata.insert("vendor".to_string(), json!(vendor));
    metadata
}

fn promote_text_spatial_header(
    metadata: &mut BTreeMap<String, Value>,
    vendor: &BTreeMap<String, Value>,
) {
    for (axis, key_fragment) in [("x", "x_"), ("y", "y_"), ("z", "z_")] {
        if let Some((_, value)) = vendor
            .iter()
            .find(|(key, _)| key.starts_with(key_fragment) && key.contains('m'))
        {
            if let Some(number) = value.as_str().and_then(parse_number) {
                insert_spatial(metadata, axis, number, "um");
            }
        }
    }
}

fn parse_text_rows(
    header: &ParsedTextHeader,
    lines: &[TextLine],
) -> Result<(Vec<f64>, Vec<TextRow>, &'static str)> {
    let first = &lines[0];
    if is_map_layout(header, first, lines) {
        parse_map_text_rows(header, lines)
    } else if is_wide_layout(first, lines) {
        parse_wide_text_rows(header, lines)
    } else if lines.iter().all(|line| line.numbers.len() == 2) {
        parse_two_column_text_rows(lines)
    } else {
        Err(Error::InvalidRecord(
            "Horiba LabSpec text export has an unsupported numeric layout".to_string(),
        ))
    }
}

fn is_map_layout(header: &ParsedTextHeader, first: &TextLine, lines: &[TextLine]) -> bool {
    let has_xy_axes = matches!(
        (header.axis_types.get(&2), header.axis_types.get(&3)),
        (Some(x), Some(y)) if x.eq_ignore_ascii_case("X") && y.eq_ignore_ascii_case("Y")
    );
    has_xy_axes
        || (first.leading_tabs >= 2
            && lines
                .get(1)
                .is_some_and(|line| line.numbers.len() == first.numbers.len() + 2))
}

fn is_wide_layout(first: &TextLine, lines: &[TextLine]) -> bool {
    first.leading_tabs >= 1
        || (first.numbers.len() > 2
            && lines
                .get(1)
                .is_some_and(|line| line.numbers.len() == first.numbers.len() + 1))
}

fn parse_two_column_text_rows(
    lines: &[TextLine],
) -> Result<(Vec<f64>, Vec<TextRow>, &'static str)> {
    let axis = lines.iter().map(|line| line.numbers[0]).collect::<Vec<_>>();
    let values = lines.iter().map(|line| line.numbers[1]).collect::<Vec<_>>();
    Ok((
        axis,
        vec![TextRow {
            values,
            metadata: BTreeMap::new(),
        }],
        "two_column",
    ))
}

fn parse_wide_text_rows(
    header: &ParsedTextHeader,
    lines: &[TextLine],
) -> Result<(Vec<f64>, Vec<TextRow>, &'static str)> {
    let axis = lines[0].numbers.clone();
    let index_key = if header
        .axis_types
        .get(&2)
        .is_some_and(|axis_type| axis_type.eq_ignore_ascii_case("Points"))
    {
        "point_index"
    } else {
        "series_index"
    };
    let mut rows = Vec::new();
    for line in &lines[1..] {
        if line.numbers.len() != axis.len() + 1 {
            return Err(Error::InvalidRecord(
                "Horiba LabSpec wide text row length does not match the axis".to_string(),
            ));
        }
        let mut metadata = BTreeMap::new();
        metadata.insert(index_key.to_string(), json!(line.numbers[0]));
        rows.push(TextRow {
            values: line.numbers[1..].to_vec(),
            metadata,
        });
    }
    Ok((axis, rows, "series_rows"))
}

fn parse_map_text_rows(
    header: &ParsedTextHeader,
    lines: &[TextLine],
) -> Result<(Vec<f64>, Vec<TextRow>, &'static str)> {
    let axis = lines[0].numbers.clone();
    let x_unit = header
        .axis_units
        .get(&2)
        .map(|unit| normalize_unit(unit))
        .unwrap_or_else(|| "um".to_string());
    let y_unit = header
        .axis_units
        .get(&3)
        .map(|unit| normalize_unit(unit))
        .unwrap_or_else(|| "um".to_string());
    let mut rows = Vec::new();
    for line in &lines[1..] {
        if line.numbers.len() != axis.len() + 2 {
            return Err(Error::InvalidRecord(
                "Horiba LabSpec map text row length does not match the axis".to_string(),
            ));
        }
        let mut metadata = BTreeMap::new();
        insert_spatial(&mut metadata, "x", line.numbers[0], &x_unit);
        insert_spatial(&mut metadata, "y", line.numbers[1], &y_unit);
        rows.push(TextRow {
            values: line.numbers[2..].to_vec(),
            metadata,
        });
    }
    Ok((axis, rows, "map_rows"))
}

fn infer_text_axis_unit(header: &ParsedTextHeader, warnings: &mut Vec<String>) -> String {
    if let Some(unit) = header.axis_units.get(&1) {
        if !unit.trim().is_empty() {
            return normalize_unit(unit);
        }
    }

    let header_text = header
        .pairs
        .iter()
        .map(|(key, value)| format!("{key} {value}"))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    warnings.push("horiba_labspec_text_axis_unit_inferred".to_string());
    if header_text.contains("nm") && !header_text.contains("cm-") {
        "nm".to_string()
    } else {
        "cm-1".to_string()
    }
}

fn build_intensity_record(input: IntensityRecordInput<'_>) -> Result<SpectralRecord> {
    let axis = SpectralAxis::new(input.axis_values, input.axis_unit, input.axis_kind)?;
    let signal = SpectralArray::new(
        axis,
        input.values,
        vec!["x".to_string()],
        SignalType::RawCounts,
        input.signal_unit,
        "intensity",
        "file",
    )?;
    let mut signals = BTreeMap::new();
    signals.insert("intensity".to_string(), signal);
    let record = SpectralRecord {
        signals,
        signal_type: SignalType::RawCounts,
        targets: BTreeMap::new(),
        metadata: input.metadata,
        provenance: provenance(
            input.format,
            input.reader_name,
            input.source,
            input.warnings,
        ),
        quality_flags: Vec::new(),
    };
    record.validate()?;
    Ok(record)
}

fn spectral_axis_kind_unit(unit: &str, warnings: &mut Vec<String>) -> (AxisKind, String) {
    let normalized = normalize_unit(unit);
    match normalized.as_str() {
        "nm" | "um" => (AxisKind::Wavelength, normalized),
        "cm-1" => (AxisKind::Wavenumber, normalized),
        "eV" => {
            if !warnings
                .iter()
                .any(|warning| warning == "horiba_unsupported_axis_kind_energy")
            {
                warnings.push("horiba_unsupported_axis_kind_energy".to_string());
            }
            (AxisKind::Index, normalized)
        }
        _ => (AxisKind::Index, normalized),
    }
}

fn normalize_unit(unit: &str) -> String {
    let trimmed = unit.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("1/cm")
        || lower.contains("cm-")
        || lower.contains("cm⁻")
        || (lower.contains("cm") && trimmed.contains('�'))
    {
        "cm-1".to_string()
    } else if lower == "ev" {
        "eV".to_string()
    } else if lower.contains("µm") || lower.contains("μm") || trimmed.contains("�m") {
        "um".to_string()
    } else if lower == "nm" || lower.contains("(nm)") {
        "nm".to_string()
    } else {
        trimmed.to_string()
    }
}

fn insert_spatial(metadata: &mut BTreeMap<String, Value>, axis: &str, value: f64, unit: &str) {
    metadata.insert(format!("spatial_{axis}"), json!(value));
    metadata.insert(format!("spatial_{axis}_unit"), json!(normalize_unit(unit)));
}

fn parse_numbers(text: &str) -> Vec<f64> {
    split_numbers(text)
}

fn split_numbers(text: &str) -> Vec<f64> {
    text.split_whitespace().filter_map(parse_number).collect()
}
