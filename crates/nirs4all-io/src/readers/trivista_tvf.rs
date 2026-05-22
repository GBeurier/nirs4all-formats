use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader as XmlReader;
use quick_xml::XmlVersion;
use serde_json::{json, Value};

use crate::readers::util::{
    parse_number, read_bytes, single_signal_record, text_lossy_from_bytes, SingleSignalSpec,
};
use crate::Reader;

const FORMAT: &str = "trivista-tvf";

pub struct TrivistaTvfReader;

impl Reader for TrivistaTvfReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::trivista_tvf"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext != "tvf" {
            return None;
        }
        let text = String::from_utf8_lossy(head);
        if text.contains("<XmlMain") && text.contains("TriVista-File") {
            return Some(FormatProbe::new(
                FORMAT,
                self.name(),
                Confidence::Definite,
                "Princeton/TriVista TVF XML spectroscopy container",
            ));
        }
        None
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
        if !text.contains("<XmlMain") {
            return Err(Error::UnsupportedFormat {
                path: path.to_path_buf(),
            });
        }
        read_trivista_tvf(&text, source, self.name())
    }
}

#[derive(Clone, Debug, Default)]
struct XmlNode {
    name: String,
    attrs: BTreeMap<String, String>,
    text: String,
    children: Vec<XmlNode>,
}

#[derive(Clone)]
struct NavAxis {
    from: f64,
    step: f64,
    points: usize,
    unit: String,
}

struct SignalAxis {
    values: Vec<f64>,
    unit: String,
    kind: AxisKind,
    display_unit: Option<String>,
    label: String,
    calibration_type: Option<String>,
    laser_wave: Option<f64>,
    xdim_length: usize,
}

fn read_trivista_tvf(
    text: &str,
    source: SourceFile,
    reader_name: &str,
) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
    let root = parse_xml(text, "TriVista TVF")?;
    let xml_main = find_first(&root, |node| node.name == "XmlMain")
        .ok_or_else(|| Error::InvalidRecord("TriVista TVF contains no XmlMain root".to_string()))?;
    let documents = direct_child(xml_main, "Documents").ok_or_else(|| {
        Error::InvalidRecord("TriVista TVF contains no Documents section".to_string())
    })?;
    let document = direct_child(documents, "Document").ok_or_else(|| {
        Error::InvalidRecord("TriVista TVF contains no Document payload".to_string())
    })?;

    let mut root_metadata = BTreeMap::new();
    root_metadata.insert("container".to_string(), json!("trivista_tvf"));
    insert_attr(&mut root_metadata, xml_main, "Version", "file_version");
    insert_attr(&mut root_metadata, xml_main, "Filename", "source_filename");
    insert_attr(&mut root_metadata, xml_main, "DateTime", "file_datetime");

    let mut out = Vec::new();
    let mut document_index = 0usize;
    collect_document_records(
        document,
        &root_metadata,
        &source,
        reader_name,
        0,
        &mut document_index,
        &mut out,
    )?;
    Ok(out)
}

