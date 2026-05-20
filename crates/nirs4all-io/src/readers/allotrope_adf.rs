use std::collections::BTreeMap;
use std::path::Path;

use hdf5_reader::group::Group;
use hdf5_reader::{Dataset, Hdf5File};
use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralRecord,
};
use serde_json::json;

use crate::readers::hdf5::read_numeric_vec;
use crate::readers::util::{safe_signal_name, single_signal_record, SingleSignalSpec};
use crate::Reader;

const HDF5_MAGIC: &[u8] = b"\x89HDF\r\n\x1a\n";

pub struct AllotropeAdfReader;

impl Reader for AllotropeAdfReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::allotrope_adf"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext != "adf" || !head.starts_with(HDF5_MAGIC) {
            return None;
        }
        Some(FormatProbe::new(
            "allotrope-adf",
            self.name(),
            Confidence::Likely,
            "Allotrope ADF HDF5 container detected; data-cube subset will be validated on read",
        ))
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let source = SourceFile::from_path(path, "primary")?;
        let file = Hdf5File::open(path)
            .map_err(|error| Error::InvalidRecord(format!("ADF HDF5 open error: {error}")))?;
        read_adf_records(&file, source, self.name())
    }
}

fn read_adf_records(
    file: &Hdf5File,
    source: SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    require_adf_groups(file)?;
    let semantics = read_adf_semantics(file);

    let data_cubes = file
        .group("/data-cubes")
        .map_err(|error| Error::InvalidRecord(format!("ADF data-cubes group error: {error}")))?;
    let mut cube_groups = data_cubes
        .groups()
        .map_err(|error| Error::InvalidRecord(format!("ADF cube traversal error: {error}")))?;
    cube_groups.sort_by(|a, b| a.name().cmp(b.name()));

    let context = AdfReadContext {
        semantics: &semantics,
        source: &source,
        reader,
    };
    let mut records = Vec::new();
    for cube in cube_groups {
        records.extend(read_cube_records(&cube, &context)?);
    }

    if records.is_empty() {
        return Err(Error::InvalidRecord(
            "ADF contains no numeric data-cube measures".to_string(),
        ));
    }
    Ok(records)
}

fn require_adf_groups(file: &Hdf5File) -> Result<()> {
    for path in [
        "/data-cubes",
        "/data-description",
        "/data-package",
        "/named-graphs",
    ] {
        file.group(path).map_err(|error| {
            Error::InvalidRecord(format!("ADF required group {path} is missing: {error}"))
        })?;
    }
    Ok(())
}

fn read_cube_records(cube: &Group, context: &AdfReadContext<'_>) -> Result<Vec<SpectralRecord>> {
    let Ok(measures) = cube.group("measures") else {
        return Ok(Vec::new());
    };
    let scales = cube.group("scales").ok();
    let mut measure_datasets = measures
        .datasets()
        .map_err(|error| Error::InvalidRecord(format!("ADF measures traversal error: {error}")))?;
    measure_datasets.sort_by(|a, b| a.name().cmp(b.name()));

    let mut records = Vec::new();
    for measure in measure_datasets {
        let shape = measure.shape();
        if shape.is_empty() || shape.len() > 2 {
            continue;
        }
        let values = read_numeric_vec(&measure, "ADF measure")?;
        let measure_records = records_from_measure(
            cube.name(),
            &measure,
            shape,
            values,
            scales.as_ref(),
            context,
        )?;
        records.extend(measure_records);
    }
    Ok(records)
}

