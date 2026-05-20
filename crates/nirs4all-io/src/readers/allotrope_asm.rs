use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SourceFile, SpectralRecord,
};
use serde_json::{json, Value};

use crate::readers::util::{
    normalize_key, read_text_lossy, safe_signal_name, signal_type_from_label, single_signal_record,
    SingleSignalSpec,
};
use crate::Reader;

pub struct AllotropeAsmReader;

impl Reader for AllotropeAsmReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::allotrope_asm"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        if path
            .extension()
            .and_then(|value| value.to_str())
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("json"))
        {
            return None;
        }
        let text = String::from_utf8_lossy(head);
        (text.contains("\"$asm.manifest\"") && text.contains("plate reader aggregate document"))
            .then(|| {
                FormatProbe::new(
                    "allotrope-asm-json",
                    self.name(),
                    Confidence::Definite,
                    "Allotrope Simple Model plate-reader JSON detected",
                )
            })
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let (text, source) = read_text_lossy(path)?;
        let root = serde_json::from_str::<Value>(&text)
            .map_err(|error| Error::InvalidRecord(format!("ASM JSON error: {error}")))?;
        read_asm_records(&root, source, self.name())
    }
}

fn read_asm_records(root: &Value, source: SourceFile, reader: &str) -> Result<Vec<SpectralRecord>> {
    let manifest = root
        .get("$asm.manifest")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let aggregate = root
        .get("plate reader aggregate document")
        .ok_or_else(|| Error::InvalidRecord("ASM missing plate reader aggregate".to_string()))?;
    let plate_docs = value_array(aggregate, "plate reader document")
        .ok_or_else(|| Error::InvalidRecord("ASM missing plate reader documents".to_string()))?;

    let system_metadata = system_metadata(aggregate);
    let mut records = Vec::new();
    for (plate_index, plate_doc) in plate_docs.iter().enumerate() {
        let Some(measurement_aggregate) = plate_doc.get("measurement aggregate document") else {
            continue;
        };
        let measurement_docs =
            value_array(measurement_aggregate, "measurement document").unwrap_or_default();
        for (measurement_index, measurement) in measurement_docs.iter().enumerate() {
            let metadata = base_metadata(
                manifest,
                plate_index,
                measurement_index,
                measurement_aggregate,
                measurement,
                &system_metadata,
            );
            if let Some(record) =
                cube_record(measurement, source.clone(), reader, metadata.clone())?
            {
                records.push(record);
                continue;
            }

            if let Some(record) = endpoint_record(measurement, source.clone(), reader, metadata)? {
                records.push(record);
            }
        }
    }

    if records.is_empty() {
        return Err(Error::InvalidRecord(
            "ASM JSON contains no supported spectral data cube or endpoint measurement".to_string(),
        ));
    }
    Ok(records)
}

fn cube_record(
    measurement: &Value,
    source: SourceFile,
    reader: &str,
    mut metadata: BTreeMap<String, Value>,
) -> Result<Option<SpectralRecord>> {
    let Some((cube_key, cube)) = measurement.as_object().and_then(|object| {
        object
            .iter()
            .find(|(key, _)| key.contains("spectrum data cube"))
    }) else {
        return Ok(None);
    };

    let dimensions = cube
        .pointer("/data/dimensions/0")
        .and_then(number_array)
        .ok_or_else(|| Error::InvalidRecord(format!("{cube_key} missing numeric dimensions")))?;
    let measures = cube
        .pointer("/data/measures/0")
        .and_then(number_array)
        .ok_or_else(|| Error::InvalidRecord(format!("{cube_key} missing numeric measures")))?;
    if dimensions.len() != measures.len() {
        return Err(Error::InvalidRecord(format!(
            "{cube_key} dimension and measure lengths differ"
        )));
    }

    let axis_unit = cube
        .pointer("/cube-structure/dimensions/0/unit")
        .and_then(Value::as_str)
        .unwrap_or("nm")
        .to_string();
    let axis_concept = cube
        .pointer("/cube-structure/dimensions/0/concept")
        .and_then(Value::as_str)
        .unwrap_or("wavelength");
    let measure_concept = cube
        .pointer("/cube-structure/measures/0/concept")
        .and_then(Value::as_str)
        .or_else(|| cube.get("label").and_then(Value::as_str))
        .unwrap_or("signal");
    let signal_unit = cube
        .pointer("/cube-structure/measures/0/unit")
        .and_then(Value::as_str)
        .map(str::to_string);
    let cube_label = cube.get("label").and_then(Value::as_str);
    let signal_label = asm_signal_label(measure_concept, cube_key, cube_label);
    let signal_type = signal_type_from_label(&signal_label);
    let signal_name = safe_signal_name(&signal_label, "signal");

    metadata.insert("asm_cube".to_string(), json!(cube_key));
    if let Some(label) = cube_label {
        metadata.insert("asm_cube_label".to_string(), json!(label));
    }
    let mut warnings = Vec::new();
    if signal_label != measure_concept {
        metadata.insert("asm_measure_concept".to_string(), json!(measure_concept));
        warnings.push(format!(
            "asm_signal_label_derived_from_cube_context:{signal_label}"
        ));
    }

    single_signal_record(
        "allotrope-asm-json",
        reader,
        source,
        SingleSignalSpec {
            axis_values: dimensions,
            axis_unit,
            axis_kind: axis_kind(axis_concept),
            values: measures,
            signal_name,
            signal_type,
            signal_unit,
            role: safe_signal_name(&signal_label, "signal"),
        },
        BTreeMap::new(),
        metadata,
        warnings,
    )
    .map(Some)
}