fn collect_document_records(
    document: &XmlNode,
    root_metadata: &BTreeMap<String, Value>,
    source: &SourceFile,
    reader_name: &str,
    depth: usize,
    document_index: &mut usize,
    out: &mut Vec<nirs4all_io_core::SpectralRecord>,
) -> Result<()> {
    let current_document_index = *document_index;
    *document_index += 1;

    let axis = parse_signal_axis(document)?;
    let frames = parse_frames(document)?;
    let info_groups = parse_info_groups(document.attrs.get("InfoSerialized").map(String::as_str))?;
    let nav_x = nav_axis_from_group(info_groups.get("X-Axis"));
    let nav_y = nav_axis_from_group(info_groups.get("Y-Axis"));
    let child_count = direct_child(document, "Childs")
        .and_then(|node| attr(node, "Count"))
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);

    let mut base_metadata = root_metadata.clone();
    base_metadata.insert("document_index".to_string(), json!(current_document_index));
    base_metadata.insert(
        "document_role".to_string(),
        json!(if depth == 0 { "primary" } else { "child" }),
    );
    base_metadata.insert("document_depth".to_string(), json!(depth));
    base_metadata.insert("child_document_count".to_string(), json!(child_count));
    base_metadata.insert("document_frame_count".to_string(), json!(frames.len()));
    base_metadata.insert("spectral_point_count".to_string(), json!(axis.values.len()));
    base_metadata.insert("xdim_length".to_string(), json!(axis.xdim_length));
    base_metadata.insert("spectral_axis_label".to_string(), json!(&axis.label));
    base_metadata.insert("spectral_axis_unit".to_string(), json!(&axis.unit));
    if let Some(display_unit) = &axis.display_unit {
        base_metadata.insert(
            "spectral_axis_display_unit".to_string(),
            json!(display_unit),
        );
    }
    if let Some(calibration_type) = &axis.calibration_type {
        base_metadata.insert(
            "spectral_axis_calibration_type".to_string(),
            json!(calibration_type),
        );
    }
    if let Some(laser_wave) = axis.laser_wave {
        base_metadata.insert("spectral_axis_laser_wave".to_string(), json!(laser_wave));
    }
    insert_attr(&mut base_metadata, document, "Label", "document_label");
    insert_attr(&mut base_metadata, document, "DataLabel", "data_label");
    insert_attr(&mut base_metadata, document, "DocType", "document_type");
    insert_attr(&mut base_metadata, document, "RecordTime", "record_time");
    insert_attr(&mut base_metadata, document, "ModeName", "mode_name");
    insert_group_metadata(&mut base_metadata, &info_groups);
    insert_nav_axis_metadata(&mut base_metadata, "x", &nav_x);
    insert_nav_axis_metadata(&mut base_metadata, "y", &nav_y);

    let signal_unit = document
        .attrs
        .get("DataLabel")
        .and_then(|value| signal_unit(value));
    let signal_type = trivista_signal_type(document.attrs.get("Label"), signal_unit.as_deref());
    let frame_count = frames.len();
    let first_timestamp = frames.first().map(|frame| frame.timestamp).unwrap_or(0);

    for (frame_index, frame) in frames.into_iter().enumerate() {
        if frame.values.len() != axis.values.len() {
            return Err(Error::InvalidRecord(format!(
                "TriVista TVF document {current_document_index} frame {frame_index} has {} points but axis has {}",
                frame.values.len(),
                axis.values.len()
            )));
        }
        if frame.x_dim != axis.values.len() {
            return Err(Error::InvalidRecord(format!(
                "TriVista TVF document {current_document_index} frame {frame_index} declares xDim={} but axis has {}",
                frame.x_dim,
                axis.values.len()
            )));
        }
        let mut metadata = base_metadata.clone();
        metadata.insert("spectrum_index".to_string(), json!(out.len()));
        metadata.insert("frame_index".to_string(), json!(frame_index));
        metadata.insert("frame_x_dim".to_string(), json!(frame.x_dim));
        metadata.insert("time_filetime_100ns".to_string(), json!(frame.timestamp));
        metadata.insert(
            "elapsed_time_seconds".to_string(),
            json!(elapsed_filetime_seconds(frame.timestamp, first_timestamp)),
        );
        metadata.insert("elapsed_time_unit".to_string(), json!("s"));
        insert_navigation_values(&mut metadata, frame_index, frame_count, &nav_x, &nav_y);

        out.push(single_signal_record(
            FORMAT,
            reader_name,
            source.clone(),
            SingleSignalSpec {
                axis_values: axis.values.clone(),
                axis_unit: axis.unit.clone(),
                axis_kind: axis.kind.clone(),
                values: frame.values,
                signal_name: "intensity".to_string(),
                signal_type: signal_type.clone(),
                signal_unit: signal_unit.clone(),
                role: "intensity".to_string(),
            },
            BTreeMap::new(),
            metadata,
            vec!["trivista_tvf_xml_reverse_engineered".to_string()],
        )?);
    }

    if let Some(childs) = direct_child(document, "Childs") {
        for child in childs
            .children
            .iter()
            .filter(|node| node.name == "Document")
        {
            collect_document_records(
                child,
                root_metadata,
                source,
                reader_name,
                depth + 1,
                document_index,
                out,
            )?;
        }
    }

    Ok(())
}

struct FrameData {
    x_dim: usize,
    timestamp: u64,
    values: Vec<f64>,
}

