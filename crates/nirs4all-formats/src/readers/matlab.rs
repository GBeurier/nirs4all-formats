use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use flate2::read::ZlibDecoder;
use hdf5_reader::{Dataset, Datatype, H5Type, Hdf5File};
use matfile::{Array, MatFile, NumericData};
use nirs4all_formats_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SidecarResolver, SignalType, SourceFile,
    SpectralArray, SpectralAxis, SpectralRecord,
};
use rds2rust::{DataFrameData, ParseConfig, RObject, VectorData};
use serde_json::{json, Value};
use xz2::read::XzDecoder;

use crate::readers::hdf5_helpers::open_hdf5;
use crate::readers::util::{provenance, single_signal_record, SingleSignalSpec};
use crate::registry::ReadOptions;
use crate::sidecars::FsSidecars;
use crate::Reader;

const HDF5_MAGIC: &[u8] = b"\x89HDF\r\n\x1a\n";
const XZ_MAGIC: &[u8] = b"\xfd7zXZ\0";
const RDATA_XDR_MAGIC: &[u8] = b"RDX3\n";

pub struct MatlabReader;

impl Reader for MatlabReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::matlab"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if matches!(ext.as_str(), "rdata" | "rda") {
            if head.starts_with(XZ_MAGIC) {
                return Some(FormatProbe::new(
                    "rdata-rdx3-xz",
                    self.name(),
                    Confidence::Likely,
                    "R workspace extension with XZ compression detected; supported datasets will be validated on read",
                ));
            }
            if head.starts_with(RDATA_XDR_MAGIC) {
                return Some(FormatProbe::new(
                    "rdata-rdx3",
                    self.name(),
                    Confidence::Definite,
                    "R workspace RDX3 stream detected; supported datasets will be validated on read",
                ));
            }
            return None;
        }
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
        let base = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let sidecars: Arc<dyn SidecarResolver> = Arc::new(FsSidecars::new(base));
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        read_inner(self.name(), path, &bytes, &sidecars)
    }

    fn read_bytes_with_sidecars(
        &self,
        name: &Path,
        bytes: &[u8],
        sidecars: &Arc<dyn SidecarResolver>,
        _options: &ReadOptions,
    ) -> Result<Vec<SpectralRecord>> {
        read_inner(self.name(), name, bytes, sidecars)
    }
}

fn read_inner(
    reader_name: &'static str,
    name: &Path,
    bytes: &[u8],
    sidecars: &Arc<dyn SidecarResolver>,
) -> Result<Vec<SpectralRecord>> {
    let source = SourceFile::from_bytes(name, bytes, "primary");
    let ext = name
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(ext.as_str(), "rdata" | "rda") {
        read_rdata(bytes, source, reader_name)
    } else if bytes.starts_with(HDF5_MAGIC) {
        read_matlab_v73(bytes, source, reader_name, sidecars)
    } else {
        read_matlab_v5(name, bytes, source, reader_name, sidecars)
    }
}

fn read_rdata(bytes: &[u8], source: SourceFile, reader: &str) -> Result<Vec<SpectralRecord>> {
    let (decompressed, container) = if bytes.starts_with(XZ_MAGIC) {
        let mut decoder = XzDecoder::new(bytes);
        let mut out = Vec::new();
        decoder
            .read_to_end(&mut out)
            .map_err(|error| Error::InvalidRecord(format!("RData XZ decode error: {error}")))?;
        (out, "rdata_rdx3_xz")
    } else {
        (bytes.to_vec(), "rdata_rdx3")
    };

    let payload = if decompressed.starts_with(RDATA_XDR_MAGIC) {
        &decompressed[RDATA_XDR_MAGIC.len()..]
    } else if decompressed.starts_with(b"X\n") {
        decompressed.as_slice()
    } else {
        return Err(Error::InvalidRecord(
            "RData file is not an RDX3/XDR workspace stream".to_string(),
        ));
    };
    let object = rds2rust::read_rds_with_config(payload, ParseConfig::large_data())
        .map_err(|error| Error::InvalidRecord(format!("RData parse error: {error}")))?
        .object
        .into_concrete_deep();

    if let Some(records) = records_from_prospectr_nirsoil(&object, reader, source, container)? {
        return Ok(records);
    }

    Err(Error::InvalidRecord(
        "RData file contains no supported NIRS dataset".to_string(),
    ))
}

fn records_from_prospectr_nirsoil(
    object: &RObject,
    reader: &str,
    source: SourceFile,
    container: &str,
) -> Result<Option<Vec<SpectralRecord>>> {
    let Some(nirsoil) = rdata_binding(object, "NIRsoil") else {
        return Ok(None);
    };
    let RObject::DataFrame(df) = nirsoil else {
        return Err(Error::InvalidRecord(
            "prospectr NIRsoil binding is not an R data.frame".to_string(),
        ));
    };

    let nt = rdata_real_column(df, "Nt")?;
    let ciso = rdata_real_column(df, "Ciso")?;
    let cec = rdata_real_column(df, "CEC")?;
    let train = rdata_real_column(df, "train")?;
    let (spc, sample_count, band_count, axis) = rdata_real_matrix_column(df, "spc")?;

    if sample_count != 825 || band_count != 700 {
        return Err(Error::InvalidRecord(
            "prospectr NIRsoil spc matrix has unexpected dimensions".to_string(),
        ));
    }
    for (name, column) in [("Nt", nt), ("Ciso", ciso), ("CEC", cec), ("train", train)] {
        if column.len() != sample_count {
            return Err(Error::InvalidRecord(format!(
                "prospectr NIRsoil {name} length does not match spc rows"
            )));
        }
    }

    let mut records = Vec::with_capacity(sample_count);
    for sample_index in 0..sample_count {
        let mut targets = BTreeMap::new();
        targets.insert("Nt".to_string(), finite_json_or_null(nt[sample_index]));
        targets.insert("Ciso".to_string(), finite_json_or_null(ciso[sample_index]));
        targets.insert("CEC".to_string(), finite_json_or_null(cec[sample_index]));

        let is_train = train[sample_index].is_finite() && train[sample_index] != 0.0;
        let mut metadata = BTreeMap::new();
        metadata.insert("container".to_string(), json!(container));
        metadata.insert("dataset".to_string(), json!("prospectr_NIRsoil"));
        metadata.insert("sample_index".to_string(), json!(sample_index));
        metadata.insert("train".to_string(), json!(is_train));
        metadata.insert(
            "split".to_string(),
            json!(if is_train { "train" } else { "test" }),
        );

        records.push(single_signal_record(
            "rdata-prospectr-nirsoil",
            reader,
            source.clone(),
            SingleSignalSpec {
                axis_values: axis.clone(),
                axis_unit: "nm".to_string(),
                axis_kind: AxisKind::Wavelength,
                values: rdata_matrix_row(spc, sample_count, band_count, sample_index),
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

    Ok(Some(records))
}

fn rdata_binding<'a>(object: &'a RObject, name: &str) -> Option<&'a RObject> {
    match object {
        RObject::Pairlist(elements) => elements
            .iter()
            .find(|element| element.tag.as_deref() == Some(name))
            .map(|element| &element.value),
        _ => None,
    }
}

fn rdata_real_column<'a>(df: &'a DataFrameData, name: &str) -> Result<&'a [f64]> {
    let column = df.columns.get(name).ok_or_else(|| {
        Error::InvalidRecord(format!("prospectr NIRsoil data.frame has no {name} column"))
    })?;
    match column {
        RObject::Real(VectorData::Owned(values)) => Ok(values),
        other => Err(Error::InvalidRecord(format!(
            "prospectr NIRsoil {name} column is {}, not a numeric vector",
            other.variant_name()
        ))),
    }
}

