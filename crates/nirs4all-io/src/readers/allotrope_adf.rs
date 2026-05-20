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

    let data_cubes = file
        .group("/data-cubes")
        .map_err(|error| Error::InvalidRecord(format!("ADF data-cubes group error: {error}")))?;
    let mut cube_groups = data_cubes
        .groups()
        .map_err(|error| Error::InvalidRecord(format!("ADF cube traversal error: {error}")))?;
    cube_groups.sort_by(|a, b| a.name().cmp(b.name()));

    let mut records = Vec::new();
    for cube in cube_groups {
        records.extend(read_cube_records(&cube, &source, reader)?);
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

fn read_cube_records(
    cube: &Group,
    source: &SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
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
            source,
            reader,
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
    source: &SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    let primary_len = usize::try_from(shape[0])
        .map_err(|_| Error::InvalidRecord("ADF measure axis is too large".to_string()))?;
    let axis = find_scale(scales, primary_len)?;

    if shape.len() == 1 {
        let context = RecordContext {
            cube_id,
            measure_id: measure.name(),
            shape,
            source,
            reader,
        };
        return Ok(vec![one_record(&context, 0, axis, values)?]);
    }

    let secondary_len = usize::try_from(shape[1]).map_err(|_| {
        Error::InvalidRecord("ADF secondary measure dimension is too large".to_string())
    })?;
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
        source,
        reader,
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

struct RecordContext<'a> {
    cube_id: &'a str,
    measure_id: &'a str,
    shape: &'a [u64],
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
    if let Some(scale_id) = axis.scale_id.as_deref() {
        metadata.insert("scale_id".to_string(), json!(scale_id));
    }

    let signal_name = safe_signal_name(context.measure_id, "adf_measure");
    single_signal_record(
        "allotrope-adf",
        context.reader,
        context.source.clone(),
        SingleSignalSpec {
            axis_values: axis.values,
            axis_unit: "index".to_string(),
            axis_kind: AxisKind::Index,
            values,
            signal_name,
            signal_type: SignalType::Unknown,
            signal_unit: None,
            role: "adf_measure".to_string(),
        },
        BTreeMap::new(),
        metadata,
        vec![
            "allotrope_adf_reverse_engineered_data_cube_subset".to_string(),
            "allotrope_adf_rdf_semantics_not_resolved".to_string(),
        ],
    )
}

#[derive(Clone)]
struct AdfAxis {
    values: Vec<f64>,
    source: &'static str,
    scale_id: Option<String>,
}

fn find_scale(scales: Option<&Group>, axis_len: usize) -> Result<AdfAxis> {
    if let Some(scales) = scales {
        let mut datasets = scales.datasets().map_err(|error| {
            Error::InvalidRecord(format!("ADF scales traversal error: {error}"))
        })?;
        datasets.sort_by(|a, b| a.name().cmp(b.name()));
        for dataset in datasets {
            if dataset.ndim() == 1 && dataset.num_elements() == axis_len as u64 {
                return Ok(AdfAxis {
                    values: read_numeric_vec(&dataset, "ADF scale")?,
                    source: "scale_dataset",
                    scale_id: Some(dataset.name().to_string()),
                });
            }
        }
    }
    Ok(AdfAxis {
        values: (0..axis_len).map(|index| index as f64).collect(),
        source: "generated_index",
        scale_id: None,
    })
}