fn asm_signal_label(measure_concept: &str, cube_key: &str, cube_label: Option<&str>) -> String {
    let combined = format!(
        "{} {} {}",
        measure_concept,
        cube_key,
        cube_label.unwrap_or_default()
    )
    .to_ascii_lowercase();
    if combined.contains("fluorescence") {
        "fluorescence".to_string()
    } else if combined.contains("emission") {
        "emission".to_string()
    } else {
        measure_concept.to_string()
    }
}

fn endpoint_record(
    measurement: &Value,
    source: SourceFile,
    reader: &str,
    mut metadata: BTreeMap<String, Value>,
) -> Result<Option<SpectralRecord>> {
    let Some((signal_key, signal_value)) = endpoint_signal(measurement) else {
        return Ok(None);
    };
    let Some((wavelength, unit)) = detector_wavelength(measurement) else {
        return Ok(None);
    };

    metadata.insert("asm_endpoint".to_string(), json!(signal_key));
    let signal_type = signal_type_from_label(signal_key);
    let signal_name = safe_signal_name(signal_key, "signal");
    single_signal_record(
        "allotrope-asm-json",
        reader,
        source,
        SingleSignalSpec {
            axis_values: vec![wavelength],
            axis_unit: unit,
            axis_kind: AxisKind::Wavelength,
            values: vec![signal_value],
            signal_name,
            signal_type,
            signal_unit: measurement
                .get(signal_key)
                .and_then(|value| value.get("unit"))
                .and_then(Value::as_str)
                .map(str::to_string),
            role: safe_signal_name(signal_key, "signal"),
        },
        BTreeMap::new(),
        metadata,
        Vec::new(),
    )
    .map(Some)
}

fn base_metadata(
    manifest: &str,
    plate_index: usize,
    measurement_index: usize,
    measurement_aggregate: &Value,
    measurement: &Value,
    system_metadata: &BTreeMap<String, Value>,
) -> BTreeMap<String, Value> {
    let mut metadata = system_metadata.clone();
    if !manifest.is_empty() {
        metadata.insert("asm_manifest".to_string(), json!(manifest));
    }
    metadata.insert("plate_index".to_string(), json!(plate_index));
    metadata.insert("measurement_index".to_string(), json!(measurement_index));
    copy_string(
        &mut metadata,
        measurement_aggregate,
        "measurement time",
        "measurement_time",
    );
    copy_string(
        &mut metadata,
        measurement_aggregate,
        "container type",
        "container_type",
    );
    if let Some(count) = measurement_aggregate
        .pointer("/plate well count/value")
        .and_then(Value::as_f64)
    {
        metadata.insert("plate_well_count".to_string(), json!(count));
    }
    copy_string(
        &mut metadata,
        measurement,
        "measurement identifier",
        "measurement_id",
    );
    copy_sample_metadata(&mut metadata, measurement);
    copy_device_metadata(&mut metadata, measurement);
    copy_errors(&mut metadata, measurement);
    metadata
}

fn copy_sample_metadata(metadata: &mut BTreeMap<String, Value>, measurement: &Value) {
    let Some(sample) = measurement.get("sample document") else {
        return;
    };
    for (source_key, metadata_key) in [
        ("sample identifier", "sample_id"),
        ("location identifier", "location_id"),
        ("well plate identifier", "well_plate_id"),
    ] {
        copy_string(metadata, sample, source_key, metadata_key);
    }
    if let Some(group) = sample
        .pointer("/custom information document/group identifier")
        .and_then(Value::as_str)
    {
        metadata.insert("group_id".to_string(), json!(group));
    }
}

