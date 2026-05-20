use std::collections::BTreeMap;
use std::path::Path;

use hdf5_reader::group::Group;
use hdf5_reader::{Attribute, Dataset, Datatype, H5Type, Hdf5File};
use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralArray,
    SpectralAxis, SpectralRecord,
};
use serde_json::{json, Value};

use crate::readers::util::{provenance, safe_signal_name, signal_type_from_label};
use crate::Reader;

const HDF5_MAGIC: &[u8] = b"\x89HDF\r\n\x1a\n";
const SPECTRA_DATASET_NAMES: &[&str] = &[
    "spectra",
    "spectrum",
    "X",
    "x",
    "absorbance",
    "absorbances",
    "reflectance",
    "reflectances",
    "transmittance",
    "transmission",
    "intensity",
    "intensities",
    "raw",
    "counts",
    "data",
];
const AXIS_DATASET_NAMES: &[&str] = &[
    "wavelengths",
    "wavelength",
    "wavelength_nm",
    "wavelengths_nm",
    "wl",
    "wl_nm",
    "lambda",
    "lambda_nm",
    "wavenumbers",
    "wavenumber",
    "wavenumber_cm-1",
    "wavenumber_cm_1",
    "wavenumbers_cm-1",
    "wavenumbers_cm_1",
    "wn",
    "wn_cm-1",
    "wn_cm_1",
    "x",
    "x_axis",
    "axis",
];

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

pub(crate) fn read_hdf5_records(
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

    let layout = candidate.layout;
    let sample_count = layout.sample_count;
    let band_count = layout.band_count;

    let axis = read_numeric_vec(&candidate.axis, "wavelength axis")?;
    if axis.len() != band_count {
        return Err(Error::InvalidRecord(
            "HDF5 axis length does not match spectra bands".to_string(),
        ));
    }
    let signal_payloads = read_signal_payloads(&candidate)?;

    let spectra_names = candidate
        .signals
        .iter()
        .map(|signal| signal.spectra_name.clone())
        .collect::<Vec<_>>();
    let target_columns = target_columns(
        &candidate.group,
        sample_count,
        &candidate.axis_name,
        &spectra_names,
    )?;
    let group_attributes =
        attribute_map(candidate.group.attributes().map_err(|error| {
            Error::InvalidRecord(format!("HDF5 group attribute error: {error}"))
        })?);
    let axis_kind = axis_kind(&candidate.axis_name, &candidate.axis);
    let axis_unit = attr_string(&candidate.axis, "units")
        .unwrap_or_else(|| default_axis_unit(&candidate.axis_name, &axis_kind));

    let mut records = Vec::with_capacity(sample_count);
    for sample_index in 0..sample_count {
        let mut metadata = base_metadata(
            &candidate,
            &root_attributes,
            &group_attributes,
            &signal_payloads,
        );
        metadata.insert("sample_index".to_string(), json!(sample_index));
        let mut targets = BTreeMap::new();
        for (name, values) in &target_columns {
            targets.insert(name.clone(), json!(values[sample_index]));
        }

        let mut signals = BTreeMap::new();
        let mut dominant = SignalType::Unknown;
        for payload in &signal_payloads {
            let signal_name = unique_signal_name(&payload.signal_name, &signals);
            if dominant == SignalType::Unknown {
                dominant = payload.signal_type.clone();
            }
            let signal = SpectralArray::new(
                SpectralAxis::new(axis.clone(), axis_unit.clone(), axis_kind.clone())?,
                sample_values(&payload.values, payload.layout, sample_index),
                vec!["x".to_string()],
                payload.signal_type.clone(),
                payload.signal_unit.clone(),
                signal_name.clone(),
                "file",
            )?;
            signals.insert(signal_name, signal);
        }
        let record = SpectralRecord {
            signals,
            signal_type: dominant,
            targets,
            metadata,
            provenance: provenance("hdf5-nirs", reader, source.clone(), Vec::new()),
            quality_flags: Vec::new(),
        };
        record.validate()?;
        records.push(record);
    }
    Ok(records)
}

struct CandidateGroup {
    group_path: String,
    group: Group,
    signals: Vec<CandidateSignal>,
    axis_name: String,
    axis: Dataset,
    layout: SpectraLayout,
}

struct CandidateSignal {
    spectra_name: String,
    spectra: Dataset,
    layout: SpectraLayout,
}