fn rdata_real_matrix_column<'a>(
    df: &'a DataFrameData,
    name: &str,
) -> Result<(&'a [f64], usize, usize, Vec<f64>)> {
    let column = df.columns.get(name).ok_or_else(|| {
        Error::InvalidRecord(format!("prospectr NIRsoil data.frame has no {name} matrix"))
    })?;
    let RObject::WithAttributes { object, attributes } = column else {
        return Err(Error::InvalidRecord(format!(
            "prospectr NIRsoil {name} column is not an attributed matrix"
        )));
    };
    let values = match object.as_ref() {
        RObject::Real(VectorData::Owned(values)) => values.as_slice(),
        other => {
            return Err(Error::InvalidRecord(format!(
                "prospectr NIRsoil {name} matrix payload is {}, not numeric",
                other.variant_name()
            )))
        }
    };
    let dim = match attributes.get("dim") {
        Some(RObject::Integer(VectorData::Owned(dim))) if dim.len() == 2 => dim,
        _ => {
            return Err(Error::InvalidRecord(format!(
                "prospectr NIRsoil {name} matrix has no 2-D dim attribute"
            )))
        }
    };
    let rows = usize::try_from(dim[0]).map_err(|_| {
        Error::InvalidRecord(format!("prospectr NIRsoil {name} row count is negative"))
    })?;
    let cols = usize::try_from(dim[1]).map_err(|_| {
        Error::InvalidRecord(format!("prospectr NIRsoil {name} column count is negative"))
    })?;
    if values.len() != rows * cols {
        return Err(Error::InvalidRecord(format!(
            "prospectr NIRsoil {name} matrix payload length does not match dimensions"
        )));
    }
    let axis = rdata_matrix_axis(attributes.get("dimnames"), cols, name)?;
    Ok((values, rows, cols, axis))
}

fn rdata_matrix_axis(
    dimnames: Option<&RObject>,
    expected_len: usize,
    context: &str,
) -> Result<Vec<f64>> {
    let Some(RObject::List(dimnames)) = dimnames else {
        return Err(Error::InvalidRecord(format!(
            "prospectr NIRsoil {context} matrix has no dimnames axis"
        )));
    };
    let Some(RObject::Character(VectorData::Owned(columns))) = dimnames.get(1) else {
        return Err(Error::InvalidRecord(format!(
            "prospectr NIRsoil {context} matrix has no column dimnames axis"
        )));
    };
    if columns.len() != expected_len {
        return Err(Error::InvalidRecord(format!(
            "prospectr NIRsoil {context} column dimnames length does not match dimensions"
        )));
    }
    columns
        .iter()
        .map(|value| {
            value.as_ref().parse::<f64>().map_err(|error| {
                Error::InvalidRecord(format!(
                    "prospectr NIRsoil wavelength label {value:?} is not numeric: {error}"
                ))
            })
        })
        .collect()
}

fn rdata_matrix_row(values: &[f64], rows: usize, cols: usize, row: usize) -> Vec<f64> {
    (0..cols).map(|col| values[row + col * rows]).collect()
}

fn finite_json_or_null(value: f64) -> Value {
    if value.is_finite() {
        json!(value)
    } else {
        Value::Null
    }
}

fn read_matlab_v5(
    path: &Path,
    bytes: &[u8],
    source: SourceFile,
    reader: &str,
    sidecars: &Arc<dyn SidecarResolver>,
) -> Result<Vec<SpectralRecord>> {
    match read_matlab_v5_simple(bytes, source.clone(), reader) {
        Ok(records) => Ok(records),
        Err(_) => read_matlab_v5_structured(path, bytes, source, reader, sidecars),
    }
}