fn records_from_measure(
    cube_id: &str,
    measure: &Dataset,
    shape: &[u64],
    values: Vec<f64>,
    scales: Option<&Group>,
    read_context: &AdfReadContext<'_>,
) -> Result<Vec<SpectralRecord>> {
    let primary_len = usize::try_from(shape[0])
        .map_err(|_| Error::InvalidRecord("ADF measure axis is too large".to_string()))?;
    let axis =
        find_scale(scales, primary_len, None, read_context.semantics, true)?.ok_or_else(|| {
            Error::InvalidRecord(
                "ADF primary axis lookup unexpectedly returned no scale".to_string(),
            )
        })?;
    let cube_semantics = read_context.semantics.cubes.get(cube_id).cloned();
    let measure_semantics = read_context.semantics.measures.get(measure.name()).cloned();

    if shape.len() == 1 {
        let context = RecordContext {
            cube_id,
            measure_id: measure.name(),
            shape,
            cube_semantics,
            measure_semantics,
            secondary_axis: None,
            semantics_decoded: read_context.semantics.decoded,
            semantics_warning: read_context.semantics.warning.clone(),
            source: read_context.source,
            reader: read_context.reader,
        };
        return Ok(vec![one_record(&context, 0, axis, values)?]);
    }

    let secondary_len = usize::try_from(shape[1]).map_err(|_| {
        Error::InvalidRecord("ADF secondary measure dimension is too large".to_string())
    })?;
    let secondary_axis = find_scale(
        scales,
        secondary_len,
        axis.scale_id.as_deref(),
        read_context.semantics,
        false,
    )?;
    if values.len() != primary_len * secondary_len {
        return Err(Error::InvalidRecord(
            "ADF measure payload length does not match dimensions".to_string(),
        ));
    }

    let mut records = Vec::with_capacity(secondary_len);
    let context = RecordContext {
        cube_id,
        measure_id: measure.name(),
        shape,
        cube_semantics,
        measure_semantics,
        secondary_axis,
        semantics_decoded: read_context.semantics.decoded,
        semantics_warning: read_context.semantics.warning.clone(),
        source: read_context.source,
        reader: read_context.reader,
    };
    for secondary_index in 0..secondary_len {
        let mut column = Vec::with_capacity(primary_len);
        for row in 0..primary_len {
            column.push(values[row * secondary_len + secondary_index]);
        }
        records.push(one_record(&context, secondary_index, axis.clone(), column)?);
    }
    Ok(records)
}

struct AdfReadContext<'a> {
    semantics: &'a AdfSemantics,
    source: &'a SourceFile,
    reader: &'a str,
}

struct RecordContext<'a> {
    cube_id: &'a str,
    measure_id: &'a str,
    shape: &'a [u64],
    cube_semantics: Option<AdfCubeSemantics>,
    measure_semantics: Option<AdfMeasureSemantics>,
    secondary_axis: Option<AdfAxis>,
    semantics_decoded: bool,
    semantics_warning: Option<String>,
    source: &'a SourceFile,
    reader: &'a str,
}

