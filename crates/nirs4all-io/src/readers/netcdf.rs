use std::collections::BTreeMap;
use std::path::Path;

use netcdf_reader::{NcAttrValue, NcAttribute, NcFile, NcVariable};
use nirs4all_io_core::{AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile};
use serde_json::{json, Value};

use crate::readers::util::{single_signal_record, SingleSignalSpec};
use crate::Reader;

pub struct NetcdfReader;

impl Reader for NetcdfReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::netcdf"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "nc" | "cdf") {
            return None;
        }
        if head.starts_with(b"CDF\x01")
            || head.starts_with(b"CDF\x02")
            || head.starts_with(b"CDF\x05")
            || head.starts_with(b"\x89HDF\r\n\x1a\n")
        {
            Some(FormatProbe::new(
                "netcdf-container",
                self.name(),
                Confidence::Likely,
                "NetCDF container detected; NIRS schema will be validated on read",
            ))
        } else {
            None
        }
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let source = SourceFile::from_path(path, "primary")?;
        let file = NcFile::open(path)
            .map_err(|error| Error::InvalidRecord(format!("NetCDF open error: {error}")))?;
        read_netcdf_records(&file, source, self.name())
    }
}

fn read_netcdf_records(
    file: &NcFile,
    source: SourceFile,
    reader: &str,
) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
    let spectra_var = file
        .variable("spectra")
        .map_err(|_| Error::InvalidRecord("NetCDF contains no spectra variable".to_string()))?;
    let shape = spectra_var.shape();
    if shape.len() != 2 {
        return Err(Error::InvalidRecord(
            "NetCDF spectra variable is not 2-D".to_string(),
        ));
    }
    let sample_count = usize::try_from(shape[0])
        .map_err(|_| Error::InvalidRecord("NetCDF sample dimension is too large".to_string()))?;
    let band_count = usize::try_from(shape[1]).map_err(|_| {
        Error::InvalidRecord("NetCDF wavelength dimension is too large".to_string())
    })?;

    let (axis_name, axis_var) = find_axis_variable(file, band_count)?;
    let axis = read_f64_vec(file, axis_name)?;
    if axis.len() != band_count {
        return Err(Error::InvalidRecord(
            "NetCDF axis length does not match spectra bands".to_string(),
        ));
    }
    let spectra = read_f64_vec(file, "spectra")?;
    if spectra.len() != sample_count * band_count {
        return Err(Error::InvalidRecord(
            "NetCDF spectra payload length does not match dimensions".to_string(),
        ));
    }

    let target_columns = target_columns(file, sample_count, axis_name)?;
    let mut records = Vec::with_capacity(sample_count);
    for sample_index in 0..sample_count {
        let start = sample_index * band_count;
        let end = start + band_count;
        let mut metadata = base_metadata(file, spectra_var);
        metadata.insert("sample_index".to_string(), json!(sample_index));
        let mut targets = BTreeMap::new();
        for (name, values) in &target_columns {
            targets.insert(name.clone(), json!(values[sample_index]));
        }
        records.push(single_signal_record(
            "netcdf-nirs",
            reader,
            source.clone(),
            SingleSignalSpec {
                axis_values: axis.clone(),
                axis_unit: attr_string(axis_var, "units").unwrap_or_else(|| "index".to_string()),
                axis_kind: AxisKind::Wavelength,
                values: spectra[start..end].to_vec(),
                signal_name: "absorbance".to_string(),
                signal_type: SignalType::Absorbance,
                signal_unit: attr_string(spectra_var, "units"),
                role: "absorbance".to_string(),
            },
            targets,
            metadata,
            Vec::new(),
        )?);
    }
    Ok(records)
}

fn find_axis_variable(file: &NcFile, band_count: usize) -> Result<(&str, &NcVariable)> {
    for name in ["wavelengths", "wavelength", "wavelength_nm", "x"] {
        if let Ok(variable) = file.variable(name) {
            if variable.ndim() == 1 && variable.num_elements() == band_count as u64 {
                return Ok((variable.name(), variable));
            }
        }
    }
    Err(Error::InvalidRecord(
        "NetCDF contains no 1-D wavelength axis matching spectra bands".to_string(),
    ))
}

fn target_columns(
    file: &NcFile,
    sample_count: usize,
    axis_name: &str,
) -> Result<Vec<(String, Vec<f64>)>> {
    let mut targets = Vec::new();
    for variable in file
        .variables()
        .map_err(|error| Error::InvalidRecord(format!("NetCDF metadata error: {error}")))?
    {
        if matches!(
            variable.name(),
            "spectra" | "wavelengths" | "wavelength" | "x"
        ) || variable.name() == axis_name
            || variable.ndim() != 1
            || variable.num_elements() != sample_count as u64
        {
            continue;
        }
        if let Ok(values) = read_f64_vec(file, variable.name()) {
            targets.push((variable.name().to_string(), values));
        }
    }
    Ok(targets)
}

fn base_metadata(file: &NcFile, spectra_var: &NcVariable) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    metadata.insert("container".to_string(), json!("netcdf"));
    if let Ok(attributes) = file.global_attributes() {
        let mut global = BTreeMap::new();
        for attribute in attributes {
            if let Some(value) = attr_value(attribute) {
                global.insert(attribute.name.clone(), value);
            }
        }
        if !global.is_empty() {
            metadata.insert("global_attributes".to_string(), json!(global));
        }
    }
    if let Some(unit) = attr_string(spectra_var, "units") {
        metadata.insert("spectra_units".to_string(), json!(unit));
    }
    metadata
}

fn read_f64_vec(file: &NcFile, name: &str) -> Result<Vec<f64>> {
    let array = file
        .read_variable_as_f64(name)
        .map_err(|error| Error::InvalidRecord(format!("NetCDF read error for {name}: {error}")))?;
    Ok(array.iter().copied().collect())
}

fn attr_string(variable: &NcVariable, name: &str) -> Option<String> {
    variable
        .attribute(name)
        .and_then(|attr| attr.value.as_string())
}

fn attr_value(attribute: &NcAttribute) -> Option<Value> {
    match &attribute.value {
        NcAttrValue::Chars(value) => Some(json!(value)),
        NcAttrValue::Strings(values) => Some(json!(values)),
        other => other.as_f64_vec().map(|values| {
            if values.len() == 1 {
                json!(values[0])
            } else {
                json!(values)
            }
        }),
    }
}
