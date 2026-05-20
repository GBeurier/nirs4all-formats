use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use hdf5_reader::{Dataset, Datatype, H5Type, Hdf5File};
use matfile::{Array, MatFile, NumericData};
use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralRecord,
};
use serde_json::{json, Value};

use crate::readers::util::{single_signal_record, SingleSignalSpec};
use crate::Reader;

const HDF5_MAGIC: &[u8] = b"\x89HDF\r\n\x1a\n";

pub struct MatlabReader;

impl Reader for MatlabReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::matlab"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext != "mat" {
            return None;
        }
        if head.starts_with(HDF5_MAGIC) {
            return Some(FormatProbe::new(
                "matlab-v73-hdf5",
                self.name(),
                Confidence::Likely,
                "MATLAB v7.3 HDF5 container detected; NIRS schema will be validated on read",
            ));
        }
        if head.starts_with(b"MATLAB 5.0 MAT-file") {
            return Some(FormatProbe::new(
                "matlab-v5",
                self.name(),
                Confidence::Definite,
                "MATLAB v5 MAT-file detected; NIRS schema will be validated on read",
            ));
        }
        None
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let source = SourceFile::from_path(path, "primary")?;
        if has_hdf5_magic(path)? {
            read_matlab_v73(path, source, self.name())
        } else {
            read_matlab_v5(path, source, self.name())
        }
    }
}