fn read_matlab_v5_simple(
    bytes: &[u8],
    source: SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    let mat = MatFile::parse(bytes)
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

fn read_matlab_v73(
    bytes: &[u8],
    source: SourceFile,
    reader: &str,
    sidecars: &Arc<dyn SidecarResolver>,
) -> Result<Vec<SpectralRecord>> {
    let file = open_hdf5(
        bytes.to_vec(),
        sidecars.clone(),
        "MATLAB v7.3 HDF5 open error",
    )?;
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

fn read_matlab_v5_structured(
    path: &Path,
    bytes: &[u8],
    source: SourceFile,
    reader: &str,
    sidecars: &Arc<dyn SidecarResolver>,
) -> Result<Vec<SpectralRecord>> {
    let mat = Mat5Document::parse(bytes)?;

    if let Some(records) = records_from_eigenvector_corn(&mat, reader, source.clone())? {
        return Ok(records);
    }
    if let Some(records) = records_from_nir_shootout(&mat, reader, source.clone())? {
        return Ok(records);
    }
    if let Some(records) = records_from_spectrochempy_dso(&mat, reader, source.clone())? {
        return Ok(records);
    }
    if let Some(records) = records_from_als2004(&mat, reader, source.clone())? {
        return Ok(records);
    }
    if let Some(records) = records_from_indian_pines(&mat, reader, source, path, sidecars)? {
        return Ok(records);
    }

    Err(Error::InvalidRecord(
        "MATLAB MAT file contains no supported structured NIRS dataset".to_string(),
    ))
}

fn records_from_eigenvector_corn(
    mat: &Mat5Document,
    reader: &str,
    source: SourceFile,
) -> Result<Option<Vec<SpectralRecord>>> {
    if !["m5spec", "mp5spec", "mp6spec", "propvals"]
        .iter()
        .all(|name| mat.values.contains_key(*name))
    {
        return Ok(None);
    }

    let spectra = ["m5spec", "mp5spec", "mp6spec"]
        .into_iter()
        .map(|name| {
            let value = mat.require(name)?;
            let data = require_numeric_field(value, "data", name)?;
            let axis = axis_from_dso(value, data.cols(), name)?;
            Ok((name.to_string(), data, axis))
        })
        .collect::<Result<Vec<_>>>()?;
    let target = require_numeric_field(mat.require("propvals")?, "data", "propvals")?;
    let sample_count = spectra[0].1.rows();
    if target.rows() != sample_count || target.cols() != 4 {
        return Err(Error::InvalidRecord(
            "Eigenvector Corn property matrix has unexpected dimensions".to_string(),
        ));
    }
    for (name, data, axis) in &spectra {
        if data.rows() != sample_count || data.cols() != axis.len() {
            return Err(Error::InvalidRecord(format!(
                "Eigenvector Corn {name} data dimensions do not match axis"
            )));
        }
    }

    let target_names = ["moisture", "oil", "protein", "starch"];
    let mut records = Vec::with_capacity(sample_count);
    for sample_index in 0..sample_count {
        let mut signals = BTreeMap::new();
        for (name, data, axis_values) in &spectra {
            signals.insert(
                name.clone(),
                spectral_array(
                    axis_values.clone(),
                    "nm",
                    AxisKind::Wavelength,
                    mat5_row(data, sample_index),
                    SignalType::Absorbance,
                    name,
                )?,
            );
        }
        let targets = targets_from_mat5_row(target, sample_index, &target_names);
        let mut metadata = BTreeMap::new();
        metadata.insert("container".to_string(), json!("matlab_v5_dso"));
        metadata.insert("dataset".to_string(), json!("eigenvector_corn"));
        metadata.insert("sample_index".to_string(), json!(sample_index));
        records.push(record_from_parts(
            "matlab-eigenvector-corn",
            reader,
            source.clone(),
            signals,
            SignalType::Absorbance,
            targets,
            metadata,
        )?);
    }
    Ok(Some(records))
}

fn records_from_nir_shootout(
    mat: &Mat5Document,
    reader: &str,
    source: SourceFile,
) -> Result<Option<Vec<SpectralRecord>>> {
    if !["calibrate_1", "calibrate_2", "calibrate_Y"]
        .iter()
        .all(|name| mat.values.contains_key(*name))
    {
        return Ok(None);
    }

    let target_names = ["weight", "hardness", "assay"];
    let mut records = Vec::new();
    for split in ["calibrate", "test", "validate"] {
        let first_name = format!("{split}_1");
        let second_name = format!("{split}_2");
        let target_name = format!("{split}_Y");
        let first = mat.require(&first_name)?;
        let second = mat.require(&second_name)?;
        let first_data = require_numeric_field(first, "data", &first_name)?;
        let second_data = require_numeric_field(second, "data", &second_name)?;
        let targets = require_numeric_field(mat.require(&target_name)?, "data", &target_name)?;
        let axis = axis_from_dso(first, first_data.cols(), &first_name)?;
        let sample_count = first_data.rows();
        if second_data.rows() != sample_count
            || second_data.cols() != first_data.cols()
            || targets.rows() != sample_count
            || targets.cols() != target_names.len()
        {
            return Err(Error::InvalidRecord(format!(
                "Eigenvector NIR Shootout split {split} dimensions are inconsistent"
            )));
        }

        for sample_index in 0..sample_count {
            let mut signals = BTreeMap::new();
            signals.insert(
                "instrument_1".to_string(),
                spectral_array(
                    axis.clone(),
                    "nm",
                    AxisKind::Wavelength,
                    mat5_row(first_data, sample_index),
                    SignalType::Absorbance,
                    "instrument_1",
                )?,
            );
            signals.insert(
                "instrument_2".to_string(),
                spectral_array(
                    axis.clone(),
                    "nm",
                    AxisKind::Wavelength,
                    mat5_row(second_data, sample_index),
                    SignalType::Absorbance,
                    "instrument_2",
                )?,
            );
            let mut metadata = BTreeMap::new();
            metadata.insert("container".to_string(), json!("matlab_v5_dso"));
            metadata.insert(
                "dataset".to_string(),
                json!("eigenvector_nir_shootout_2002"),
            );
            metadata.insert("split".to_string(), json!(split));
            metadata.insert("sample_index".to_string(), json!(sample_index));
            records.push(record_from_parts(
                "matlab-eigenvector-nir-shootout",
                reader,
                source.clone(),
                signals,
                SignalType::Absorbance,
                targets_from_mat5_row(targets, sample_index, &target_names),
                metadata,
            )?);
        }
    }
    Ok(Some(records))
}

fn records_from_spectrochempy_dso(
    mat: &Mat5Document,
    reader: &str,
    source: SourceFile,
) -> Result<Option<Vec<SpectralRecord>>> {
    let Some(value) = mat.values.get("X") else {
        return Ok(None);
    };
    let data = match numeric_field(value, "data") {
        Some(data) if data.rows() == 20 && data.cols() == 426 => data,
        _ => return Ok(None),
    };
    let axis = axis_from_dso(value, data.cols(), "X")?;
    let pressure = axis_from_dso(value, data.rows(), "X").ok();
    let dso_name = text_field(value, "name");

    let mut records = Vec::with_capacity(data.rows());
    for sample_index in 0..data.rows() {
        let mut metadata = BTreeMap::new();
        metadata.insert("container".to_string(), json!("matlab_v5_dso"));
        metadata.insert("dataset".to_string(), json!("spectrochempy_dso"));
        metadata.insert("sample_index".to_string(), json!(sample_index));
        if let Some(name) = &dso_name {
            metadata.insert("dso_name".to_string(), json!(name));
        }
        if let Some(values) = &pressure {
            metadata.insert("pressure_bar".to_string(), json!(values[sample_index]));
        }
        records.push(single_signal_record(
            "matlab-spectrochempy-dso",
            reader,
            source.clone(),
            SingleSignalSpec {
                axis_values: axis.clone(),
                axis_unit: "cm-1".to_string(),
                axis_kind: AxisKind::Wavenumber,
                values: mat5_row(data, sample_index),
                signal_name: "absorbance".to_string(),
                signal_type: SignalType::Absorbance,
                signal_unit: None,
                role: "absorbance".to_string(),
            },
            BTreeMap::new(),
            metadata,
            Vec::new(),
        )?);
    }
    Ok(Some(records))
}

fn records_from_als2004(
    mat: &Mat5Document,
    reader: &str,
    source: SourceFile,
) -> Result<Option<Vec<SpectralRecord>>> {
    let (Some(matrix), Some(targets)) = (
        mat.values.get("MATRIX").and_then(Mat5Value::as_numeric),
        mat.values.get("cpure").and_then(Mat5Value::as_numeric),
    ) else {
        return Ok(None);
    };
    if matrix.rows() != 204 || matrix.cols() != 96 || targets.rows() != 204 || targets.cols() != 4 {
        return Ok(None);
    }

    let axis = (1..=matrix.cols())
        .map(|value| value as f64)
        .collect::<Vec<_>>();
    let target_names = ["component_1", "component_2", "component_3", "component_4"];
    let mut records = Vec::with_capacity(matrix.rows());
    for sample_index in 0..matrix.rows() {
        let mut metadata = BTreeMap::new();
        metadata.insert("container".to_string(), json!("matlab_v5_matrix"));
        metadata.insert("dataset".to_string(), json!("spectrochempy_als2004"));
        metadata.insert("sample_index".to_string(), json!(sample_index));
        records.push(single_signal_record(
            "matlab-als2004",
            reader,
            source.clone(),
            SingleSignalSpec {
                axis_values: axis.clone(),
                axis_unit: "index".to_string(),
                axis_kind: AxisKind::Index,
                values: mat5_row(matrix, sample_index),
                signal_name: "signal".to_string(),
                signal_type: SignalType::Unknown,
                signal_unit: None,
                role: "signal".to_string(),
            },
            targets_from_mat5_row(targets, sample_index, &target_names),
            metadata,
            Vec::new(),
        )?);
    }
    Ok(Some(records))
}

fn records_from_indian_pines(
    mat: &Mat5Document,
    reader: &str,
    source: SourceFile,
    path: &Path,
    sidecars: &Arc<dyn SidecarResolver>,
) -> Result<Option<Vec<SpectralRecord>>> {
    let Some(cube) = mat
        .values
        .get("indian_pines_corrected")
        .and_then(Mat5Value::as_numeric)
    else {
        return Ok(None);
    };
    if cube.dims.as_slice() != [145, 145, 200] {
        return Err(Error::InvalidRecord(
            "Indian Pines corrected cube has unexpected dimensions".to_string(),
        ));
    }
    let (rows, cols, bands) = (cube.dims[0], cube.dims[1], cube.dims[2]);
    if cube.values.len() != rows * cols * bands {
        return Err(Error::InvalidRecord(
            "Indian Pines corrected cube payload length does not match dimensions".to_string(),
        ));
    }

    let gt = read_indian_pines_gt(path, rows, cols, sidecars)?;
    let axis = (0..bands).map(|value| value as f64).collect::<Vec<_>>();
    let mut records = Vec::with_capacity(rows * cols);
    for y in 0..rows {
        for x in 0..cols {
            let sample_index = y * cols + x;
            let mut targets = BTreeMap::new();
            if let Some((gt_values, _gt_source)) = &gt {
                targets.insert(
                    "land_cover_class".to_string(),
                    json!(gt_values[y + x * rows] as u64),
                );
            }

            let mut metadata = BTreeMap::new();
            metadata.insert(
                "container".to_string(),
                json!("matlab_v5_hyperspectral_cube"),
            );
            metadata.insert("dataset".to_string(), json!("indian_pines_corrected"));
            metadata.insert("sample_index".to_string(), json!(sample_index));
            metadata.insert("pixel_x".to_string(), json!(x));
            metadata.insert("pixel_y".to_string(), json!(y));
            metadata.insert("cube_rows".to_string(), json!(rows));
            metadata.insert("cube_cols".to_string(), json!(cols));
            metadata.insert("cube_bands".to_string(), json!(bands));

            let mut record = single_signal_record(
                "matlab-indian-pines-cube",
                reader,
                source.clone(),
                SingleSignalSpec {
                    axis_values: axis.clone(),
                    axis_unit: "index".to_string(),
                    axis_kind: AxisKind::Index,
                    values: indian_pines_pixel_values(cube, rows, cols, bands, y, x),
                    signal_name: "raw_counts".to_string(),
                    signal_type: SignalType::RawCounts,
                    signal_unit: Some("counts".to_string()),
                    role: "raw_counts".to_string(),
                },
                targets,
                metadata,
                vec!["matlab_hyperspectral_cube_axis_generated_index".to_string()],
            )?;
            if let Some((_gt_values, gt_source)) = &gt {
                record.provenance.sources.push(gt_source.clone());
            }
            records.push(record);
        }
    }
    Ok(Some(records))
}

fn read_indian_pines_gt(
    cube_path: &Path,
    rows: usize,
    cols: usize,
    sidecars: &Arc<dyn SidecarResolver>,
) -> Result<Option<(Vec<u16>, SourceFile)>> {
    let gt_rel = PathBuf::from("indian_pines_gt.mat");
    if !sidecars.contains(&gt_rel) {
        return Ok(None);
    }
    let bytes = sidecars.read(&gt_rel)?;
    let gt_display = cube_path
        .parent()
        .map(|p| p.join(&gt_rel))
        .unwrap_or_else(|| gt_rel.clone());
    let source = SourceFile::from_bytes(&gt_display, &bytes, "target_sidecar");
    let gt_mat = Mat5Document::parse(&bytes)?;
    let Some(gt) = gt_mat
        .values
        .get("indian_pines_gt")
        .and_then(Mat5Value::as_numeric)
    else {
        return Err(Error::InvalidRecord(
            "Indian Pines ground-truth sidecar has no indian_pines_gt matrix".to_string(),
        ));
    };
    if gt.dims.as_slice() != [rows, cols] || gt.values.len() != rows * cols {
        return Err(Error::InvalidRecord(
            "Indian Pines ground-truth dimensions do not match cube".to_string(),
        ));
    }
    let values = gt
        .values
        .iter()
        .map(|value| {
            if !value.is_finite()
                || *value < 0.0
                || *value > u16::MAX as f64
                || value.fract() != 0.0
            {
                return Err(Error::InvalidRecord(
                    "Indian Pines ground-truth class is out of range".to_string(),
                ));
            }
            Ok(*value as u16)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Some((values, source)))
}

fn indian_pines_pixel_values(
    cube: &Mat5Numeric,
    rows: usize,
    cols: usize,
    bands: usize,
    y: usize,
    x: usize,
) -> Vec<f64> {
    (0..bands)
        .map(|band| cube.values[y + x * rows + band * rows * cols])
        .collect()
}

fn spectral_array(
    axis_values: Vec<f64>,
    axis_unit: &str,
    axis_kind: AxisKind,
    values: Vec<f64>,
    signal_type: SignalType,
    role: &str,
) -> Result<SpectralArray> {
    SpectralArray::new(
        SpectralAxis::new(axis_values, axis_unit, axis_kind)?,
        values,
        vec!["x".to_string()],
        signal_type,
        None,
        role,
        "file",
    )
}

fn record_from_parts(
    format: &str,
    reader: &str,
    source: SourceFile,
    signals: BTreeMap<String, SpectralArray>,
    signal_type: SignalType,
    targets: BTreeMap<String, Value>,
    metadata: BTreeMap<String, Value>,
) -> Result<SpectralRecord> {
    let record = SpectralRecord {
        signals,
        signal_type,
        targets,
        metadata,
        provenance: provenance(format, reader, source, Vec::new()),
        quality_flags: Vec::new(),
    };
    record.validate()?;
    Ok(record)
}

fn require_numeric_field<'a>(
    value: &'a Mat5Value,
    field: &str,
    context: &str,
) -> Result<&'a Mat5Numeric> {
    numeric_field(value, field).ok_or_else(|| {
        Error::InvalidRecord(format!(
            "MATLAB structured dataset {context} has no numeric {field} field"
        ))
    })
}

fn numeric_field<'a>(value: &'a Mat5Value, field: &str) -> Option<&'a Mat5Numeric> {
    value.field(field).and_then(Mat5Value::as_numeric)
}