fn copy_device_metadata(metadata: &mut BTreeMap<String, Value>, measurement: &Value) {
    let Some(device_control) = first_device_control(measurement) else {
        return;
    };
    for (source_key, metadata_key) in [
        ("device type", "device_type"),
        ("detection type", "detection_type"),
        ("detector carriage speed setting", "detector_carriage_speed"),
        ("detector gain setting", "detector_gain"),
        (
            "scan position setting (plate reader)",
            "plate_reader_scan_position",
        ),
    ] {
        copy_string(metadata, device_control, source_key, metadata_key);
    }
    for (source_key, metadata_key) in [
        (
            "detector distance setting (plate reader)",
            "detector_distance",
        ),
        ("number of averages", "number_of_averages"),
        ("detector bandwidth setting", "detector_bandwidth"),
        ("excitation wavelength setting", "excitation_wavelength"),
        ("excitation bandwidth setting", "excitation_bandwidth"),
    ] {
        copy_quantity(metadata, device_control, source_key, metadata_key);
    }
}

fn copy_errors(metadata: &mut BTreeMap<String, Value>, measurement: &Value) {
    let Some(errors) = measurement
        .pointer("/error aggregate document/error document")
        .and_then(Value::as_array)
    else {
        return;
    };
    let summarized = errors
        .iter()
        .filter_map(|error| {
            let code = error.get("error").and_then(Value::as_str)?;
            let feature = error.get("error feature").and_then(Value::as_str);
            Some(match feature {
                Some(feature) => format!("{code}:{feature}"),
                None => code.to_string(),
            })
        })
        .collect::<Vec<_>>();
    if !summarized.is_empty() {
        metadata.insert("asm_errors".to_string(), json!(summarized));
    }
}

fn system_metadata(aggregate: &Value) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    if let Some(system) = aggregate.get("data system document") {
        for key in [
            "ASM file identifier",
            "ASM converter name",
            "ASM converter version",
            "software name",
            "software version",
        ] {
            if let Some(value) = system.get(key).and_then(Value::as_str) {
                metadata.insert(format!("data_system_{}", normalize_key(key)), json!(value));
            }
        }
    }
    if let Some(system) = aggregate.get("device system document") {
        for key in [
            "device identifier",
            "model number",
            "equipment serial number",
        ] {
            if let Some(value) = system.get(key).and_then(Value::as_str) {
                metadata.insert(
                    format!("device_system_{}", normalize_key(key)),
                    json!(value),
                );
            }
        }
    }
    metadata
}

fn endpoint_signal(measurement: &Value) -> Option<(&str, f64)> {
    for key in ["absorbance", "transmittance", "fluorescence"] {
        let value = measurement
            .get(key)
            .and_then(|node| node.get("value"))
            .and_then(Value::as_f64);
        if let Some(value) = value {
            return Some((key, value));
        }
    }
    None
}

fn detector_wavelength(measurement: &Value) -> Option<(f64, String)> {
    let control = first_device_control(measurement)?;
    let setting = control.get("detector wavelength setting")?;
    let wavelength = setting.get("value").and_then(Value::as_f64)?;
    let unit = setting
        .get("unit")
        .and_then(Value::as_str)
        .unwrap_or("nm")
        .to_string();
    Some((wavelength, unit))
}

fn first_device_control(measurement: &Value) -> Option<&Value> {
    measurement
        .pointer("/device control aggregate document/device control document")
        .and_then(Value::as_array)?
        .first()
}

fn value_array<'a>(value: &'a Value, key: &str) -> Option<&'a [Value]> {
    value.get(key).and_then(Value::as_array).map(Vec::as_slice)
}

fn number_array(value: &Value) -> Option<Vec<f64>> {
    value
        .as_array()?
        .iter()
        .map(Value::as_f64)
        .collect::<Option<Vec<_>>>()
}

fn axis_kind(concept: &str) -> AxisKind {
    let lower = concept.to_ascii_lowercase();
    if lower.contains("wavenumber") {
        AxisKind::Wavenumber
    } else if lower.contains("frequency") {
        AxisKind::Frequency
    } else {
        AxisKind::Wavelength
    }
}

fn copy_string(
    metadata: &mut BTreeMap<String, Value>,
    value: &Value,
    source_key: &str,
    metadata_key: &str,
) {
    if let Some(text) = value.get(source_key).and_then(Value::as_str) {
        metadata.insert(metadata_key.to_string(), json!(text));
    }
}

fn copy_quantity(
    metadata: &mut BTreeMap<String, Value>,
    value: &Value,
    source_key: &str,
    metadata_key: &str,
) {
    let Some(quantity) = value.get(source_key) else {
        return;
    };
    if let Some(number) = quantity.get("value").and_then(Value::as_f64) {
        metadata.insert(metadata_key.to_string(), json!(number));
    }
    if let Some(unit) = quantity.get("unit").and_then(Value::as_str) {
        metadata.insert(format!("{metadata_key}_unit"), json!(unit));
    }
}