fn parse_signal_axis(document: &XmlNode) -> Result<SignalAxis> {
    let x_dim = direct_child(document, "xDim")
        .ok_or_else(|| Error::InvalidRecord("TriVista TVF document has no xDim".to_string()))?;
    let xdim_length = attr(x_dim, "Length")
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| Error::InvalidRecord("TriVista TVF xDim has no Length".to_string()))?;
    for calibration in x_dim
        .children
        .iter()
        .filter(|node| node.name == "Calibration")
    {
        let Some(value_array) = attr(calibration, "ValueArray") else {
            continue;
        };
        let values = parse_value_array(value_array)?;
        if values.is_empty() {
            continue;
        }
        if values.len() != xdim_length {
            return Err(Error::InvalidRecord(format!(
                "TriVista TVF xDim Length={xdim_length} but Calibration ValueArray contains {} points",
                values.len()
            )));
        }
        let raw_unit = attr(calibration, "Unit").unwrap_or("Nanometer");
        let unit = normalize_axis_unit(raw_unit);
        let label = attr(calibration, "Label")
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| default_axis_label(&unit).to_string());
        return Ok(SignalAxis {
            values,
            kind: axis_kind_for_unit(&unit),
            unit,
            display_unit: attr(calibration, "DisplayUnit")
                .filter(|value| !value.trim().is_empty())
                .map(ToString::to_string),
            label,
            calibration_type: attr(calibration, "Type")
                .filter(|value| !value.trim().is_empty())
                .map(ToString::to_string),
            laser_wave: attr(calibration, "LaserWave").and_then(parse_number),
            xdim_length,
        });
    }
    Err(Error::InvalidRecord(
        "TriVista TVF spectral axis is empty".to_string(),
    ))
}

fn parse_frames(document: &XmlNode) -> Result<Vec<FrameData>> {
    let data = direct_child(document, "Data")
        .ok_or_else(|| Error::InvalidRecord("TriVista TVF document has no Data".to_string()))?;
    let mut frames = Vec::new();
    for frame in data.children.iter().filter(|node| node.name == "Frame") {
        let values = frame
            .text
            .split(';')
            .filter_map(parse_number)
            .collect::<Vec<_>>();
        let x_dim = attr(frame, "xDim")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(values.len());
        let timestamp = attr(frame, "TimeStamp")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        frames.push(FrameData {
            x_dim,
            timestamp,
            values,
        });
    }
    if frames.is_empty() {
        return Err(Error::InvalidRecord(
            "TriVista TVF document has no Frame data".to_string(),
        ));
    }
    Ok(frames)
}

fn parse_value_array(value_array: &str) -> Result<Vec<f64>> {
    if value_array.trim() == "0" {
        return Ok(Vec::new());
    }
    let mut parts = value_array.split('|');
    let declared = parts
        .next()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| {
            Error::InvalidRecord("TriVista TVF ValueArray has no point count".to_string())
        })?;
    let values = parts.filter_map(parse_number).collect::<Vec<_>>();
    if values.len() != declared {
        return Err(Error::InvalidRecord(format!(
            "TriVista TVF ValueArray declares {declared} points but contains {}",
            values.len()
        )));
    }
    Ok(values)
}

fn normalize_axis_unit(unit: &str) -> String {
    match unit.trim().to_ascii_lowercase().as_str() {
        "nanometer" | "nanometers" | "nm" => "nm".to_string(),
        "1/cm" | "cm-1" | "wavenumber" => "cm-1".to_string(),
        "" => "index".to_string(),
        _ => unit.trim().to_string(),
    }
}

fn axis_kind_for_unit(unit: &str) -> AxisKind {
    match unit {
        "nm" => AxisKind::Wavelength,
        "cm-1" => AxisKind::Wavenumber,
        _ => AxisKind::Index,
    }
}

fn default_axis_label(unit: &str) -> &'static str {
    match axis_kind_for_unit(unit) {
        AxisKind::Wavelength => "Wavelength",
        AxisKind::Wavenumber => "Wavenumber",
        AxisKind::Frequency => "Frequency",
        AxisKind::Energy => "Energy",
        AxisKind::Time => "Time",
        AxisKind::Index => "Index",
    }
}