fn has_hdf5_magic(path: &Path) -> Result<bool> {
    let mut file = File::open(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut head = [0_u8; HDF5_MAGIC.len()];
    let read = file.read(&mut head).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(read == HDF5_MAGIC.len() && head == HDF5_MAGIC)
}

fn read_matlab_v5(path: &Path, source: SourceFile, reader: &str) -> Result<Vec<SpectralRecord>> {
    let file = File::open(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mat = MatFile::parse(file)
        .map_err(|error| Error::InvalidRecord(format!("MATLAB v5 parse error: {error}")))?;
    let x = mat
        .find_by_name("X")
        .ok_or_else(|| Error::InvalidRecord("MATLAB MAT file contains no X matrix".to_string()))?;
    let axis = find_v5_array(&mat, &["wavelengths", "wavelength", "wavelength_nm", "x"])
        .ok_or_else(|| {
            Error::InvalidRecord("MATLAB MAT file contains no wavelength axis".to_string())
        })?;
    let target = mat.find_by_name("y");

    let shape = matrix_shape(x, "X")?;
    let axis_values = real_values(axis, axis.name())?;
    let target_values = target
        .map(|array| real_values(array, array.name()))
        .transpose()?;
    let layout = infer_layout(
        shape,
        axis_values.len(),
        target_values.as_ref().map(Vec::len),
    )?;
    let matrix = real_values(x, "X")?;
    validate_matrix_len(&matrix, layout)?;

    records_from_matrix(
        "matlab-mat-v5",
        reader,
        source,
        MatrixRecords {
            matrix,
            layout,
            axis_values,
            target_name: target.map(|array| array.name().to_string()),
            target_values,
            metadata: base_metadata("matlab_v5", "X", axis.name()),
        },
    )
}

fn read_matlab_v73(path: &Path, source: SourceFile, reader: &str) -> Result<Vec<SpectralRecord>> {
    let file = Hdf5File::open(path)
        .map_err(|error| Error::InvalidRecord(format!("MATLAB v7.3 HDF5 open error: {error}")))?;
    let x = file
        .dataset("/X")
        .map_err(|_| Error::InvalidRecord("MATLAB v7.3 file contains no /X dataset".to_string()))?;
    let axis = find_hdf5_axis(&file)?;
    let target = file.dataset("/y").ok();

    let shape = hdf5_matrix_shape(&x, "X")?;
    let axis_values = read_hdf5_numeric_vec(&axis, axis.name())?;
    let target_values = target
        .as_ref()
        .map(|dataset| read_hdf5_numeric_vec(dataset, dataset.name()))
        .transpose()?;
    let layout = infer_layout(
        shape,
        axis_values.len(),
        target_values.as_ref().map(Vec::len),
    )?;
    let matrix = read_hdf5_numeric_vec(&x, "X")?;
    validate_matrix_len(&matrix, layout)?;

    records_from_matrix(
        "matlab-mat-v73",
        reader,
        source,
        MatrixRecords {
            matrix,
            layout,
            axis_values,
            target_name: target.map(|dataset| dataset.name().trim_start_matches('/').to_string()),
            target_values,
            metadata: base_metadata("matlab_v73_hdf5", "X", axis.name()),
        },
    )
}

struct MatrixRecords {
    matrix: Vec<f64>,
    layout: MatrixLayout,
    axis_values: Vec<f64>,
    target_name: Option<String>,
    target_values: Option<Vec<f64>>,
    metadata: BTreeMap<String, Value>,
}

fn records_from_matrix(
    format: &str,
    reader: &str,
    source: SourceFile,
    input: MatrixRecords,
) -> Result<Vec<SpectralRecord>> {
    if let Some(values) = &input.target_values {
        if values.len() != input.layout.sample_count {
            return Err(Error::InvalidRecord(
                "MATLAB target length does not match sample count".to_string(),
            ));
        }
    }

    let mut records = Vec::with_capacity(input.layout.sample_count);
    for sample_index in 0..input.layout.sample_count {
        let mut metadata = input.metadata.clone();
        metadata.insert("sample_index".to_string(), json!(sample_index));
        metadata.insert("matrix_orientation".to_string(), json!(input.layout.name));

        let mut targets = BTreeMap::new();
        if let Some(values) = &input.target_values {
            targets.insert(
                input.target_name.clone().unwrap_or_else(|| "y".to_string()),
                json!(values[sample_index]),
            );
        }

        records.push(single_signal_record(
            format,
            reader,
            source.clone(),
            SingleSignalSpec {
                axis_values: input.axis_values.clone(),
                axis_unit: "nm".to_string(),
                axis_kind: AxisKind::Wavelength,
                values: sample_values(&input.matrix, input.layout, sample_index),
                signal_name: "absorbance".to_string(),
                signal_type: SignalType::Absorbance,
                signal_unit: None,
                role: "absorbance".to_string(),
            },
            targets,
            metadata,
            Vec::new(),
        )?);
    }
    Ok(records)
}

#[derive(Clone, Copy)]
struct MatrixLayout {
    sample_count: usize,
    band_count: usize,
    storage: MatrixStorage,
    name: &'static str,
}

#[derive(Clone, Copy)]
enum MatrixStorage {
    MatlabSamplesByBands,
    MatlabBandsBySamples,
    Hdf5SamplesByBands,
    Hdf5BandsBySamples,
}

fn infer_layout(
    shape: MatrixShape,
    axis_len: usize,
    target_len: Option<usize>,
) -> Result<MatrixLayout> {
    match (
        shape.storage,
        shape.rows == axis_len,
        shape.cols == axis_len,
    ) {
        (MatrixFileStorage::MatlabColumnMajor, false, true) => Ok(MatrixLayout {
            sample_count: shape.rows,
            band_count: shape.cols,
            storage: MatrixStorage::MatlabSamplesByBands,
            name: "samples_by_bands",
        }),
        (MatrixFileStorage::MatlabColumnMajor, true, false)
            if target_len.is_none_or(|count| count == shape.cols) =>
        {
            Ok(MatrixLayout {
                sample_count: shape.cols,
                band_count: shape.rows,
                storage: MatrixStorage::MatlabBandsBySamples,
                name: "bands_by_samples",
            })
        }
        (MatrixFileStorage::Hdf5RowMajor, false, true) => Ok(MatrixLayout {
            sample_count: shape.rows,
            band_count: shape.cols,
            storage: MatrixStorage::Hdf5SamplesByBands,
            name: "samples_by_bands",
        }),
        (MatrixFileStorage::Hdf5RowMajor, true, false)
            if target_len.is_none_or(|count| count == shape.cols) =>
        {
            Ok(MatrixLayout {
                sample_count: shape.cols,
                band_count: shape.rows,
                storage: MatrixStorage::Hdf5BandsBySamples,
                name: "bands_by_samples",
            })
        }
        _ => Err(Error::InvalidRecord(
            "MATLAB X dimensions do not match wavelength axis".to_string(),
        )),
    }
}

fn validate_matrix_len(matrix: &[f64], layout: MatrixLayout) -> Result<()> {
    let expected = layout
        .sample_count
        .checked_mul(layout.band_count)
        .ok_or_else(|| Error::InvalidRecord("MATLAB matrix dimensions overflow".to_string()))?;
    if matrix.len() != expected {
        return Err(Error::InvalidRecord(
            "MATLAB X payload length does not match dimensions".to_string(),
        ));
    }
    Ok(())
}

fn sample_values(matrix: &[f64], layout: MatrixLayout, sample_index: usize) -> Vec<f64> {
    match layout.storage {
        MatrixStorage::MatlabSamplesByBands => (0..layout.band_count)
            .map(|band_index| matrix[sample_index + band_index * layout.sample_count])
            .collect(),
        MatrixStorage::MatlabBandsBySamples => {
            let start = sample_index * layout.band_count;
            matrix[start..start + layout.band_count].to_vec()
        }
        MatrixStorage::Hdf5SamplesByBands => {
            let start = sample_index * layout.band_count;
            matrix[start..start + layout.band_count].to_vec()
        }
        MatrixStorage::Hdf5BandsBySamples => (0..layout.band_count)
            .map(|band_index| matrix[band_index * layout.sample_count + sample_index])
            .collect(),
    }
}

#[derive(Clone, Copy)]
struct MatrixShape {
    rows: usize,
    cols: usize,
    storage: MatrixFileStorage,
}

#[derive(Clone, Copy)]
enum MatrixFileStorage {
    MatlabColumnMajor,
    Hdf5RowMajor,
}

fn matrix_shape(array: &Array, name: &str) -> Result<MatrixShape> {
    let size = array.size();
    if size.len() != 2 {
        return Err(Error::InvalidRecord(format!(
            "MATLAB {name} matrix is not 2-D"
        )));
    }
    Ok(MatrixShape {
        rows: size[0],
        cols: size[1],
        storage: MatrixFileStorage::MatlabColumnMajor,
    })
}

fn hdf5_matrix_shape(dataset: &Dataset, name: &str) -> Result<MatrixShape> {
    let shape = dataset.shape();
    if shape.len() != 2 {
        return Err(Error::InvalidRecord(format!(
            "MATLAB {name} dataset is not 2-D"
        )));
    }
    Ok(MatrixShape {
        rows: usize::try_from(shape[0])
            .map_err(|_| Error::InvalidRecord(format!("MATLAB {name} rows exceed usize")))?,
        cols: usize::try_from(shape[1])
            .map_err(|_| Error::InvalidRecord(format!("MATLAB {name} columns exceed usize")))?,
        storage: MatrixFileStorage::Hdf5RowMajor,
    })
}

fn find_v5_array<'a>(mat: &'a MatFile, names: &[&str]) -> Option<&'a Array> {
    names.iter().find_map(|name| mat.find_by_name(name))
}