fn one_record(
    context: &RecordContext<'_>,
    secondary_index: usize,
    axis: AdfAxis,
    values: Vec<f64>,
) -> Result<SpectralRecord> {
    let mut metadata = BTreeMap::new();
    metadata.insert("container".to_string(), json!("allotrope-adf"));
    metadata.insert("cube_id".to_string(), json!(context.cube_id));
    metadata.insert("measure_id".to_string(), json!(context.measure_id));
    metadata.insert("measure_shape".to_string(), json!(context.shape));
    metadata.insert("secondary_index".to_string(), json!(secondary_index));
    metadata.insert("axis_source".to_string(), json!(axis.source));
    metadata.insert("axis_unit".to_string(), json!(axis.unit));
    metadata.insert("axis_kind".to_string(), json!(axis.kind.clone()));
    if let Some(scale_id) = axis.scale_id.as_deref() {
        metadata.insert("scale_id".to_string(), json!(scale_id));
    }
    if let Some(component_type) = axis.component_type.as_deref() {
        metadata.insert("adf_axis_component_type".to_string(), json!(component_type));
    }
    if let Some(order) = axis.order {
        metadata.insert("adf_axis_order".to_string(), json!(order));
    }
    if let Some(cube_semantics) = context.cube_semantics.as_ref() {
        if let Some(title) = cube_semantics.title.as_deref() {
            metadata.insert("cube_title".to_string(), json!(title));
        }
        if let Some(label) = cube_semantics.label.as_deref() {
            metadata.insert("cube_label".to_string(), json!(label));
        }
        if let Some(description) = cube_semantics.description.as_deref() {
            metadata.insert("cube_description".to_string(), json!(description));
        }
    }
    if let Some(measure_semantics) = context.measure_semantics.as_ref() {
        metadata.insert(
            "adf_measure_component_type".to_string(),
            json!(measure_semantics.component_type),
        );
    }
    if let Some(secondary_axis) = context.secondary_axis.as_ref() {
        metadata.insert(
            "secondary_axis_source".to_string(),
            json!(secondary_axis.source),
        );
        metadata.insert(
            "secondary_axis_unit".to_string(),
            json!(secondary_axis.unit),
        );
        metadata.insert(
            "secondary_axis_kind".to_string(),
            json!(secondary_axis.kind.clone()),
        );
        if let Some(scale_id) = secondary_axis.scale_id.as_deref() {
            metadata.insert("secondary_scale_id".to_string(), json!(scale_id));
        }
        if let Some(component_type) = secondary_axis.component_type.as_deref() {
            metadata.insert(
                "secondary_axis_component_type".to_string(),
                json!(component_type),
            );
        }
        if let Some(order) = secondary_axis.order {
            metadata.insert("secondary_axis_order".to_string(), json!(order));
        }
        if let Some(value) = secondary_axis.values.get(secondary_index) {
            metadata.insert("secondary_axis_value".to_string(), json!(value));
        }
    }

    let signal_type = context
        .measure_semantics
        .as_ref()
        .map(|semantics| signal_type_for_component(&semantics.component_type))
        .unwrap_or(SignalType::Unknown);
    let signal_unit = context
        .measure_semantics
        .as_ref()
        .and_then(|semantics| signal_unit_for_component(&semantics.component_type));
    let signal_name = signal_name(context.measure_id, signal_type.clone());
    let mut warnings = vec!["allotrope_adf_reverse_engineered_data_cube_subset".to_string()];
    if context.semantics_decoded {
        warnings.push("allotrope_adf_rdf_semantics_partially_mapped".to_string());
    } else {
        warnings.push("allotrope_adf_rdf_semantics_not_resolved".to_string());
    }
    if axis.component_type.as_deref() == Some("SecondTimeValue") {
        warnings.push("allotrope_adf_time_axis_mapped_as_index".to_string());
    }
    if let Some(warning) = context.semantics_warning.as_deref() {
        warnings.push(warning.to_string());
    }

    single_signal_record(
        "allotrope-adf",
        context.reader,
        context.source.clone(),
        SingleSignalSpec {
            axis_values: axis.values,
            axis_unit: axis.unit,
            axis_kind: axis.kind,
            values,
            signal_name,
            signal_type,
            signal_unit,
            role: "adf_measure".to_string(),
        },
        BTreeMap::new(),
        metadata,
        warnings,
    )
}

#[derive(Clone)]
struct AdfAxis {
    values: Vec<f64>,
    source: &'static str,
    scale_id: Option<String>,
    unit: String,
    kind: AxisKind,
    component_type: Option<String>,
    order: Option<u64>,
}