fn parse_info_groups(
    serialized: Option<&str>,
) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
    let Some(serialized) = serialized else {
        return Ok(BTreeMap::new());
    };
    let stripped = strip_xml_declaration(serialized.trim());
    if stripped.is_empty() {
        return Ok(BTreeMap::new());
    }
    let root = parse_xml(stripped, "TriVista InfoSerialized")?;
    let mut groups = BTreeMap::new();
    collect_info_groups(&root, &mut groups);
    Ok(groups)
}

fn strip_xml_declaration(text: &str) -> &str {
    if text.starts_with("<?xml") {
        text.find("?>")
            .map(|index| text[index + 2..].trim_start())
            .unwrap_or(text)
    } else {
        text
    }
}

fn collect_info_groups(node: &XmlNode, out: &mut BTreeMap<String, BTreeMap<String, String>>) {
    if node.name == "Group" {
        if let Some(name) = direct_child_text(node, "Name") {
            let pairs = item_pairs(node);
            if !pairs.is_empty() {
                let key = unique_group_key(out, name);
                out.insert(key, pairs);
            }
        }
    }
    for child in &node.children {
        collect_info_groups(child, out);
    }
}

fn unique_group_key(groups: &BTreeMap<String, BTreeMap<String, String>>, name: &str) -> String {
    if !groups.contains_key(name) {
        return name.to_string();
    }
    let mut index = 2usize;
    loop {
        let candidate = format!("{name}{index}");
        if !groups.contains_key(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn item_pairs(group: &XmlNode) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(items) = direct_child(group, "Items") else {
        return out;
    };
    for item in items.children.iter().filter(|node| node.name == "Item") {
        if let Some(name) = direct_child_text(item, "Name") {
            let value = direct_child_text(item, "Value").unwrap_or_default();
            out.insert(name.to_string(), value.to_string());
        }
    }
    out
}

fn nav_axis_from_group(group: Option<&BTreeMap<String, String>>) -> Option<NavAxis> {
    let group = group?;
    Some(NavAxis {
        from: parse_number(group.get("From")?)?,
        step: parse_number(group.get("Step")?)?,
        points: group.get("Points")?.parse::<usize>().ok()?,
        unit: nav_axis_unit(group),
    })
}

fn nav_axis_unit(group: &BTreeMap<String, String>) -> String {
    let unit = ["Unit", "Units", "DisplayUnit", "Display Unit"]
        .iter()
        .find_map(|key| group.get(*key))
        .map(String::as_str)
        .unwrap_or("unknown");
    normalize_spatial_unit(unit)
}

fn normalize_spatial_unit(unit: &str) -> String {
    match unit.trim().to_ascii_lowercase().as_str() {
        "micrometer" | "micrometers" | "micron" | "microns" | "um" | "µm" => "um".to_string(),
        "millimeter" | "millimeters" | "mm" => "mm".to_string(),
        "nanometer" | "nanometers" | "nm" => "nm".to_string(),
        "unknown" | "" => "unknown".to_string(),
        _ => unit.trim().to_string(),
    }
}

fn insert_group_metadata(
    metadata: &mut BTreeMap<String, Value>,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
) {
    insert_group_string(metadata, groups, "Experiment", "Mode", "experiment_mode");
    insert_group_string(
        metadata,
        groups,
        "Experiment",
        "Stage Mode",
        "experiment_stage_mode",
    );
    insert_group_string(metadata, groups, "Experiment", "Used Setup", "used_setup");
    insert_group_string(metadata, groups, "Experiment", "Used Time", "used_time");
    insert_group_number(
        metadata,
        groups,
        "Detector",
        "Exposure_Time_(ms)",
        "exposure_time_ms",
    );
    insert_group_number(
        metadata,
        groups,
        "Detector",
        "No_of_Accumulations",
        "accumulation_count",
    );
    insert_group_string(metadata, groups, "Detector", "Calc_Average", "calc_average");
    insert_group_number(
        metadata,
        groups,
        "Detector",
        "No_of_Frames",
        "declared_frame_count",
    );
    insert_group_number(
        metadata,
        groups,
        "Detector",
        "Detector_Temperature",
        "detector_temperature_c",
    );
    insert_group_string(metadata, groups, "Detector", "Name", "detector_name");
    insert_group_string(
        metadata,
        groups,
        "Detector",
        "Serialnumber",
        "detector_serial_number",
    );
    insert_group_string(
        metadata,
        groups,
        "Detector",
        "Detector_Size",
        "detector_size",
    );
    insert_group_string(
        metadata,
        groups,
        "Detector",
        "ADC__Readout_Port",
        "detector_adc_readout_port",
    );
    insert_group_string(
        metadata,
        groups,
        "Detector",
        "ADC__Rate_Resolution",
        "detector_adc_rate_resolution",
    );
    insert_group_number(
        metadata,
        groups,
        "Detector",
        "ADC__Gain",
        "detector_adc_gain",
    );
    insert_group_number(
        metadata,
        groups,
        "Detector",
        "Clearing__No_of_Cleans",
        "detector_clearing_count",
    );
    insert_group_string(
        metadata,
        groups,
        "Detector",
        "Region_of_Interests",
        "detector_roi",
    );
    insert_group_number(
        metadata,
        groups,
        "Calibration",
        "Center_Wavelength",
        "center_wavelength_nm",
    );
    insert_group_number(
        metadata,
        groups,
        "Calibration",
        "Laser_Wavelength",
        "laser_wavelength_nm",
    );
    insert_spectrometer_metadata(metadata, groups);
}

fn insert_group_string(
    metadata: &mut BTreeMap<String, Value>,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
    group: &str,
    key: &str,
    metadata_key: &str,
) {
    if let Some(value) = groups.get(group).and_then(|group| group.get(key)) {
        if !value.is_empty() {
            metadata.insert(metadata_key.to_string(), json!(value));
        }
    }
}

fn insert_group_number(
    metadata: &mut BTreeMap<String, Value>,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
    group: &str,
    key: &str,
    metadata_key: &str,
) {
    if let Some(value) = groups
        .get(group)
        .and_then(|group| group.get(key))
        .and_then(|value| parse_number(value))
    {
        metadata.insert(metadata_key.to_string(), json!(value));
    }
}

fn insert_spectrometer_metadata(
    metadata: &mut BTreeMap<String, Value>,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
) {
    let spectrometers = groups
        .iter()
        .filter_map(|(name, group)| {
            if group.is_empty() {
                None
            } else {
                spectrometer_group_index(name).map(|index| (index, group))
            }
        })
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect::<Vec<_>>();
    if spectrometers.is_empty() {
        return;
    }

    metadata.insert("spectrometer_count".to_string(), json!(spectrometers.len()));
    insert_first_spectrometer_string(metadata, &spectrometers, "Serialnumber", "serial_number");
    insert_first_spectrometer_string(metadata, &spectrometers, "Model", "model");
    insert_first_spectrometer_number(metadata, &spectrometers, "Stage_Number", "stage_number");
    insert_first_spectrometer_number(metadata, &spectrometers, "Focallength", "focal_length_mm");
    insert_first_spectrometer_number(
        metadata,
        &spectrometers,
        "Inclusion_Angle",
        "inclusion_angle",
    );
    insert_first_spectrometer_number(metadata, &spectrometers, "Detector_Angle", "detector_angle");
    insert_first_spectrometer_string(metadata, &spectrometers, "Groove_Density", "groove_density");
    insert_first_spectrometer_number(metadata, &spectrometers, "Order", "order");

    insert_spectrometer_string_array(
        metadata,
        &spectrometers,
        "Serialnumber",
        "spectrometer_serial_numbers",
    );
    insert_spectrometer_string_array(metadata, &spectrometers, "Model", "spectrometer_models");
    insert_spectrometer_number_array(
        metadata,
        &spectrometers,
        "Stage_Number",
        "spectrometer_stage_numbers",
    );
    insert_spectrometer_number_array(
        metadata,
        &spectrometers,
        "Focallength",
        "spectrometer_focal_lengths_mm",
    );
    insert_spectrometer_number_array(
        metadata,
        &spectrometers,
        "Inclusion_Angle",
        "spectrometer_inclusion_angles",
    );
    insert_spectrometer_number_array(
        metadata,
        &spectrometers,
        "Detector_Angle",
        "spectrometer_detector_angles",
    );
    insert_spectrometer_string_array(
        metadata,
        &spectrometers,
        "Groove_Density",
        "spectrometer_groove_densities",
    );
    insert_spectrometer_number_array(metadata, &spectrometers, "Order", "spectrometer_orders");
}

fn spectrometer_group_index(name: &str) -> Option<usize> {
    let suffix = name.strip_prefix("Spectrometer")?;
    if suffix.is_empty() {
        Some(1)
    } else {
        suffix.parse::<usize>().ok()
    }
}

fn insert_first_spectrometer_string(
    metadata: &mut BTreeMap<String, Value>,
    spectrometers: &[&BTreeMap<String, String>],
    key: &str,
    suffix: &str,
) {
    if let Some(value) = spectrometers.first().and_then(|group| group.get(key)) {
        if !value.is_empty() {
            metadata.insert(format!("spectrometer_{suffix}"), json!(value));
        }
    }
}

fn insert_first_spectrometer_number(
    metadata: &mut BTreeMap<String, Value>,
    spectrometers: &[&BTreeMap<String, String>],
    key: &str,
    suffix: &str,
) {
    if let Some(value) = spectrometers
        .first()
        .and_then(|group| group.get(key))
        .and_then(|value| parse_number(value))
    {
        metadata.insert(format!("spectrometer_{suffix}"), json!(value));
    }
}

fn insert_spectrometer_string_array(
    metadata: &mut BTreeMap<String, Value>,
    spectrometers: &[&BTreeMap<String, String>],
    key: &str,
    metadata_key: &str,
) {
    let values = spectrometers
        .iter()
        .filter_map(|group| group.get(key))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if !values.is_empty() {
        metadata.insert(metadata_key.to_string(), json!(values));
    }
}

fn insert_spectrometer_number_array(
    metadata: &mut BTreeMap<String, Value>,
    spectrometers: &[&BTreeMap<String, String>],
    key: &str,
    metadata_key: &str,
) {
    let values = spectrometers
        .iter()
        .filter_map(|group| group.get(key))
        .filter_map(|value| parse_number(value))
        .collect::<Vec<_>>();
    if !values.is_empty() {
        metadata.insert(metadata_key.to_string(), json!(values));
    }
}

fn insert_nav_axis_metadata(
    metadata: &mut BTreeMap<String, Value>,
    axis_name: &str,
    axis: &Option<NavAxis>,
) {
    if let Some(axis) = axis {
        metadata.insert(format!("spatial_{axis_name}_from"), json!(axis.from));
        metadata.insert(format!("spatial_{axis_name}_step"), json!(axis.step));
        metadata.insert(format!("spatial_{axis_name}_points"), json!(axis.points));
        metadata.insert(format!("spatial_{axis_name}_unit"), json!(&axis.unit));
    }
}

fn insert_navigation_values(
    metadata: &mut BTreeMap<String, Value>,
    frame_index: usize,
    frame_count: usize,
    nav_x: &Option<NavAxis>,
    nav_y: &Option<NavAxis>,
) {
    match (nav_x, nav_y) {
        (Some(x_axis), Some(y_axis)) => {
            let x_index = frame_index % x_axis.points;
            let y_index = frame_index / x_axis.points;
            metadata.insert("spatial_x_index".to_string(), json!(x_index));
            metadata.insert("spatial_y_index".to_string(), json!(y_index));
            metadata.insert(
                "spatial_x".to_string(),
                json!(x_axis.from + x_axis.step * x_index as f64),
            );
            metadata.insert(
                "spatial_y".to_string(),
                json!(y_axis.from + y_axis.step * y_index as f64),
            );
            metadata.insert("spatial_x_unit".to_string(), json!(&x_axis.unit));
            metadata.insert("spatial_y_unit".to_string(), json!(&y_axis.unit));
        }
        (Some(x_axis), None) => {
            metadata.insert("spatial_x_index".to_string(), json!(frame_index));
            metadata.insert(
                "spatial_x".to_string(),
                json!(x_axis.from + x_axis.step * frame_index as f64),
            );
            metadata.insert("spatial_x_unit".to_string(), json!(&x_axis.unit));
        }
        (None, Some(y_axis)) => {
            metadata.insert("spatial_y_index".to_string(), json!(frame_index));
            metadata.insert(
                "spatial_y".to_string(),
                json!(y_axis.from + y_axis.step * frame_index as f64),
            );
            metadata.insert("spatial_y_unit".to_string(), json!(&y_axis.unit));
        }
        (None, None) => {
            if frame_count > 1 {
                metadata.insert("time_index".to_string(), json!(frame_index));
            }
        }
    }
}

fn signal_unit(data_label: &str) -> Option<String> {
    if data_label.to_ascii_lowercase().contains("count") {
        Some("counts".to_string())
    } else {
        None
    }
}

fn trivista_signal_type(label: Option<&String>, unit: Option<&str>) -> SignalType {
    let label = label
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    if label.contains("intensity") || unit == Some("counts") {
        SignalType::RawCounts
    } else {
        SignalType::Unknown
    }
}

fn elapsed_filetime_seconds(value: u64, first: u64) -> f64 {
    if value >= first {
        (value - first) as f64 / 10_000_000.0
    } else {
        -((first - value) as f64 / 10_000_000.0)
    }
}

fn insert_attr(metadata: &mut BTreeMap<String, Value>, node: &XmlNode, attr_name: &str, key: &str) {
    if let Some(value) = attr(node, attr_name) {
        if !value.is_empty() {
            metadata.insert(key.to_string(), json!(value));
        }
    }
}

fn parse_xml(text: &str, context: &str) -> Result<XmlNode> {
    let mut reader = XmlReader::from_str(text);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<XmlNode>::new();
    let mut root = XmlNode {
        name: "document".to_string(),
        ..XmlNode::default()
    };

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => stack.push(node_from_start(&event, context)?),
            Ok(Event::Empty(event)) => {
                let node = node_from_start(&event, context)?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    root.children.push(node);
                }
            }
            Ok(Event::Text(event)) => {
                let text = event.decode().map_err(|error| {
                    Error::InvalidRecord(format!("{context} text error: {error}"))
                })?;
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&text);
                }
            }
            Ok(Event::CData(event)) => {
                let text = event.decode().map_err(|error| {
                    Error::InvalidRecord(format!("{context} CDATA error: {error}"))
                })?;
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&text);
                }
            }
            Ok(Event::End(_)) => {
                let Some(node) = stack.pop() else {
                    return Err(Error::InvalidRecord(format!(
                        "{context} has an unmatched closing tag"
                    )));
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
                    "{context} parse error: {error}"
                )));
            }
            _ => {}
        }
    }

    if !stack.is_empty() {
        return Err(Error::InvalidRecord(format!(
            "{context} ended before all tags were closed"
        )));
    }

    Ok(root)
}