fn find_hdf5_axis(file: &Hdf5File) -> Result<Dataset> {
    for name in ["/wavelengths", "/wavelength", "/wavelength_nm", "/x"] {
        if let Ok(dataset) = file.dataset(name) {
            return Ok(dataset);
        }
    }
    Err(Error::InvalidRecord(
        "MATLAB v7.3 file contains no wavelength axis".to_string(),
    ))
}

fn real_values(array: &Array, name: &str) -> Result<Vec<f64>> {
    match array.data() {
        NumericData::Double { real, imag: None } => Ok(real.clone()),
        NumericData::Single { real, imag: None } => {
            Ok(real.iter().map(|value| f64::from(*value)).collect())
        }
        NumericData::Int8 { real, imag: None } => Ok(values_to_f64(real)),
        NumericData::UInt8 { real, imag: None } => Ok(values_to_f64(real)),
        NumericData::Int16 { real, imag: None } => Ok(values_to_f64(real)),
        NumericData::UInt16 { real, imag: None } => Ok(values_to_f64(real)),
        NumericData::Int32 { real, imag: None } => Ok(values_to_f64(real)),
        NumericData::UInt32 { real, imag: None } => Ok(values_to_f64(real)),
        NumericData::Int64 { real, imag: None } => Ok(values_to_f64(real)),
        NumericData::UInt64 { real, imag: None } => Ok(values_to_f64(real)),
        _ => Err(Error::InvalidRecord(format!(
            "MATLAB {name} array is complex or unsupported"
        ))),
    }
}