fn find_scale(
    scales: Option<&Group>,
    axis_len: usize,
    excluded_scale_id: Option<&str>,
    semantics: &AdfSemantics,
    allow_generated_index: bool,
) -> Result<Option<AdfAxis>> {
    if let Some(scales) = scales {
        let mut datasets = scales.datasets().map_err(|error| {
            Error::InvalidRecord(format!("ADF scales traversal error: {error}"))
        })?;
        datasets.sort_by(|a, b| a.name().cmp(b.name()));
        for dataset in datasets {
            if excluded_scale_id == Some(dataset.name()) {
                continue;
            }
            if dataset.ndim() == 1 && dataset.num_elements() == axis_len as u64 {
                let axis_semantics = semantics.scales.get(dataset.name());
                let (unit, kind) = axis_semantics
                    .map(|semantics| axis_mapping_for_component(&semantics.component_type))
                    .unwrap_or_else(|| ("index".to_string(), AxisKind::Index));
                return Ok(Some(AdfAxis {
                    values: read_numeric_vec(&dataset, "ADF scale")?,
                    source: "scale_dataset",
                    scale_id: Some(dataset.name().to_string()),
                    unit,
                    kind,
                    component_type: axis_semantics
                        .map(|semantics| semantics.component_type.clone()),
                    order: axis_semantics.and_then(|semantics| semantics.order),
                }));
            }
        }
    }
    if !allow_generated_index {
        return Ok(None);
    }
    Ok(Some(AdfAxis {
        values: (0..axis_len).map(|index| index as f64).collect(),
        source: "generated_index",
        scale_id: None,
        unit: "index".to_string(),
        kind: AxisKind::Index,
        component_type: None,
        order: None,
    }))
}

#[derive(Default)]
struct AdfSemantics {
    decoded: bool,
    warning: Option<String>,
    cubes: BTreeMap<String, AdfCubeSemantics>,
    measures: BTreeMap<String, AdfMeasureSemantics>,
    scales: BTreeMap<String, AdfScaleSemantics>,
}

#[derive(Clone, Default)]
struct AdfCubeSemantics {
    title: Option<String>,
    label: Option<String>,
    description: Option<String>,
}

#[derive(Clone)]
struct AdfMeasureSemantics {
    component_type: String,
}

#[derive(Clone)]
struct AdfScaleSemantics {
    component_type: String,
    order: Option<u64>,
}

#[derive(Clone)]
struct AdfComponentSemantics {
    component_type: String,
    is_measure: bool,
    is_dimension: bool,
    order: Option<u64>,
}

struct AdfTriple {
    subject: String,
    predicate: String,
    object: String,
}

fn read_adf_semantics(file: &Hdf5File) -> AdfSemantics {
    match decode_adf_semantics(file) {
        Ok(mut semantics) => {
            semantics.decoded = true;
            semantics
        }
        Err(message) => AdfSemantics {
            warning: Some(format!(
                "allotrope_adf_rdf_semantics_decode_failed:{message}"
            )),
            ..AdfSemantics::default()
        },
    }
}

