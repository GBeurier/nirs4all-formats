use std::collections::BTreeMap;
use std::path::Path;

use hdf5_reader::group::Group;
use hdf5_reader::{Attribute, Dataset, Datatype, H5Type, Hdf5File};
use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SourceFile, SpectralRecord,
};
use serde_json::{json, Value};

use crate::readers::util::{
    safe_signal_name, signal_type_from_label, single_signal_record, SingleSignalSpec,
};
use crate::Reader;

const HDF5_MAGIC: &[u8] = b"\x89HDF\r\n\x1a\n";

pub struct Hdf5Reader;

impl Reader for Hdf5Reader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::hdf5"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "h5" | "hdf5") || !head.starts_with(HDF5_MAGIC) {
            return None;
        }
        Some(FormatProbe::new(
            "hdf5-nirs-container",
            self.name(),
            Confidence::Likely,
            "HDF5 container detected; NIRS schema will be validated on read",
        ))
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let source = SourceFile::from_path(path, "primary")?;
        let file = Hdf5File::open(path)
            .map_err(|error| Error::InvalidRecord(format!("HDF5 open error: {error}")))?;
        read_hdf5_records(&file, source, self.name())
    }
}

fn read_hdf5_records(
    file: &Hdf5File,
    source: SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    let root = file
        .root_group()
        .map_err(|error| Error::InvalidRecord(format!("HDF5 root error: {error}")))?;
    let root_attributes = attribute_map(
        root.attributes()
            .map_err(|error| Error::InvalidRecord(format!("HDF5 attribute error: {error}")))?,
    );
    let candidate = find_candidate_group(&root, "/", 0)?.ok_or_else(|| {
        Error::InvalidRecord(
            "HDF5 contains no spectra dataset with matching wavelength axis".to_string(),
        )
    })?;

    let spectra_shape = candidate.spectra.shape();
    if spectra_shape.len() != 2 {
        return Err(Error::InvalidRecord(
            "HDF5 spectra dataset is not 2-D".to_string(),
        ));
    }
    let sample_count = usize::try_from(spectra_shape[0])
        .map_err(|_| Error::InvalidRecord("HDF5 sample dimension is too large".to_string()))?;
    let band_count = usize::try_from(spectra_shape[1])
        .map_err(|_| Error::InvalidRecord("HDF5 wavelength dimension is too large".to_string()))?;

    let axis = read_numeric_vec(&candidate.axis, "wavelength axis")?;
    if axis.len() != band_count {
        return Err(Error::InvalidRecord(
            "HDF5 axis length does not match spectra bands".to_string(),
        ));
    }
    let spectra = read_numeric_vec(&candidate.spectra, "spectra")?;
    if spectra.len() != sample_count * band_count {
        return Err(Error::InvalidRecord(
            "HDF5 spectra payload length does not match dimensions".to_string(),
        ));
    }

    let target_columns = target_columns(
        &candidate.group,
        sample_count,
        &candidate.axis_name,
        candidate.spectra.name(),
    )?;
    let group_attributes =
        attribute_map(candidate.group.attributes().map_err(|error| {
            Error::InvalidRecord(format!("HDF5 group attribute error: {error}"))
        })?);
    let signal_unit = attr_string(&candidate.spectra, "units");
    let signal_label = signal_unit.as_deref().unwrap_or("absorbance");
    let signal_type = signal_type_from_label(signal_label);
    let signal_name = safe_signal_name(signal_label, "absorbance");

    let mut records = Vec::with_capacity(sample_count);
    for sample_index in 0..sample_count {
        let start = sample_index * band_count;
        let end = start + band_count;
        let mut metadata = base_metadata(
            &candidate,
            &root_attributes,
            &group_attributes,
            signal_unit.as_deref(),
        );
        metadata.insert("sample_index".to_string(), json!(sample_index));
        let mut targets = BTreeMap::new();
        for (name, values) in &target_columns {
            targets.insert(name.clone(), json!(values[sample_index]));
        }

        records.push(single_signal_record(
            "hdf5-nirs",
            reader,
            source.clone(),
            SingleSignalSpec {
                axis_values: axis.clone(),
                axis_unit: attr_string(&candidate.axis, "units").unwrap_or_else(|| {
                    if candidate.axis_name.contains("wavenumber") {
                        "cm-1".to_string()
                    } else {
                        "nm".to_string()
                    }
                }),
                axis_kind: axis_kind(&candidate.axis_name, &candidate.axis),
                values: spectra[start..end].to_vec(),
                signal_name: signal_name.clone(),
                signal_type: signal_type.clone(),
                signal_unit: signal_unit.clone(),
                role: signal_name.clone(),
            },
            targets,
            metadata,
            Vec::new(),
        )?);
    }
    Ok(records)
}