fn values_to_f64<T>(values: &[T]) -> Vec<f64>
where
    T: Copy + IntoF64,
{
    values.iter().copied().map(IntoF64::into_f64).collect()
}

fn read_hdf5_numeric_vec(dataset: &Dataset, context: &str) -> Result<Vec<f64>> {
    match dataset.dtype() {
        Datatype::FloatingPoint { size: 4, .. } => read_hdf5_array_as_f64::<f32>(dataset, context),
        Datatype::FloatingPoint { size: 8, .. } => read_hdf5_array_as_f64::<f64>(dataset, context),
        Datatype::FixedPoint {
            size: 1,
            signed: true,
            ..
        } => read_hdf5_array_as_f64::<i8>(dataset, context),
        Datatype::FixedPoint {
            size: 1,
            signed: false,
            ..
        } => read_hdf5_array_as_f64::<u8>(dataset, context),
        Datatype::FixedPoint {
            size: 2,
            signed: true,
            ..
        } => read_hdf5_array_as_f64::<i16>(dataset, context),
        Datatype::FixedPoint {
            size: 2,
            signed: false,
            ..
        } => read_hdf5_array_as_f64::<u16>(dataset, context),
        Datatype::FixedPoint {
            size: 4,
            signed: true,
            ..
        } => read_hdf5_array_as_f64::<i32>(dataset, context),
        Datatype::FixedPoint {
            size: 4,
            signed: false,
            ..
        } => read_hdf5_array_as_f64::<u32>(dataset, context),
        Datatype::FixedPoint {
            size: 8,
            signed: true,
            ..
        } => read_hdf5_array_as_f64::<i64>(dataset, context),
        Datatype::FixedPoint {
            size: 8,
            signed: false,
            ..
        } => read_hdf5_array_as_f64::<u64>(dataset, context),
        other => Err(Error::InvalidRecord(format!(
            "MATLAB {context} dataset has unsupported numeric dtype {other:?}"
        ))),
    }
}

fn read_hdf5_array_as_f64<T>(dataset: &Dataset, context: &str) -> Result<Vec<f64>>
where
    T: H5Type + IntoF64,
{
    let array = dataset.read_array::<T>().map_err(|error| {
        Error::InvalidRecord(format!("MATLAB read error for {context}: {error}"))
    })?;
    let values = array
        .as_slice_memory_order()
        .ok_or_else(|| Error::InvalidRecord(format!("MATLAB {context} array is not contiguous")))?;
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

fn base_metadata(
    container: &str,
    matrix_dataset: &str,
    axis_dataset: &str,
) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    metadata.insert("container".to_string(), json!(container));
    metadata.insert("matrix_dataset".to_string(), json!(matrix_dataset));
    metadata.insert(
        "axis_dataset".to_string(),
        json!(axis_dataset.trim_start_matches('/')),
    );
    metadata
}