fn text_field(value: &Mat5Value, field: &str) -> Option<String> {
    value
        .field(field)
        .and_then(Mat5Value::as_text)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn axis_from_dso(value: &Mat5Value, len: usize, context: &str) -> Result<Vec<f64>> {
    let cell = value
        .field("axisscale")
        .and_then(Mat5Value::as_cell)
        .ok_or_else(|| Error::InvalidRecord(format!("MATLAB {context} has no axisscale cell")))?;
    cell.elements
        .iter()
        .filter_map(Mat5Value::as_numeric)
        .find(|numeric| numeric.values.len() == len)
        .map(|numeric| numeric.values.clone())
        .ok_or_else(|| {
            Error::InvalidRecord(format!("MATLAB {context} has no axis of length {len}"))
        })
}

fn mat5_row(matrix: &Mat5Numeric, row: usize) -> Vec<f64> {
    let rows = matrix.rows();
    (0..matrix.cols())
        .map(|col| matrix.values[row + col * rows])
        .collect()
}

fn targets_from_mat5_row(
    matrix: &Mat5Numeric,
    row: usize,
    names: &[&str],
) -> BTreeMap<String, Value> {
    let rows = matrix.rows();
    names
        .iter()
        .enumerate()
        .map(|(col, name)| ((*name).to_string(), json!(matrix.values[row + col * rows])))
        .collect()
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

struct Mat5Document {
    values: BTreeMap<String, Mat5Value>,
}

impl Mat5Document {
    fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 128 {
            return Err(Error::InvalidRecord(
                "MATLAB v5 MAT file is too short".to_string(),
            ));
        }
        let endian = match &bytes[126..128] {
            b"IM" => Mat5Endian::Little,
            b"MI" => Mat5Endian::Big,
            _ => {
                return Err(Error::InvalidRecord(
                    "MATLAB v5 MAT file has invalid endian marker".to_string(),
                ));
            }
        };
        let mut values = BTreeMap::new();
        for value in parse_mat5_elements(&bytes[128..], endian)? {
            if let Some(name) = value.name() {
                values.insert(name.to_string(), value);
            }
        }
        Ok(Self { values })
    }

    fn require(&self, name: &str) -> Result<&Mat5Value> {
        self.values.get(name).ok_or_else(|| {
            Error::InvalidRecord(format!("MATLAB structured dataset is missing {name}"))
        })
    }
}

struct Mat5Numeric {
    name: String,
    dims: Vec<usize>,
    values: Vec<f64>,
}

impl Mat5Numeric {
    fn rows(&self) -> usize {
        self.dims.first().copied().unwrap_or(0)
    }

    fn cols(&self) -> usize {
        self.dims.get(1).copied().unwrap_or(1)
    }
}

struct Mat5Cell {
    name: String,
    elements: Vec<Mat5Value>,
}

enum Mat5Value {
    Numeric(Mat5Numeric),
    Char {
        name: String,
        value: String,
    },
    Cell(Mat5Cell),
    Struct {
        name: String,
        fields: BTreeMap<String, Vec<Mat5Value>>,
    },
    Empty,
}

impl Mat5Value {
    fn name(&self) -> Option<&str> {
        match self {
            Self::Numeric(value) => Some(&value.name),
            Self::Char { name, .. } => Some(name),
            Self::Cell(value) => Some(&value.name),
            Self::Struct { name, .. } => Some(name),
            Self::Empty => None,
        }
    }

    fn as_numeric(&self) -> Option<&Mat5Numeric> {
        match self {
            Self::Numeric(value) => Some(value),
            _ => None,
        }
    }

    fn as_text(&self) -> Option<&str> {
        match self {
            Self::Char { value, .. } => Some(value),
            _ => None,
        }
    }

    fn as_cell(&self) -> Option<&Mat5Cell> {
        match self {
            Self::Cell(value) => Some(value),
            _ => None,
        }
    }

    fn field(&self, name: &str) -> Option<&Mat5Value> {
        match self {
            Self::Struct { fields, .. } => fields.get(name).and_then(|values| values.first()),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum Mat5Endian {
    Little,
    Big,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Mat5DataType {
    Int8,
    UInt8,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Single,
    Double,
    Int64,
    UInt64,
    Matrix,
    Compressed,
    Utf8,
    Utf16,
    Utf32,
}

impl Mat5DataType {
    fn from_u32(value: u32) -> Result<Self> {
        match value {
            1 => Ok(Self::Int8),
            2 => Ok(Self::UInt8),
            3 => Ok(Self::Int16),
            4 => Ok(Self::UInt16),
            5 => Ok(Self::Int32),
            6 => Ok(Self::UInt32),
            7 => Ok(Self::Single),
            9 => Ok(Self::Double),
            12 => Ok(Self::Int64),
            13 => Ok(Self::UInt64),
            14 => Ok(Self::Matrix),
            15 => Ok(Self::Compressed),
            16 => Ok(Self::Utf8),
            17 => Ok(Self::Utf16),
            18 => Ok(Self::Utf32),
            other => Err(Error::InvalidRecord(format!(
                "unsupported MATLAB v5 data type {other}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Mat5ArrayClass {
    Cell,
    Struct,
    Object,
    Char,
    Sparse,
    Double,
    Single,
    Int8,
    UInt8,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Int64,
    UInt64,
}

impl Mat5ArrayClass {
    fn from_u32(value: u32) -> Result<Self> {
        match value {
            1 => Ok(Self::Cell),
            2 => Ok(Self::Struct),
            3 => Ok(Self::Object),
            4 => Ok(Self::Char),
            5 => Ok(Self::Sparse),
            6 => Ok(Self::Double),
            7 => Ok(Self::Single),
            8 => Ok(Self::Int8),
            9 => Ok(Self::UInt8),
            10 => Ok(Self::Int16),
            11 => Ok(Self::UInt16),
            12 => Ok(Self::Int32),
            13 => Ok(Self::UInt32),
            14 => Ok(Self::Int64),
            15 => Ok(Self::UInt64),
            other => Err(Error::InvalidRecord(format!(
                "unsupported MATLAB v5 array class {other}"
            ))),
        }
    }
}

struct Mat5Tag {
    data_type: Mat5DataType,
    data_offset: usize,
    data_size: usize,
}

fn parse_mat5_elements(bytes: &[u8], endian: Mat5Endian) -> Result<Vec<Mat5Value>> {
    let mut values = Vec::new();
    let mut pos = 0_usize;
    while pos < bytes.len() {
        if bytes[pos..].iter().all(|value| *value == 0) {
            break;
        }
        let (tag, next_pos) = parse_mat5_tag(bytes, pos, endian)?;
        let data = mat5_tag_data(bytes, &tag)?;
        match tag.data_type {
            Mat5DataType::Matrix => values.push(parse_mat5_matrix(data, endian)?),
            Mat5DataType::Compressed => {
                let mut decoder = ZlibDecoder::new(data);
                let mut decoded = Vec::new();
                decoder.read_to_end(&mut decoded).map_err(|error| {
                    Error::InvalidRecord(format!("MATLAB v5 zlib decode error: {error}"))
                })?;
                values.extend(parse_mat5_elements(&decoded, endian)?);
            }
            _ => {}
        }
        pos = next_pos;
    }
    Ok(values)
}

fn parse_mat5_matrix(bytes: &[u8], endian: Mat5Endian) -> Result<Mat5Value> {
    let mut cursor = 0_usize;
    let (flags_tag, flags_bytes) = read_mat5_subelement(bytes, &mut cursor, endian)?;
    if flags_tag.data_type != Mat5DataType::UInt32 || flags_bytes.len() < 8 {
        return Err(Error::InvalidRecord(
            "MATLAB v5 array flags are invalid".to_string(),
        ));
    }
    let flag_word = mat5_u32(&flags_bytes[0..4], endian);
    let class = Mat5ArrayClass::from_u32(flag_word & 0xff)?;

    let (dims_tag, dims_bytes) = read_mat5_subelement(bytes, &mut cursor, endian)?;
    if dims_tag.data_type != Mat5DataType::Int32 || !dims_bytes.len().is_multiple_of(4) {
        return Err(Error::InvalidRecord(
            "MATLAB v5 dimensions are invalid".to_string(),
        ));
    }
    let dims = dims_bytes
        .chunks_exact(4)
        .map(|chunk| mat5_i32(chunk, endian))
        .map(|value| {
            if value < 0 {
                Err(Error::InvalidRecord(
                    "MATLAB v5 dimensions contain negative values".to_string(),
                ))
            } else {
                Ok(value as usize)
            }
        })
        .collect::<Result<Vec<_>>>()?;

    let (name_tag, name_bytes) = read_mat5_subelement(bytes, &mut cursor, endian)?;
    if !matches!(name_tag.data_type, Mat5DataType::Int8 | Mat5DataType::UInt8) {
        return Err(Error::InvalidRecord(
            "MATLAB v5 array name is invalid".to_string(),
        ));
    }
    let name = mat5_ascii(name_bytes)?;

    match class {
        Mat5ArrayClass::Cell => parse_mat5_cell(name, dims, bytes, &mut cursor, endian),
        Mat5ArrayClass::Struct => parse_mat5_struct_like(name, dims, bytes, &mut cursor, endian),
        Mat5ArrayClass::Object => {
            let _class_name = read_mat5_named_text(bytes, &mut cursor, endian)?;
            parse_mat5_struct_like(name, dims, bytes, &mut cursor, endian)
        }
        Mat5ArrayClass::Char => parse_mat5_char(name, dims, bytes, &mut cursor, endian),
        Mat5ArrayClass::Sparse => Err(Error::InvalidRecord(
            "MATLAB v5 sparse arrays are not supported in structured NIRS mapper".to_string(),
        )),
        _ => parse_mat5_numeric(name, dims, bytes, &mut cursor, endian),
    }
}

fn parse_mat5_struct_like(
    name: String,
    dims: Vec<usize>,
    bytes: &[u8],
    cursor: &mut usize,
    endian: Mat5Endian,
) -> Result<Mat5Value> {
    let (_, field_len_bytes) = read_mat5_subelement(bytes, cursor, endian)?;
    if field_len_bytes.len() < 4 {
        return Err(Error::InvalidRecord(
            "MATLAB v5 struct field-name length is invalid".to_string(),
        ));
    }
    let field_len = mat5_i32(&field_len_bytes[0..4], endian);
    if field_len <= 0 {
        return Err(Error::InvalidRecord(
            "MATLAB v5 struct field-name length is zero".to_string(),
        ));
    }
    let field_len = field_len as usize;
    let (_, field_bytes) = read_mat5_subelement(bytes, cursor, endian)?;
    if !field_bytes.len().is_multiple_of(field_len) {
        return Err(Error::InvalidRecord(
            "MATLAB v5 struct field-name block is misaligned".to_string(),
        ));
    }
    let field_names = field_bytes
        .chunks_exact(field_len)
        .map(mat5_ascii)
        .collect::<Result<Vec<_>>>()?;
    let element_count = dims_product(&dims)?;
    let mut fields = field_names
        .iter()
        .cloned()
        .map(|field_name| (field_name, Vec::with_capacity(element_count)))
        .collect::<BTreeMap<_, _>>();

    for _ in 0..element_count {
        for field_name in &field_names {
            let values = fields.get_mut(field_name).expect("field initialized");
            let (tag, data) = read_mat5_subelement(bytes, cursor, endian)?;
            if tag.data_type == Mat5DataType::Matrix && tag.data_size > 0 {
                values.push(parse_mat5_matrix(data, endian)?);
            } else {
                values.push(Mat5Value::Empty);
            }
        }
    }

    Ok(Mat5Value::Struct { name, fields })
}

fn parse_mat5_cell(
    name: String,
    dims: Vec<usize>,
    bytes: &[u8],
    cursor: &mut usize,
    endian: Mat5Endian,
) -> Result<Mat5Value> {
    let element_count = dims_product(&dims)?;
    let mut elements = Vec::with_capacity(element_count);
    for _ in 0..element_count {
        let (tag, data) = read_mat5_subelement(bytes, cursor, endian)?;
        if tag.data_type == Mat5DataType::Matrix && tag.data_size > 0 {
            elements.push(parse_mat5_matrix(data, endian)?);
        } else {
            elements.push(Mat5Value::Empty);
        }
    }
    Ok(Mat5Value::Cell(Mat5Cell { name, elements }))
}

fn parse_mat5_char(
    name: String,
    dims: Vec<usize>,
    bytes: &[u8],
    cursor: &mut usize,
    endian: Mat5Endian,
) -> Result<Mat5Value> {
    let (tag, data) = read_mat5_subelement(bytes, cursor, endian)?;
    let chars = mat5_chars(tag.data_type, data, endian)?;
    let value = mat5_char_matrix_to_string(&dims, chars);
    Ok(Mat5Value::Char { name, value })
}

fn parse_mat5_numeric(
    name: String,
    dims: Vec<usize>,
    bytes: &[u8],
    cursor: &mut usize,
    endian: Mat5Endian,
) -> Result<Mat5Value> {
    let (tag, data) = read_mat5_subelement(bytes, cursor, endian)?;
    let values = mat5_numeric_values(tag.data_type, data, endian)?;
    Ok(Mat5Value::Numeric(Mat5Numeric { name, dims, values }))
}

fn read_mat5_named_text(bytes: &[u8], cursor: &mut usize, endian: Mat5Endian) -> Result<String> {
    let (tag, data) = read_mat5_subelement(bytes, cursor, endian)?;
    match tag.data_type {
        Mat5DataType::Int8 | Mat5DataType::UInt8 => mat5_ascii(data),
        Mat5DataType::Utf8 | Mat5DataType::Utf16 | Mat5DataType::UInt16 => {
            Ok(mat5_chars(tag.data_type, data, endian)?
                .into_iter()
                .collect())
        }
        _ => Err(Error::InvalidRecord(
            "MATLAB v5 object class name is invalid".to_string(),
        )),
    }
}

fn read_mat5_subelement<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    endian: Mat5Endian,
) -> Result<(Mat5Tag, &'a [u8])> {
    let (tag, next_pos) = parse_mat5_tag(bytes, *cursor, endian)?;
    let data = mat5_tag_data(bytes, &tag)?;
    *cursor = next_pos;
    Ok((tag, data))
}

fn parse_mat5_tag(bytes: &[u8], pos: usize, endian: Mat5Endian) -> Result<(Mat5Tag, usize)> {
    if pos + 4 > bytes.len() {
        return Err(Error::InvalidRecord(
            "truncated MATLAB v5 data-element tag".to_string(),
        ));
    }
    let word = mat5_u32(&bytes[pos..pos + 4], endian);
    if word & 0xffff_0000 == 0 {
        if pos + 8 > bytes.len() {
            return Err(Error::InvalidRecord(
                "truncated MATLAB v5 data-element size".to_string(),
            ));
        }
        let data_type = Mat5DataType::from_u32(word)?;
        let data_size = mat5_u32(&bytes[pos + 4..pos + 8], endian) as usize;
        let data_offset = pos + 8;
        let next_pos = data_offset
            .checked_add(align_to(data_size, 8))
            .ok_or_else(|| Error::InvalidRecord("MATLAB v5 tag size overflow".to_string()))?;
        Ok((
            Mat5Tag {
                data_type,
                data_offset,
                data_size,
            },
            next_pos,
        ))
    } else {
        let data_type = Mat5DataType::from_u32(word & 0xffff)?;
        let data_size = ((word >> 16) & 0xffff) as usize;
        if data_size > 4 {
            return Err(Error::InvalidRecord(
                "invalid MATLAB v5 small data-element size".to_string(),
            ));
        }
        let data_offset = pos + 4;
        let next_pos = pos + 8;
        Ok((
            Mat5Tag {
                data_type,
                data_offset,
                data_size,
            },
            next_pos,
        ))
    }
}

fn mat5_tag_data<'a>(bytes: &'a [u8], tag: &Mat5Tag) -> Result<&'a [u8]> {
    let end = tag
        .data_offset
        .checked_add(tag.data_size)
        .ok_or_else(|| Error::InvalidRecord("MATLAB v5 data offset overflow".to_string()))?;
    bytes
        .get(tag.data_offset..end)
        .ok_or_else(|| Error::InvalidRecord("truncated MATLAB v5 data payload".to_string()))
}

fn mat5_ascii(bytes: &[u8]) -> Result<String> {
    let raw = bytes
        .iter()
        .copied()
        .take_while(|value| *value != 0)
        .collect::<Vec<_>>();
    String::from_utf8(raw).map_err(|error| Error::InvalidRecord(error.to_string()))
}

fn mat5_chars(data_type: Mat5DataType, bytes: &[u8], endian: Mat5Endian) -> Result<Vec<char>> {
    match data_type {
        Mat5DataType::Int8 | Mat5DataType::UInt8 | Mat5DataType::Utf8 => {
            Ok(bytes.iter().copied().map(char::from).collect::<Vec<char>>())
        }
        Mat5DataType::UInt16 | Mat5DataType::Utf16 => {
            if !bytes.len().is_multiple_of(2) {
                return Err(Error::InvalidRecord(
                    "MATLAB v5 UTF-16 char payload is misaligned".to_string(),
                ));
            }
            Ok(bytes
                .chunks_exact(2)
                .filter_map(|chunk| char::from_u32(mat5_u16(chunk, endian) as u32))
                .collect())
        }
        Mat5DataType::Utf32 => {
            if !bytes.len().is_multiple_of(4) {
                return Err(Error::InvalidRecord(
                    "MATLAB v5 UTF-32 char payload is misaligned".to_string(),
                ));
            }
            Ok(bytes
                .chunks_exact(4)
                .filter_map(|chunk| char::from_u32(mat5_u32(chunk, endian)))
                .collect())
        }
        _ => Err(Error::InvalidRecord(
            "MATLAB v5 char payload has unsupported type".to_string(),
        )),
    }
}

fn mat5_char_matrix_to_string(dims: &[usize], chars: Vec<char>) -> String {
    if dims.len() >= 2 && dims[0] > 1 {
        let rows = dims[0];
        let cols = dims[1];
        let mut out = String::new();
        for row in 0..rows {
            if row > 0 {
                out.push('\n');
            }
            for col in 0..cols {
                if let Some(ch) = chars.get(row + col * rows) {
                    out.push(*ch);
                }
            }
        }
        out.trim_end_matches(['\0', ' ']).to_string()
    } else {
        chars
            .into_iter()
            .collect::<String>()
            .trim_end_matches(['\0', ' '])
            .to_string()
    }
}

fn mat5_numeric_values(
    data_type: Mat5DataType,
    bytes: &[u8],
    endian: Mat5Endian,
) -> Result<Vec<f64>> {
    macro_rules! chunks {
        ($size:literal, $read:expr) => {{
            if !bytes.len().is_multiple_of($size) {
                return Err(Error::InvalidRecord(
                    "MATLAB v5 numeric payload is misaligned".to_string(),
                ));
            }
            bytes
                .chunks_exact($size)
                .map($read)
                .map(|value| value as f64)
                .collect()
        }};
    }

    Ok(match data_type {
        Mat5DataType::Int8 => bytes.iter().map(|value| *value as i8 as f64).collect(),
        Mat5DataType::UInt8 => bytes.iter().map(|value| f64::from(*value)).collect(),
        Mat5DataType::Int16 => chunks!(2, |chunk: &[u8]| mat5_i16(chunk, endian)),
        Mat5DataType::UInt16 => chunks!(2, |chunk: &[u8]| mat5_u16(chunk, endian)),
        Mat5DataType::Int32 => chunks!(4, |chunk: &[u8]| mat5_i32(chunk, endian)),
        Mat5DataType::UInt32 => chunks!(4, |chunk: &[u8]| mat5_u32(chunk, endian)),
        Mat5DataType::Single => chunks!(4, |chunk: &[u8]| mat5_f32(chunk, endian)),
        Mat5DataType::Double => chunks!(8, |chunk: &[u8]| mat5_f64(chunk, endian)),
        Mat5DataType::Int64 => chunks!(8, |chunk: &[u8]| mat5_i64(chunk, endian)),
        Mat5DataType::UInt64 => chunks!(8, |chunk: &[u8]| mat5_u64(chunk, endian)),
        _ => {
            return Err(Error::InvalidRecord(
                "MATLAB v5 numeric payload has unsupported storage type".to_string(),
            ));
        }
    })
}

fn dims_product(dims: &[usize]) -> Result<usize> {
    dims.iter().try_fold(1_usize, |acc, dim| {
        acc.checked_mul(*dim)
            .ok_or_else(|| Error::InvalidRecord("MATLAB v5 dimensions overflow".to_string()))
    })
}

fn align_to(value: usize, align: usize) -> usize {
    value.div_ceil(align) * align
}

fn mat5_u16(bytes: &[u8], endian: Mat5Endian) -> u16 {
    let raw = [bytes[0], bytes[1]];
    match endian {
        Mat5Endian::Little => u16::from_le_bytes(raw),
        Mat5Endian::Big => u16::from_be_bytes(raw),
    }
}

fn mat5_i16(bytes: &[u8], endian: Mat5Endian) -> i16 {
    let raw = [bytes[0], bytes[1]];
    match endian {
        Mat5Endian::Little => i16::from_le_bytes(raw),
        Mat5Endian::Big => i16::from_be_bytes(raw),
    }
}

fn mat5_u32(bytes: &[u8], endian: Mat5Endian) -> u32 {
    let raw = [bytes[0], bytes[1], bytes[2], bytes[3]];
    match endian {
        Mat5Endian::Little => u32::from_le_bytes(raw),
        Mat5Endian::Big => u32::from_be_bytes(raw),
    }
}

fn mat5_i32(bytes: &[u8], endian: Mat5Endian) -> i32 {
    let raw = [bytes[0], bytes[1], bytes[2], bytes[3]];
    match endian {
        Mat5Endian::Little => i32::from_le_bytes(raw),
        Mat5Endian::Big => i32::from_be_bytes(raw),
    }
}

fn mat5_u64(bytes: &[u8], endian: Mat5Endian) -> u64 {
    let raw = [
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ];
    match endian {
        Mat5Endian::Little => u64::from_le_bytes(raw),
        Mat5Endian::Big => u64::from_be_bytes(raw),
    }
}

fn mat5_i64(bytes: &[u8], endian: Mat5Endian) -> i64 {
    let raw = [
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ];
    match endian {
        Mat5Endian::Little => i64::from_le_bytes(raw),
        Mat5Endian::Big => i64::from_be_bytes(raw),
    }
}

fn mat5_f32(bytes: &[u8], endian: Mat5Endian) -> f32 {
    let raw = [bytes[0], bytes[1], bytes[2], bytes[3]];
    match endian {
        Mat5Endian::Little => f32::from_le_bytes(raw),
        Mat5Endian::Big => f32::from_be_bytes(raw),
    }
}

fn mat5_f64(bytes: &[u8], endian: Mat5Endian) -> f64 {
    let raw = [
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ];
    match endian {
        Mat5Endian::Little => f64::from_le_bytes(raw),
        Mat5Endian::Big => f64::from_be_bytes(raw),
    }
}