struct CandidateGroup {
    group_path: String,
    group: Group,
    spectra: Dataset,
    axis_name: String,
    axis: Dataset,
}

fn find_candidate_group(
    group: &Group,
    group_path: &str,
    depth: usize,
) -> Result<Option<CandidateGroup>> {
    if let Ok(spectra) = group.dataset("spectra") {
        let shape = spectra.shape();
        if shape.len() != 2 {
            return Err(Error::InvalidRecord(
                "HDF5 spectra dataset is not 2-D".to_string(),
            ));
        }
        let band_count = usize::try_from(shape[1]).map_err(|_| {
            Error::InvalidRecord("HDF5 wavelength dimension is too large".to_string())
        })?;
        if let Some((axis_name, axis)) = find_axis_dataset(group, band_count)? {
            return Ok(Some(CandidateGroup {
                group_path: group_path.to_string(),
                group: group.clone(),
                spectra,
                axis_name,
                axis,
            }));
        }
        return Err(Error::InvalidRecord(
            "HDF5 contains no 1-D wavelength axis matching spectra bands".to_string(),
        ));
    }

    if depth >= 4 {
        return Ok(None);
    }
    let child_groups = group
        .groups()
        .map_err(|error| Error::InvalidRecord(format!("HDF5 group traversal error: {error}")))?;
    for child in child_groups {
        let child_path = join_hdf5_path(group_path, child.name());
        if let Some(candidate) = find_candidate_group(&child, &child_path, depth + 1)? {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

fn find_axis_dataset(group: &Group, band_count: usize) -> Result<Option<(String, Dataset)>> {
    for name in [
        "wavelengths",
        "wavelength",
        "wavelength_nm",
        "wavenumbers",
        "wavenumber",
        "x",
    ] {
        let Ok(dataset) = group.dataset(name) else {
            continue;
        };
        if dataset.ndim() == 1 && dataset.num_elements() == band_count as u64 {
            return Ok(Some((name.to_string(), dataset)));
        }
    }
    Ok(None)
}

fn target_columns(
    group: &Group,
    sample_count: usize,
    axis_name: &str,
    spectra_name: &str,
) -> Result<Vec<(String, Vec<f64>)>> {
    let datasets = group
        .datasets()
        .map_err(|error| Error::InvalidRecord(format!("HDF5 dataset traversal error: {error}")))?;
    let mut targets = Vec::new();
    for dataset in datasets {
        let name = dataset.name();
        if name == spectra_name
            || name == axis_name
            || matches!(
                name,
                "wavelengths" | "wavelength" | "wavelength_nm" | "wavenumbers" | "wavenumber" | "x"
            )
            || dataset.ndim() != 1
            || dataset.num_elements() != sample_count as u64
        {
            continue;
        }
        if let Ok(values) = read_numeric_vec(&dataset, name) {
            targets.push((name.to_string(), values));
        }
    }
    Ok(targets)
}

fn base_metadata(
    candidate: &CandidateGroup,
    root_attributes: &BTreeMap<String, Value>,
    group_attributes: &BTreeMap<String, Value>,
    signal_unit: Option<&str>,
) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    metadata.insert("container".to_string(), json!("hdf5"));
    metadata.insert("group_path".to_string(), json!(candidate.group_path));
    if !root_attributes.is_empty() {
        metadata.insert("root_attributes".to_string(), json!(root_attributes));
    }
    if !group_attributes.is_empty() && candidate.group_path != "/" {
        metadata.insert("group_attributes".to_string(), json!(group_attributes));
    }
    if let Some(unit) = signal_unit {
        metadata.insert("spectra_units".to_string(), json!(unit));
    }
    metadata
}

fn read_numeric_vec(dataset: &Dataset, context: &str) -> Result<Vec<f64>> {
    match dataset.dtype() {
        Datatype::FloatingPoint { size: 4, .. } => read_array_as_f64::<f32>(dataset, context),
        Datatype::FloatingPoint { size: 8, .. } => read_array_as_f64::<f64>(dataset, context),
        Datatype::FixedPoint {
            size: 1,
            signed: true,
            ..
        } => read_array_as_f64::<i8>(dataset, context),
        Datatype::FixedPoint {
            size: 1,
            signed: false,
            ..
        } => read_array_as_f64::<u8>(dataset, context),
        Datatype::FixedPoint {
            size: 2,
            signed: true,
            ..
        } => read_array_as_f64::<i16>(dataset, context),
        Datatype::FixedPoint {
            size: 2,
            signed: false,
            ..
        } => read_array_as_f64::<u16>(dataset, context),
        Datatype::FixedPoint {
            size: 4,
            signed: true,
            ..
        } => read_array_as_f64::<i32>(dataset, context),
        Datatype::FixedPoint {
            size: 4,
            signed: false,
            ..
        } => read_array_as_f64::<u32>(dataset, context),
        Datatype::FixedPoint {
            size: 8,
            signed: true,
            ..
        } => read_array_as_f64::<i64>(dataset, context),
        Datatype::FixedPoint {
            size: 8,
            signed: false,
            ..
        } => read_array_as_f64::<u64>(dataset, context),
        other => Err(Error::InvalidRecord(format!(
            "HDF5 {context} dataset has unsupported numeric dtype {other:?}"
        ))),
    }
}

fn read_array_as_f64<T>(dataset: &Dataset, context: &str) -> Result<Vec<f64>>
where
    T: H5Type + IntoF64,
{
    let array = dataset
        .read_array::<T>()
        .map_err(|error| Error::InvalidRecord(format!("HDF5 read error for {context}: {error}")))?;
    let values = array
        .as_slice_memory_order()
        .ok_or_else(|| Error::InvalidRecord(format!("HDF5 {context} array is not contiguous")))?;
    Ok(values.iter().cloned().map(IntoF64::into_f64).collect())
}

trait IntoF64 {
    fn into_f64(self) -> f64;
}

macro_rules! impl_into_f64 {
    ($($ty:ty),* $(,)?) => {
        $(
            impl IntoF64 for $ty {
                fn into_f64(self) -> f64 {
                    self as f64
                }
            }
        )*
    };
}

impl_into_f64!(f32, f64, i8, u8, i16, u16, i32, u32, i64, u64);

fn axis_kind(axis_name: &str, axis: &Dataset) -> AxisKind {
    let unit = attr_string(axis, "units")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let name = axis_name.to_ascii_lowercase();
    if name.contains("wavenumber") || unit.contains("cm") {
        AxisKind::Wavenumber
    } else if name.contains("wavelength") || unit.contains("nm") {
        AxisKind::Wavelength
    } else {
        AxisKind::Index
    }
}

fn attr_string(dataset: &Dataset, name: &str) -> Option<String> {
    dataset.attribute(name).ok()?.read_string().ok()
}

fn attribute_map(attributes: Vec<Attribute>) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for attribute in attributes {
        if let Some(value) = attribute_value(&attribute) {
            out.insert(attribute.name.clone(), value);
        }
    }
    out
}

fn attribute_value(attribute: &Attribute) -> Option<Value> {
    if let Ok(value) = attribute.read_string() {
        return Some(json!(value));
    }
    if let Ok(values) = attribute.read_strings() {
        if !values.is_empty() {
            return Some(json!(values));
        }
    }
    if attribute.num_elements() == 1 {
        if let Ok(value) = attribute.read_as_f64() {
            return Some(json!(value));
        }
    }
    if let Ok(values) = attribute.read_1d::<f64>() {
        return Some(json!(values));
    }
    if let Ok(values) = attribute.read_1d::<f32>() {
        return Some(json!(values.into_iter().map(f64::from).collect::<Vec<_>>()));
    }
    None
}

fn join_hdf5_path(parent: &str, child: &str) -> String {
    if parent == "/" {
        format!("/{child}")
    } else {
        format!("{parent}/{child}")
    }
}