fn find_candidate_group(
    group: &Group,
    group_path: &str,
    depth: usize,
) -> Result<Option<CandidateGroup>> {
    if let Some(candidate) = find_candidate_in_group(group, group_path)? {
        return Ok(Some(candidate));
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

fn find_candidate_in_group(group: &Group, group_path: &str) -> Result<Option<CandidateGroup>> {
    let mut axis_name = None;
    let mut axis = None;
    let mut layout = None;
    let mut signals = Vec::new();

    for spectra_name in SPECTRA_DATASET_NAMES {
        let Ok(spectra) = group.dataset(spectra_name) else {
            continue;
        };
        if spectra.ndim() != 2 {
            continue;
        }

        let Some((candidate_axis_name, candidate_axis, candidate_layout)) =
            find_axis_dataset(group, &spectra)?
        else {
            continue;
        };

        if let Some(selected_axis_name) = axis_name.as_deref() {
            if selected_axis_name != candidate_axis_name || layout != Some(candidate_layout) {
                continue;
            }
        } else {
            axis_name = Some(candidate_axis_name);
            axis = Some(candidate_axis);
            layout = Some(candidate_layout);
        }

        signals.push(CandidateSignal {
            spectra_name: (*spectra_name).to_string(),
            spectra,
            layout: candidate_layout,
        });
    }

    if signals.is_empty() {
        Ok(None)
    } else {
        Ok(Some(CandidateGroup {
            group_path: group_path.to_string(),
            group: group.clone(),
            signals,
            axis_name: axis_name.expect("set with non-empty signals"),
            axis: axis.expect("set with non-empty signals"),
            layout: layout.expect("set with non-empty signals"),
        }))
    }
}

struct SignalPayload {
    spectra_name: String,
    signal_name: String,
    signal_type: SignalType,
    signal_unit: Option<String>,
    layout: SpectraLayout,
    values: Vec<f64>,
}

fn read_signal_payloads(candidate: &CandidateGroup) -> Result<Vec<SignalPayload>> {
    candidate
        .signals
        .iter()
        .map(|signal| {
            let values = read_numeric_vec(&signal.spectra, &signal.spectra_name)?;
            if values.len() != signal.layout.sample_count * signal.layout.band_count {
                return Err(Error::InvalidRecord(format!(
                    "HDF5 {} payload length does not match dimensions",
                    signal.spectra_name
                )));
            }
            let signal_unit = attr_string(&signal.spectra, "units");
            let signal_label = signal_unit
                .as_deref()
                .unwrap_or_else(|| default_signal_label(&signal.spectra_name));
            let signal_type = signal_type_from_label(signal_label);
            let signal_name = safe_signal_name(signal_label, "absorbance");
            Ok(SignalPayload {
                spectra_name: signal.spectra_name.clone(),
                signal_name,
                signal_type,
                signal_unit,
                layout: signal.layout,
                values,
            })
        })
        .collect()
}

fn find_axis_dataset(
    group: &Group,
    spectra: &Dataset,
) -> Result<Option<(String, Dataset, SpectraLayout)>> {
    let shape = spectra.shape();
    if shape.len() != 2 {
        return Ok(None);
    }
    let rows = usize::try_from(shape[0])
        .map_err(|_| Error::InvalidRecord("HDF5 spectra row dimension is too large".to_string()))?;
    let cols = usize::try_from(shape[1]).map_err(|_| {
        Error::InvalidRecord("HDF5 spectra column dimension is too large".to_string())
    })?;

    for name in AXIS_DATASET_NAMES {
        let Ok(dataset) = group.dataset(name) else {
            continue;
        };
        if dataset.ndim() != 1 {
            continue;
        }
        let axis_len = usize::try_from(dataset.num_elements())
            .map_err(|_| Error::InvalidRecord("HDF5 axis dimension is too large".to_string()))?;
        if let Some(layout) = infer_spectra_layout(rows, cols, axis_len) {
            return Ok(Some(((*name).to_string(), dataset, layout)));
        }
    }
    Ok(None)
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct SpectraLayout {
    sample_count: usize,
    band_count: usize,
    storage: SpectraStorage,
    name: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SpectraStorage {
    SamplesByBands,
    BandsBySamples,
}

fn infer_spectra_layout(rows: usize, cols: usize, axis_len: usize) -> Option<SpectraLayout> {
    if rows == 0 || cols == 0 || axis_len == 0 {
        return None;
    }
    match (cols == axis_len, rows == axis_len) {
        (true, false) => Some(SpectraLayout {
            sample_count: rows,
            band_count: cols,
            storage: SpectraStorage::SamplesByBands,
            name: "samples_by_bands",
        }),
        (false, true) => Some(SpectraLayout {
            sample_count: cols,
            band_count: rows,
            storage: SpectraStorage::BandsBySamples,
            name: "bands_by_samples",
        }),
        _ => None,
    }
}

fn sample_values(matrix: &[f64], layout: SpectraLayout, sample_index: usize) -> Vec<f64> {
    match layout.storage {
        SpectraStorage::SamplesByBands => {
            let start = sample_index * layout.band_count;
            matrix[start..start + layout.band_count].to_vec()
        }
        SpectraStorage::BandsBySamples => (0..layout.band_count)
            .map(|band_index| matrix[band_index * layout.sample_count + sample_index])
            .collect(),
    }
}

fn target_columns(
    group: &Group,
    sample_count: usize,
    axis_name: &str,
    spectra_names: &[String],
) -> Result<Vec<(String, Vec<f64>)>> {
    let datasets = group
        .datasets()
        .map_err(|error| Error::InvalidRecord(format!("HDF5 dataset traversal error: {error}")))?;
    let mut targets = Vec::new();
    for dataset in datasets {
        let name = dataset.name();
        if spectra_names
            .iter()
            .any(|spectra_name| spectra_name == name)
            || name == axis_name
            || is_axis_dataset_name(name)
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
    signals: &[SignalPayload],
) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    metadata.insert("container".to_string(), json!("hdf5"));
    metadata.insert("group_path".to_string(), json!(candidate.group_path));
    if signals.len() == 1 {
        if signals[0].spectra_name != "spectra" {
            metadata.insert(
                "spectra_dataset".to_string(),
                json!(signals[0].spectra_name),
            );
        }
    } else {
        metadata.insert(
            "signal_datasets".to_string(),
            json!(signals
                .iter()
                .map(|signal| signal.spectra_name.as_str())
                .collect::<Vec<_>>()),
        );
    }
    if candidate.axis_name != "wavelengths" {
        metadata.insert("axis_dataset".to_string(), json!(candidate.axis_name));
    }
    if candidate.layout.name != "samples_by_bands" {
        metadata.insert(
            "matrix_orientation".to_string(),
            json!(candidate.layout.name),
        );
    }
    if !root_attributes.is_empty() {
        metadata.insert("root_attributes".to_string(), json!(root_attributes));
    }
    if !group_attributes.is_empty() && candidate.group_path != "/" {
        metadata.insert("group_attributes".to_string(), json!(group_attributes));
    }
    let signal_units = signals
        .iter()
        .filter_map(|signal| {
            signal
                .signal_unit
                .as_ref()
                .map(|unit| (signal.signal_name.clone(), unit.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    if signals.len() == 1 {
        if let Some(unit) = signals[0].signal_unit.as_ref() {
            metadata.insert("spectra_units".to_string(), json!(unit));
        }
    } else if !signal_units.is_empty() {
        metadata.insert("signal_units".to_string(), json!(signal_units));
    }
    metadata
}

fn unique_signal_name(base: &str, signals: &BTreeMap<String, SpectralArray>) -> String {
    if !signals.contains_key(base) {
        return base.to_string();
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{base}_{suffix}");
        if !signals.contains_key(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

pub(crate) fn read_numeric_vec(dataset: &Dataset, context: &str) -> Result<Vec<f64>> {
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
    let compact_name = name.replace([' ', '_', '-'], "");
    let compact_unit = unit.replace([' ', '_'], "");
    if name.contains("wavenumber")
        || compact_name == "wn"
        || compact_name.starts_with("wncm")
        || compact_unit.contains("cm-1")
        || compact_unit.contains("cm^-1")
        || compact_unit.contains("1/cm")
        || unit.contains("cm")
    {
        AxisKind::Wavenumber
    } else if name.contains("wavelength")
        || compact_name == "wl"
        || compact_name.starts_with("wlnm")
        || compact_name.contains("lambda")
        || unit.contains("nm")
        || unit.contains("um")
    {
        AxisKind::Wavelength
    } else {
        AxisKind::Index
    }
}

fn default_axis_unit(axis_name: &str, axis_kind: &AxisKind) -> String {
    match axis_kind {
        AxisKind::Wavenumber => "cm-1".to_string(),
        AxisKind::Wavelength => "nm".to_string(),
        AxisKind::Index if axis_name == "x" => "nm".to_string(),
        _ => String::new(),
    }
}

fn default_signal_label(spectra_name: &str) -> &'static str {
    let name = spectra_name.to_ascii_lowercase();
    if name.contains("reflect") {
        "reflectance"
    } else if name.contains("trans") {
        "transmittance"
    } else if name.contains("raw") || name.contains("count") {
        "raw_counts"
    } else if name.contains("intens") {
        "intensity"
    } else {
        "absorbance"
    }
}

fn is_axis_dataset_name(name: &str) -> bool {
    AXIS_DATASET_NAMES.contains(&name)
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