fn decode_adf_semantics(file: &Hdf5File) -> std::result::Result<AdfSemantics, String> {
    let dictionary = read_adf_dictionary(file)?;
    let triples = read_adf_triples(file, &dictionary)?;

    let mut component_types: BTreeMap<String, String> = BTreeMap::new();
    let mut component_is_measure: BTreeMap<String, bool> = BTreeMap::new();
    let mut component_is_dimension: BTreeMap<String, bool> = BTreeMap::new();
    let mut component_orders: BTreeMap<String, u64> = BTreeMap::new();
    let mut rdf_mappings_by_subject: BTreeMap<String, String> = BTreeMap::new();
    let mut components_by_subject: BTreeMap<String, String> = BTreeMap::new();
    let mut datasets_by_rdf_mapping: BTreeMap<String, String> = BTreeMap::new();
    let mut semantics = AdfSemantics::default();

    for triple in &triples {
        match triple.predicate.as_str() {
            "title" => {
                semantics
                    .cubes
                    .entry(triple.subject.clone())
                    .or_default()
                    .title = Some(triple.object.clone());
            }
            "label" => {
                semantics
                    .cubes
                    .entry(triple.subject.clone())
                    .or_default()
                    .label = Some(triple.object.clone());
            }
            "description" => {
                semantics
                    .cubes
                    .entry(triple.subject.clone())
                    .or_default()
                    .description = Some(triple.object.clone());
            }
            "type" if triple.object == "Measure" => {
                component_is_measure.insert(triple.subject.clone(), true);
            }
            "type" if triple.object == "Dimension" => {
                component_is_dimension.insert(triple.subject.clone(), true);
            }
            "componentDataType" => {
                component_types.insert(triple.subject.clone(), triple.object.clone());
            }
            "order" => {
                if let Ok(order) = triple.object.parse::<u64>() {
                    component_orders.insert(triple.subject.clone(), order);
                }
            }
            "mapsComponent" => {
                components_by_subject.insert(triple.subject.clone(), triple.object.clone());
            }
            "rdfMapping" => {
                rdf_mappings_by_subject.insert(triple.subject.clone(), triple.object.clone());
            }
            "hdfDataset" => {
                datasets_by_rdf_mapping.insert(triple.subject.clone(), triple.object.clone());
            }
            _ => {}
        }
    }

    let mut components = BTreeMap::new();
    for (component_id, component_type) in component_types {
        components.insert(
            component_id.clone(),
            AdfComponentSemantics {
                component_type,
                is_measure: component_is_measure
                    .get(&component_id)
                    .copied()
                    .unwrap_or(false),
                is_dimension: component_is_dimension
                    .get(&component_id)
                    .copied()
                    .unwrap_or(false),
                order: component_orders.get(&component_id).copied(),
            },
        );
    }

    for (mapping_subject, rdf_mapping) in rdf_mappings_by_subject {
        let Some(component_id) = components_by_subject.get(&mapping_subject) else {
            continue;
        };
        let Some(dataset_id) = datasets_by_rdf_mapping.get(&rdf_mapping) else {
            continue;
        };
        let Some(component) = components.get(component_id) else {
            continue;
        };
        if component.is_measure {
            semantics.measures.insert(
                dataset_id.clone(),
                AdfMeasureSemantics {
                    component_type: component.component_type.clone(),
                },
            );
        } else if component.is_dimension {
            semantics.scales.insert(
                dataset_id.clone(),
                AdfScaleSemantics {
                    component_type: component.component_type.clone(),
                    order: component.order,
                },
            );
        }
    }

    Ok(semantics)
}

fn read_adf_dictionary(file: &Hdf5File) -> std::result::Result<Vec<String>, String> {
    let dictionary = file
        .group("/data-description/dictionary")
        .map_err(|error| format!("dictionary group: {error}"))?;
    let bytes = dictionary
        .dataset("bytes")
        .map_err(|error| format!("dictionary bytes dataset: {error}"))?
        .read_raw_bytes()
        .map_err(|error| format!("dictionary bytes read: {error}"))?;
    let keys_dataset = dictionary
        .dataset("keys")
        .map_err(|error| format!("dictionary keys dataset: {error}"))?;
    let keys_shape = keys_dataset.shape();
    if keys_shape.len() != 2 || keys_shape[1] != 13 {
        return Err(format!("unexpected dictionary keys shape {keys_shape:?}"));
    }
    let row_count =
        usize::try_from(keys_shape[0]).map_err(|_| "dictionary keys too large".to_string())?;
    let logical_size = dictionary
        .attribute("size")
        .ok()
        .and_then(|attribute| attribute.read_as_f64().ok())
        .filter(|value| *value >= 0.0)
        .map(|value| value as usize)
        .unwrap_or(row_count)
        .min(row_count);
    let keys = keys_dataset
        .read_raw_bytes()
        .map_err(|error| format!("dictionary keys read: {error}"))?;
    let mut out = Vec::with_capacity(logical_size);
    for index in 0..logical_size {
        let start = index
            .checked_mul(13)
            .ok_or_else(|| "dictionary key offset overflow".to_string())?;
        let end = start + 13;
        let row = keys
            .get(start..end)
            .ok_or_else(|| format!("dictionary key {index} is truncated"))?;
        out.push(decode_adf_dictionary_key(row, &bytes)?);
    }
    Ok(out)
}