fn node_from_start(event: &BytesStart<'_>, context: &str) -> Result<XmlNode> {
    let mut attrs = BTreeMap::new();
    for attr in event.attributes().flatten() {
        let key = local_name(attr.key.as_ref());
        let value = attr
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|error| Error::InvalidRecord(format!("{context} attribute error: {error}")))?
            .into_owned();
        attrs.insert(key, value);
    }
    Ok(XmlNode {
        name: local_name(event.name().as_ref()),
        attrs,
        text: String::new(),
        children: Vec::new(),
    })
}

fn local_name(name: &[u8]) -> String {
    let local = name
        .iter()
        .rposition(|byte| *byte == b':')
        .map_or(name, |index| &name[index + 1..]);
    String::from_utf8_lossy(local).into_owned()
}

fn attr<'a>(node: &'a XmlNode, key: &str) -> Option<&'a str> {
    node.attrs.get(key).map(String::as_str)
}

fn direct_child<'a>(node: &'a XmlNode, name: &str) -> Option<&'a XmlNode> {
    node.children.iter().find(|child| child.name == name)
}

fn direct_child_text<'a>(node: &'a XmlNode, name: &str) -> Option<&'a str> {
    direct_child(node, name).map(|child| child.text.trim())
}

fn find_first(node: &XmlNode, predicate: impl Fn(&XmlNode) -> bool + Copy) -> Option<&XmlNode> {
    if predicate(node) {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| find_first(child, predicate))
}