fn decode_adf_dictionary_key(row: &[u8], bytes: &[u8]) -> std::result::Result<String, String> {
    if row.len() != 13 {
        return Err("dictionary key row is not 13 bytes".to_string());
    }
    let inline_len = row[12] as i8;
    let value = if inline_len >= 0 {
        let len = usize::from(inline_len as u8);
        row.get(..len)
            .ok_or_else(|| format!("inline dictionary key length {len} is invalid"))?
    } else {
        let offset = u64::from_be_bytes(
            row[0..8]
                .try_into()
                .map_err(|_| "dictionary key offset decode failed".to_string())?,
        );
        let len = u32::from_be_bytes(
            row[8..12]
                .try_into()
                .map_err(|_| "dictionary key length decode failed".to_string())?,
        );
        let offset =
            usize::try_from(offset).map_err(|_| "dictionary key offset too large".to_string())?;
        let len =
            usize::try_from(len).map_err(|_| "dictionary key length too large".to_string())?;
        bytes
            .get(offset..offset + len)
            .ok_or_else(|| "dictionary key points outside byte store".to_string())?
    };
    Ok(String::from_utf8_lossy(value).to_string())
}

fn read_adf_triples(
    file: &Hdf5File,
    dictionary: &[String],
) -> std::result::Result<Vec<AdfTriple>, String> {
    let quads = file
        .dataset("/data-description/quads")
        .map_err(|error| format!("quads dataset: {error}"))?;
    let shape = quads.shape();
    if shape.len() != 2 || shape[1] != 5 {
        return Err(format!("unexpected quads shape {shape:?}"));
    }
    let row_count = usize::try_from(shape[0]).map_err(|_| "quads rows too large".to_string())?;
    let logical_size = quads
        .attribute("size")
        .ok()
        .and_then(|attribute| attribute.read_as_f64().ok())
        .filter(|value| *value >= 0.0)
        .map(|value| value as usize)
        .unwrap_or(row_count)
        .min(row_count);
    let array = quads
        .read_array::<i64>()
        .map_err(|error| format!("quads read: {error}"))?;
    let values = array
        .as_slice_memory_order()
        .ok_or_else(|| "quads array is not contiguous".to_string())?;
    let mut triples = Vec::with_capacity(logical_size);
    for row_index in 0..logical_size {
        let start = row_index
            .checked_mul(5)
            .ok_or_else(|| "quad row offset overflow".to_string())?;
        let row = values
            .get(start..start + 5)
            .ok_or_else(|| format!("quad row {row_index} is truncated"))?;
        triples.push(AdfTriple {
            subject: adf_dictionary_value(dictionary, row[1])?.to_string(),
            predicate: adf_dictionary_value(dictionary, row[2])?.to_string(),
            object: adf_dictionary_value(dictionary, row[3])?.to_string(),
        });
    }
    Ok(triples)
}

fn adf_dictionary_value(dictionary: &[String], encoded: i64) -> std::result::Result<&str, String> {
    let index = (encoded as u64 & 0x7fff_ffff) as usize;
    dictionary.get(index).map(String::as_str).ok_or_else(|| {
        format!(
            "dictionary index {index} outside {} entries",
            dictionary.len()
        )
    })
}

fn axis_mapping_for_component(component_type: &str) -> (String, AxisKind) {
    match component_type {
        "NanometerValue" => ("nm".to_string(), AxisKind::Wavelength),
        "SecondTimeValue" => ("s".to_string(), AxisKind::Index),
        _ => ("index".to_string(), AxisKind::Index),
    }
}

fn signal_type_for_component(component_type: &str) -> SignalType {
    match component_type {
        "AbsorbanceUnitValue" => SignalType::Absorbance,
        _ => SignalType::Unknown,
    }
}

fn signal_unit_for_component(component_type: &str) -> Option<String> {
    match component_type {
        "AbsorbanceUnitValue" => Some("mAU".to_string()),
        _ => None,
    }
}

fn signal_name(measure_id: &str, signal_type: SignalType) -> String {
    match signal_type {
        SignalType::Absorbance => "absorbance".to_string(),
        _ => safe_signal_name(measure_id, "adf_measure"),
    }
}
