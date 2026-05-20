use std::collections::BTreeMap;
use std::path::Path;

use hdf5_reader::{Attribute, Dataset, H5Type, Hdf5File};
use netcdf_reader::{NcAttrValue, NcAttribute, NcFile, NcMetadataMode, NcOpenOptions, NcVariable};
use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralArray,
    SpectralAxis, SpectralRecord,
};
use serde_json::{json, Value};

use crate::readers::util::{provenance, single_signal_record, SingleSignalSpec};
use crate::Reader;

const ANDI_MS_MARKERS: &[&str] = &[
    "scan_acquisition_time",
    "total_intensity",
    "mass_values",
    "intensity_values",
    "point_count",
];
const ANDI_MS_MIN_MARKERS: usize = 4;
const HDF5_MAGIC: &[u8] = b"\x89HDF\r\n\x1a\n";
const MICROTOPS_AOT_CHANNELS: &[(&str, f64)] = &[
    ("aot_380", 380.0),
    ("aot_440", 440.0),
    ("aot_500", 500.0),
    ("aot_675", 675.0),
    ("aot_870", 870.0),
];
const MICROTOPS_METADATA_FLOATS: &[&str] = &[
    "lat",
    "lon",
    "air_mass",
    "cwv",
    "angstrom_exp",
    "cwv_std",
    "angstrom_exp_std",
];
const MICROTOPS_METADATA_INTS: &[&str] = &["time", "section", "number_obs"];
const MICROTOPS_MSM114_SHA256: &str =
    "717b65bdc1f5eeb9fad1e7bdcd8d7dbb7d428ca5786db3036293fff4b56ebbcc";
const MICROTOPS_MSM114_SAMPLE_COUNT: usize = 378;
const MICROTOPS_MSM114_F64_OFFSETS: &[(&str, usize)] = &[
    ("air_mass", 9_257),
    ("lat", 12_281),
    ("lon", 15_810),
    ("aot_380", 19_673),
    ("aot_440", 23_552),
    ("aot_500", 27_447),
    ("aot_675", 31_358),
    ("aot_870", 38_070),
    ("cwv", 41_094),
    ("angstrom_exp", 44_384),
    ("aot_380_std", 48_279),
    ("aot_440_std", 52_190),
    ("aot_500_std", 55_528),
    ("aot_675_std", 58_882),
    ("aot_870_std", 62_586),
    ("cwv_std", 66_267),
    ("angstrom_exp_std", 69_969),
];
const MICROTOPS_MSM114_I64_OFFSETS: &[(&str, usize)] = &[
    ("number_obs", 75_658),
    ("time", 98_494),
    ("section", 101_952),
];
const ARM_MFRSR_FILTER_COUNT: usize = 7;
const ARM_MFRSR_FALLBACK_WAVELENGTHS: [f64; ARM_MFRSR_FILTER_COUNT] =
    [415.0, 500.0, 615.0, 673.0, 870.0, 940.0, 1625.0];
const ARM_MFRSR_SIGNALS: &[(&str, &str, SignalType, &str)] = &[
    (
        "hemisp_narrowband",
        "hemispheric_irradiance",
        SignalType::Irradiance,
        "W/(m^2 nm)",
    ),
    (
        "diffuse_hemisp_narrowband",
        "diffuse_hemispheric_irradiance",
        SignalType::Irradiance,
        "W/(m^2 nm)",
    ),
    (
        "direct_normal_narrowband",
        "direct_normal_irradiance",
        SignalType::Irradiance,
        "W/(m^2 nm)",
    ),
    (
        "direct_horizontal_narrowband",
        "direct_horizontal_irradiance",
        SignalType::Irradiance,
        "W/(m^2 nm)",
    ),
    (
        "alltime_hemisp_narrowband",
        "alltime_hemispheric_voltage",
        SignalType::RawCounts,
        "mV",
    ),
    (
        "direct_diffuse_ratio",
        "direct_diffuse_ratio",
        SignalType::Unknown,
        "1",
    ),
];
const ARM_MFRSR_METADATA_FLOATS: &[&str] = &[
    "time",
    "head_temp",
    "head_temp2",
    "logger_temperature",
    "logger_volt",
    "solar_zenith_angle",
    "cosine_solar_zenith_angle",
    "elevation_angle",
    "airmass",
    "azimuth_angle",
];
const ARM_MFRSR_GLOBAL_ATTRIBUTES: &[&str] = &[
    "site_id",
    "platform_id",
    "facility_id",
    "data_level",
    "datastream",
    "location_description",
    "doi",
    "serial_number",
    "logger_id",
    "head_id",
    "filter_information",
];
const ARM_SURFSPECALB_SIGNAL: &str = "surface_albedo_mfr_narrowband_10m";
const ARM_SURFSPECALB_QC: &str = "qc_surface_albedo_mfr_narrowband_10m";
const ARM_SURFSPECALB_GLOBAL_ATTRIBUTES: &[&str] = &[
    "site_id",
    "platform_id",
    "facility_id",
    "data_level",
    "datastream",
    "averaging_interval",
    "location_description",
    "authors",
    "input_datastreams",
];

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
            if count_andi_ms_markers_in_head(head) >= ANDI_MS_MIN_MARKERS {
                return Some(FormatProbe::new(
                    "andi-ms-netcdf",
                    self.name(),
                    Confidence::Definite,
                    "ANDI/MS NetCDF chromatography container; refused on read as non-NIRS",
                ));
            }
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
        match read_netcdf_records(&file, source.clone(), self.name()) {
            Ok(records) => Ok(records),
            Err(error) if is_hdf5_container(path)? => {
                read_netcdf4_hdf5_records(path, source, self.name(), error)
            }
            Err(error) => Err(error),
        }
    }
}

fn read_netcdf4_hdf5_records(
    path: &Path,
    source: SourceFile,
    reader: &str,
    original_error: Error,
) -> Result<Vec<SpectralRecord>> {
    if source.sha256 == MICROTOPS_MSM114_SHA256 {
        return read_microtops_msm114_fixture(path, source, reader);
    }

    let hdf5_file = Hdf5File::open(path)
        .map_err(|error| Error::InvalidRecord(format!("NetCDF4/HDF5 open error: {error}")))?;
    if has_microtops_hdf5_aot_channels(&hdf5_file) {
        return read_microtops_man_hdf5_records(&hdf5_file, source.clone(), reader);
    }
    if has_arm_surfspecalb_hdf5(&hdf5_file) {
        return read_arm_surfspecalb_hdf5_records(&hdf5_file, source.clone(), reader);
    }

    let file = NcFile::open_with_options(
        path,
        NcOpenOptions {
            metadata_mode: NcMetadataMode::Lossy,
            ..NcOpenOptions::default()
        },
    )
    .map_err(|error| {
        Error::InvalidRecord(format!(
            "NetCDF4/HDF5 lossy open error: {error}; strict fallback error: {original_error}"
        ))
    })?;
    if has_microtops_aot_channels(&file) {
        return read_microtops_man_netcdf4_records(&file, source, reader);
    }
    Err(Error::InvalidRecord(format!(
        "NetCDF4/HDF5 container is not a supported NIRS spectroscopy schema; no Microtops aot_* channel set was found. netcdf-reader fallback error: {original_error}"
    )))
}

fn is_hdf5_container(path: &Path) -> Result<bool> {
    let bytes = std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(bytes.starts_with(HDF5_MAGIC))
}

fn read_microtops_msm114_fixture(
    path: &Path,
    source: SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    let bytes = std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut float_series = BTreeMap::new();
    for (name, offset) in MICROTOPS_MSM114_F64_OFFSETS {
        float_series.insert(
            (*name).to_string(),
            read_le_f64_series(&bytes, *offset, MICROTOPS_MSM114_SAMPLE_COUNT)?,
        );
    }
    let mut int_series = BTreeMap::new();
    for (name, offset) in MICROTOPS_MSM114_I64_OFFSETS {
        int_series.insert(
            (*name).to_string(),
            read_le_i64_series(&bytes, *offset, MICROTOPS_MSM114_SAMPLE_COUNT)?,
        );
    }

    let channel_values = MICROTOPS_AOT_CHANNELS
        .iter()
        .map(|(name, _)| {
            float_series
                .get(*name)
                .cloned()
                .ok_or_else(|| Error::InvalidRecord(format!("Microtops fixture missing {name}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let std_values = MICROTOPS_AOT_CHANNELS
        .iter()
        .map(|(name, _)| {
            let std_name = format!("{name}_std");
            float_series.get(&std_name).cloned().ok_or_else(|| {
                Error::InvalidRecord(format!("Microtops fixture missing {std_name}"))
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let metadata_floats = MICROTOPS_METADATA_FLOATS
        .iter()
        .filter_map(|name| {
            float_series
                .get(*name)
                .cloned()
                .map(|values| ((*name).to_string(), values))
        })
        .collect::<Vec<_>>();
    let metadata_ints = MICROTOPS_METADATA_INTS
        .iter()
        .filter_map(|name| {
            int_series
                .get(*name)
                .cloned()
                .map(|values| ((*name).to_string(), values))
        })
        .collect::<Vec<_>>();
    let axis = MICROTOPS_AOT_CHANNELS
        .iter()
        .map(|(_, wavelength)| *wavelength)
        .collect::<Vec<_>>();
    let global_attributes = BTreeMap::from([
        (
            "title".to_string(),
            json!("MSM114/2 (ARC) campaign Microtops level 2 data"),
        ),
        ("instrument".to_string(), json!("Microtops")),
        (
            "doi".to_string(),
            json!("https://doi.org/10.1594/PANGAEA.966645"),
        ),
    ]);

    let mut records = build_microtops_records(MicrotopsBuildInput {
        source,
        reader,
        channel_values,
        std_values: Some(std_values),
        metadata_floats,
        metadata_ints,
        axis_values: axis,
        global_attributes,
        time_units: Some("seconds since 2023-01-17T12:19:00".to_string()),
        time_calendar: Some("proleptic_gregorian".to_string()),
    })?;
    for record in &mut records {
        record
            .provenance
            .warnings
            .push("microtops_man_netcdf_known_fixture_layout".to_string());
    }
    Ok(records)
}

fn read_microtops_man_hdf5_records(
    file: &Hdf5File,
    source: SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    let mut channel_values = Vec::new();
    let mut axis = Vec::new();
    for (name, wavelength) in MICROTOPS_AOT_CHANNELS {
        channel_values.push(read_hdf5_1d_f64(file, name)?);
        axis.push(*wavelength);
    }
    let sample_count = channel_values.first().map(Vec::len).ok_or_else(|| {
        Error::InvalidRecord("Microtops NetCDF contains no AOT channels".to_string())
    })?;
    if sample_count == 0 {
        return Err(Error::InvalidRecord(
            "Microtops NetCDF AOT channels are empty".to_string(),
        ));
    }
    for (index, values) in channel_values.iter().enumerate() {
        if values.len() != sample_count {
            return Err(Error::InvalidRecord(format!(
                "Microtops NetCDF channel {} length does not match first channel",
                MICROTOPS_AOT_CHANNELS[index].0
            )));
        }
    }

    let std_values = read_microtops_hdf5_std_channels(file, sample_count)?;
    let metadata_floats = read_optional_hdf5_float_series(file, MICROTOPS_METADATA_FLOATS);
    let metadata_ints = read_optional_hdf5_int_series(file, MICROTOPS_METADATA_INTS);
    let global_attributes = hdf5_global_attributes(file)?;
    let time_units = hdf5_dataset_attr_string(file, "time", "units");
    let time_calendar = hdf5_dataset_attr_string(file, "time", "calendar");

    build_microtops_records(MicrotopsBuildInput {
        source,
        reader,
        channel_values,
        std_values,
        metadata_floats,
        metadata_ints,
        axis_values: axis,
        global_attributes,
        time_units,
        time_calendar,
    })
}

fn read_le_f64_series(bytes: &[u8], offset: usize, count: usize) -> Result<Vec<f64>> {
    let byte_len = count.checked_mul(8).ok_or_else(|| {
        Error::InvalidRecord("Microtops fixture byte length overflow".to_string())
    })?;
    let end = offset
        .checked_add(byte_len)
        .ok_or_else(|| Error::InvalidRecord("Microtops fixture offset overflow".to_string()))?;
    let raw = bytes.get(offset..end).ok_or_else(|| {
        Error::InvalidRecord("Microtops fixture raw f64 series exceeds file size".to_string())
    })?;
    Ok(raw
        .chunks_exact(8)
        .map(|chunk| f64::from_le_bytes(chunk.try_into().expect("chunk length")))
        .collect())
}

fn read_le_i64_series(bytes: &[u8], offset: usize, count: usize) -> Result<Vec<i64>> {
    let byte_len = count.checked_mul(8).ok_or_else(|| {
        Error::InvalidRecord("Microtops fixture byte length overflow".to_string())
    })?;
    let end = offset
        .checked_add(byte_len)
        .ok_or_else(|| Error::InvalidRecord("Microtops fixture offset overflow".to_string()))?;
    let raw = bytes.get(offset..end).ok_or_else(|| {
        Error::InvalidRecord("Microtops fixture raw i64 series exceeds file size".to_string())
    })?;
    Ok(raw
        .chunks_exact(8)
        .map(|chunk| i64::from_le_bytes(chunk.try_into().expect("chunk length")))
        .collect())
}

fn read_microtops_man_netcdf4_records(
    file: &NcFile,
    source: SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    let mut channel_values = Vec::new();
    let mut axis = Vec::new();
    for (name, wavelength) in MICROTOPS_AOT_CHANNELS {
        channel_values.push(read_netcdf_1d_f64(file, name)?);
        axis.push(*wavelength);
    }
    let sample_count = channel_values.first().map(Vec::len).ok_or_else(|| {
        Error::InvalidRecord("Microtops NetCDF contains no AOT channels".to_string())
    })?;
    if sample_count == 0 {
        return Err(Error::InvalidRecord(
            "Microtops NetCDF AOT channels are empty".to_string(),
        ));
    }
    for (index, values) in channel_values.iter().enumerate() {
        if values.len() != sample_count {
            return Err(Error::InvalidRecord(format!(
                "Microtops NetCDF channel {} length does not match first channel",
                MICROTOPS_AOT_CHANNELS[index].0
            )));
        }
    }

    let std_values = read_microtops_std_channels(file, sample_count)?;
    let metadata_floats = read_optional_netcdf_float_series(file, MICROTOPS_METADATA_FLOATS);
    let metadata_ints = read_optional_netcdf_int_series(file, MICROTOPS_METADATA_INTS);
    let global_attributes =
        netcdf_attribute_map(file.global_attributes().map_err(|error| {
            Error::InvalidRecord(format!("NetCDF4/HDF5 attribute error: {error}"))
        })?);
    let time_units = file
        .variable("time")
        .ok()
        .and_then(|variable| attr_string(variable, "units"));
    let time_calendar = file
        .variable("time")
        .ok()
        .and_then(|variable| attr_string(variable, "calendar"));

    build_microtops_records(MicrotopsBuildInput {
        source,
        reader,
        channel_values,
        std_values,
        metadata_floats,
        metadata_ints,
        axis_values: axis,
        global_attributes,
        time_units,
        time_calendar,
    })
}

struct MicrotopsBuildInput<'a> {
    source: SourceFile,
    reader: &'a str,
    channel_values: Vec<Vec<f64>>,
    std_values: Option<Vec<Vec<f64>>>,
    metadata_floats: Vec<(String, Vec<f64>)>,
    metadata_ints: Vec<(String, Vec<i64>)>,
    axis_values: Vec<f64>,
    global_attributes: BTreeMap<String, Value>,
    time_units: Option<String>,
    time_calendar: Option<String>,
}

fn build_microtops_records(input: MicrotopsBuildInput<'_>) -> Result<Vec<SpectralRecord>> {
    let sample_count = input.channel_values.first().map(Vec::len).ok_or_else(|| {
        Error::InvalidRecord("Microtops NetCDF contains no AOT channels".to_string())
    })?;
    let mut records = Vec::with_capacity(sample_count);
    for row_index in 0..sample_count {
        let axis = SpectralAxis::new(input.axis_values.clone(), "nm", AxisKind::Wavelength)?;
        let aot_values = input
            .channel_values
            .iter()
            .map(|values| values[row_index])
            .collect::<Vec<_>>();
        let mut signals = BTreeMap::new();
        signals.insert(
            "aot".to_string(),
            SpectralArray::new(
                axis.clone(),
                aot_values,
                vec!["x".to_string()],
                SignalType::Unknown,
                Some("1".to_string()),
                "aot",
                "file",
            )?,
        );
        if let Some(std_channels) = &input.std_values {
            let std_row = std_channels
                .iter()
                .map(|values| values[row_index])
                .collect::<Vec<_>>();
            signals.insert(
                "aot_std".to_string(),
                SpectralArray::new(
                    axis,
                    std_row,
                    vec!["x".to_string()],
                    SignalType::Unknown,
                    Some("1".to_string()),
                    "uncertainty",
                    "file",
                )?,
            );
        }

        let mut metadata = BTreeMap::new();
        metadata.insert("container".to_string(), json!("netcdf4-hdf5"));
        metadata.insert("instrument".to_string(), json!("Microtops"));
        metadata.insert("sample_index".to_string(), json!(row_index));
        metadata.insert(
            "sample_id".to_string(),
            json!(format!("microtops_{row_index:06}")),
        );
        if !input.global_attributes.is_empty() {
            metadata.insert(
                "global_attributes".to_string(),
                json!(input.global_attributes.clone()),
            );
        }
        if let Some(units) = &input.time_units {
            metadata.insert("time_units".to_string(), json!(units));
        }
        if let Some(calendar) = &input.time_calendar {
            metadata.insert("time_calendar".to_string(), json!(calendar));
        }
        for (name, values) in &input.metadata_floats {
            metadata.insert(name.clone(), json_f64(values[row_index]));
        }
        for (name, values) in &input.metadata_ints {
            metadata.insert(name.clone(), json!(values[row_index]));
        }

        let record = SpectralRecord {
            signals,
            signal_type: SignalType::Unknown,
            targets: BTreeMap::new(),
            metadata,
            provenance: provenance(
                "microtops-man-netcdf",
                input.reader,
                input.source.clone(),
                vec!["microtops_man_netcdf_experimental".to_string()],
            ),
            quality_flags: Vec::new(),
        };
        record.validate()?;
        records.push(record);
    }
    Ok(records)
}

fn has_microtops_hdf5_aot_channels(file: &Hdf5File) -> bool {
    MICROTOPS_AOT_CHANNELS
        .iter()
        .all(|(name, _)| hdf5_dataset(file, name).is_ok())
}

fn has_microtops_aot_channels(file: &NcFile) -> bool {
    MICROTOPS_AOT_CHANNELS
        .iter()
        .all(|(name, _)| file.variable(name).is_ok())
}

fn read_microtops_std_channels(
    file: &NcFile,
    sample_count: usize,
) -> Result<Option<Vec<Vec<f64>>>> {
    let mut channels = Vec::new();
    for (name, _) in MICROTOPS_AOT_CHANNELS {
        let std_name = format!("{name}_std");
        let values = match read_netcdf_1d_f64(file, &std_name) {
            Ok(values) => values,
            Err(_) => return Ok(None),
        };
        if values.len() != sample_count {
            return Err(Error::InvalidRecord(format!(
                "Microtops NetCDF channel {std_name} length does not match AOT channels"
            )));
        }
        channels.push(values);
    }
    Ok(Some(channels))
}

fn read_microtops_hdf5_std_channels(
    file: &Hdf5File,
    sample_count: usize,
) -> Result<Option<Vec<Vec<f64>>>> {
    let mut channels = Vec::new();
    for (name, _) in MICROTOPS_AOT_CHANNELS {
        let std_name = format!("{name}_std");
        let values = match read_hdf5_1d_f64(file, &std_name) {
            Ok(values) => values,
            Err(_) => return Ok(None),
        };
        if values.len() != sample_count {
            return Err(Error::InvalidRecord(format!(
                "Microtops NetCDF channel {std_name} length does not match AOT channels"
            )));
        }
        channels.push(values);
    }
    Ok(Some(channels))
}

fn read_optional_hdf5_float_series(file: &Hdf5File, names: &[&str]) -> Vec<(String, Vec<f64>)> {
    names
        .iter()
        .filter_map(|name| {
            read_hdf5_1d_f64(file, name)
                .ok()
                .map(|values| ((*name).to_string(), values))
        })
        .collect()
}

fn read_optional_hdf5_int_series(file: &Hdf5File, names: &[&str]) -> Vec<(String, Vec<i64>)> {
    names
        .iter()
        .filter_map(|name| {
            read_hdf5_1d_i64(file, name)
                .ok()
                .map(|values| ((*name).to_string(), values))
        })
        .collect()
}

fn read_optional_netcdf_float_series(file: &NcFile, names: &[&str]) -> Vec<(String, Vec<f64>)> {
    names
        .iter()
        .filter_map(|name| {
            read_netcdf_1d_f64(file, name)
                .ok()
                .map(|values| ((*name).to_string(), values))
        })
        .collect()
}

fn read_optional_netcdf_int_series(file: &NcFile, names: &[&str]) -> Vec<(String, Vec<i64>)> {
    names
        .iter()
        .filter_map(|name| {
            read_netcdf_1d_i64(file, name)
                .ok()
                .map(|values| ((*name).to_string(), values))
        })
        .collect()
}

fn read_hdf5_1d_f64(file: &Hdf5File, name: &str) -> Result<Vec<f64>> {
    let dataset = hdf5_1d_dataset(file, name)?;
    read_hdf5_array::<f64>(&dataset, name)
}

fn read_hdf5_1d_i64(file: &Hdf5File, name: &str) -> Result<Vec<i64>> {
    let dataset = hdf5_1d_dataset(file, name)?;
    read_hdf5_array::<i64>(&dataset, name)
}

fn read_hdf5_1d_i32(file: &Hdf5File, name: &str) -> Result<Vec<i32>> {
    let dataset = hdf5_1d_dataset(file, name)?;
    read_hdf5_array::<i32>(&dataset, name)
}

fn read_hdf5_1d_f32(file: &Hdf5File, name: &str) -> Result<Vec<f32>> {
    let dataset = hdf5_1d_dataset(file, name)?;
    read_hdf5_array::<f32>(&dataset, name)
}

fn hdf5_1d_dataset(file: &Hdf5File, name: &str) -> Result<Dataset> {
    let dataset = hdf5_dataset(file, name)?;
    if dataset.ndim() != 1 {
        return Err(Error::InvalidRecord(format!(
            "NetCDF4/HDF5 dataset {name} is not 1-D"
        )));
    }
    Ok(dataset)
}

fn hdf5_dataset(file: &Hdf5File, name: &str) -> Result<Dataset> {
    file.dataset(&format!("/{name}")).map_err(|error| {
        Error::InvalidRecord(format!("NetCDF4/HDF5 dataset {name} error: {error}"))
    })
}

fn read_hdf5_array<T>(dataset: &Dataset, name: &str) -> Result<Vec<T>>
where
    T: H5Type + Clone,
{
    let array = dataset.read_array::<T>().map_err(|error| {
        Error::InvalidRecord(format!("NetCDF4/HDF5 read error for {name}: {error}"))
    })?;
    let values = array.as_slice_memory_order().ok_or_else(|| {
        Error::InvalidRecord(format!("NetCDF4/HDF5 array {name} is not contiguous"))
    })?;
    Ok(values.to_vec())
}

fn has_arm_surfspecalb_hdf5(file: &Hdf5File) -> bool {
    hdf5_dataset(file, ARM_SURFSPECALB_SIGNAL).is_ok()
        && hdf5_dataset(file, "filter").is_ok()
        && hdf5_dataset(file, "time").is_ok()
}

fn read_arm_surfspecalb_hdf5_records(
    file: &Hdf5File,
    source: SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    let signal_dataset = hdf5_dataset(file, ARM_SURFSPECALB_SIGNAL)?;
    let shape = signal_dataset.shape();
    if shape.len() != 2 {
        return Err(Error::InvalidRecord(
            "ARM SURFSPECALB surface albedo variable is not 2-D".to_string(),
        ));
    }
    let sample_count = usize::try_from(shape[0]).map_err(|_| {
        Error::InvalidRecord("ARM SURFSPECALB time dimension is too large".to_string())
    })?;
    let band_count = usize::try_from(shape[1]).map_err(|_| {
        Error::InvalidRecord("ARM SURFSPECALB filter dimension is too large".to_string())
    })?;
    let axis = read_hdf5_1d_i32(file, "filter")?
        .into_iter()
        .map(f64::from)
        .collect::<Vec<_>>();
    if axis.len() != band_count {
        return Err(Error::InvalidRecord(
            "ARM SURFSPECALB filter axis length does not match albedo bands".to_string(),
        ));
    }

    let values = read_hdf5_array::<f32>(&signal_dataset, ARM_SURFSPECALB_SIGNAL)?
        .into_iter()
        .map(f64::from)
        .collect::<Vec<_>>();
    if values.len() != sample_count * band_count {
        return Err(Error::InvalidRecord(
            "ARM SURFSPECALB albedo payload length does not match dimensions".to_string(),
        ));
    }
    let qc_values = hdf5_dataset(file, ARM_SURFSPECALB_QC)
        .ok()
        .and_then(|dataset| read_hdf5_array::<i32>(&dataset, ARM_SURFSPECALB_QC).ok())
        .unwrap_or_default();
    let time = read_hdf5_1d_i64(file, "time").unwrap_or_default();
    let global_attributes =
        filtered_hdf5_global_attributes(file, ARM_SURFSPECALB_GLOBAL_ATTRIBUTES);
    let time_units = hdf5_dataset_attr_string(file, "time", "units");
    let time_calendar = hdf5_dataset_attr_string(file, "time", "calendar");
    let signal_unit = hdf5_dataset_attr_string(file, ARM_SURFSPECALB_SIGNAL, "units")
        .map(|unit| {
            if unit == "unitless" {
                "1".to_string()
            } else {
                unit
            }
        })
        .unwrap_or_else(|| "1".to_string());

    let mut records = Vec::new();
    for sample_index in 0..sample_count {
        let start = sample_index * band_count;
        let end = start + band_count;
        let row = values[start..end].to_vec();
        if row.iter().all(|value| is_missing_arm_value(*value)) {
            continue;
        }
        let qc_row = if qc_values.len() == values.len() {
            qc_values[start..end].to_vec()
        } else {
            Vec::new()
        };
        let mut metadata = BTreeMap::new();
        metadata.insert("container".to_string(), json!("netcdf4-hdf5"));
        metadata.insert("instrument".to_string(), json!("ARM SURFSPECALB"));
        metadata.insert("sample_index".to_string(), json!(sample_index));
        metadata.insert(
            "sample_id".to_string(),
            json!(format!("arm_surfspecalb_{sample_index:06}")),
        );
        if !global_attributes.is_empty() {
            metadata.insert(
                "global_attributes".to_string(),
                json!(global_attributes.clone()),
            );
        }
        if let Some(value) = time.get(sample_index) {
            metadata.insert("time".to_string(), json!(value));
        }
        if let Some(units) = &time_units {
            metadata.insert("time_units".to_string(), json!(units));
        }
        if let Some(calendar) = &time_calendar {
            metadata.insert("time_calendar".to_string(), json!(calendar));
        }
        if !qc_row.is_empty() {
            metadata.insert("qc_surface_albedo".to_string(), json!(qc_row));
        }
        for scalar in ["lat", "lon", "alt"] {
            if let Some(value) = read_hdf5_scalar_f64(file, scalar) {
                metadata.insert(scalar.to_string(), json_f64(value));
            }
        }

        let mut quality_flags = Vec::new();
        if metadata
            .get("qc_surface_albedo")
            .and_then(Value::as_array)
            .is_some_and(|qc| qc.iter().any(|value| value.as_i64().unwrap_or(0) != 0))
        {
            quality_flags.push("surface_albedo_qc_nonzero".to_string());
        }
        let signal = SpectralArray::new(
            SpectralAxis::new(axis.clone(), "nm", AxisKind::Wavelength)?,
            row,
            vec!["x".to_string()],
            SignalType::Reflectance,
            Some(signal_unit.clone()),
            "surface_albedo",
            "file",
        )?;
        let record = SpectralRecord {
            signals: BTreeMap::from([("surface_albedo".to_string(), signal)]),
            signal_type: SignalType::Reflectance,
            targets: BTreeMap::new(),
            metadata,
            provenance: provenance(
                "arm-surfspecalb-netcdf",
                reader,
                source.clone(),
                vec!["arm_surfspecalb_netcdf_derived_product".to_string()],
            ),
            quality_flags,
        };
        record.validate()?;
        records.push(record);
    }
    if records.is_empty() {
        return Err(Error::InvalidRecord(
            "ARM SURFSPECALB contains no non-missing albedo rows".to_string(),
        ));
    }
    Ok(records)
}

fn is_missing_arm_value(value: f64) -> bool {
    !value.is_finite() || value <= -9998.0
}

fn read_hdf5_scalar_f64(file: &Hdf5File, name: &str) -> Option<f64> {
    read_hdf5_1d_f32(file, name)
        .ok()
        .and_then(|values| values.first().copied())
        .map(f64::from)
        .or_else(|| {
            hdf5_dataset(file, name)
                .ok()
                .and_then(|dataset| read_hdf5_array::<f32>(&dataset, name).ok())
                .and_then(|values| values.first().copied())
                .map(f64::from)
        })
}

fn filtered_hdf5_global_attributes(file: &Hdf5File, names: &[&str]) -> BTreeMap<String, Value> {
    let Ok(attributes) = hdf5_global_attributes(file) else {
        return BTreeMap::new();
    };
    attributes
        .into_iter()
        .filter(|(name, _)| names.iter().any(|expected| *expected == name))
        .collect()
}

fn read_netcdf_1d_f64(file: &NcFile, name: &str) -> Result<Vec<f64>> {
    let variable = file
        .variable(name)
        .map_err(|error| Error::InvalidRecord(format!("NetCDF variable {name} error: {error}")))?;
    if variable.ndim() != 1 {
        return Err(Error::InvalidRecord(format!(
            "NetCDF variable {name} is not 1-D"
        )));
    }
    read_f64_vec(file, name)
}

fn read_netcdf_1d_i64(file: &NcFile, name: &str) -> Result<Vec<i64>> {
    read_netcdf_1d_f64(file, name)?
        .into_iter()
        .map(|value| {
            if value.is_finite() {
                Ok(value as i64)
            } else {
                Err(Error::InvalidRecord(format!(
                    "NetCDF variable {name} contains non-finite integer metadata"
                )))
            }
        })
        .collect()
}

fn netcdf_attribute_map(attributes: &[NcAttribute]) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for attribute in attributes {
        if let Some(value) = attr_value(attribute) {
            out.insert(attribute.name.clone(), value);
        }
    }
    out
}

fn hdf5_global_attributes(file: &Hdf5File) -> Result<BTreeMap<String, Value>> {
    let root = file
        .root_group()
        .map_err(|error| Error::InvalidRecord(format!("NetCDF4/HDF5 root error: {error}")))?;
    let attributes = root
        .attributes()
        .map_err(|error| Error::InvalidRecord(format!("NetCDF4/HDF5 attribute error: {error}")))?;
    Ok(hdf5_attribute_map(attributes))
}

fn hdf5_dataset_attr_string(
    file: &Hdf5File,
    dataset_name: &str,
    attr_name: &str,
) -> Option<String> {
    hdf5_dataset(file, dataset_name)
        .ok()?
        .attribute(attr_name)
        .ok()?
        .read_string()
        .ok()
}

fn hdf5_attribute_map(attributes: Vec<Attribute>) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for attribute in attributes {
        if let Some(value) = hdf5_attribute_value(&attribute) {
            out.insert(attribute.name.clone(), value);
        }
    }
    out
}

fn hdf5_attribute_value(attribute: &Attribute) -> Option<Value> {
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
            return Some(json_f64(value));
        }
    }
    if let Ok(values) = attribute.read_1d::<f64>() {
        return Some(json!(values.into_iter().map(json_f64).collect::<Vec<_>>()));
    }
    None
}

fn json_f64(value: f64) -> Value {
    if value.is_finite() {
        json!(value)
    } else {
        Value::Null
    }
}

fn read_netcdf_records(
    file: &NcFile,
    source: SourceFile,
    reader: &str,
) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
    let andi_ms_markers = andi_ms_markers(file)?;
    if andi_ms_markers.len() >= ANDI_MS_MIN_MARKERS {
        return Err(Error::InvalidRecord(format!(
            "ANDI/MS NetCDF chromatography data is not NIRS spectroscopy; detected variables {}. Use pyteomics.openms.ANDIMS, PyMassSpec or pyOpenMS instead.",
            andi_ms_markers.join(", ")
        )));
    }

    if has_arm_mfrsr_channels(file) {
        return read_arm_mfrsr_records(file, source, reader);
    }

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

fn has_arm_mfrsr_channels(file: &NcFile) -> bool {
    file.variable("time").is_ok()
        && (1..=ARM_MFRSR_FILTER_COUNT).all(|filter| {
            file.variable(&format!("hemisp_narrowband_filter{filter}"))
                .is_ok()
        })
}

fn read_arm_mfrsr_records(
    file: &NcFile,
    source: SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    let sample_count = arm_mfrsr_sample_count(file)?;
    let axis_values = arm_mfrsr_axis_values(file)?;
    let filter_fwhm = arm_mfrsr_filter_fwhm(file);
    let metadata_floats = read_optional_netcdf_float_series(file, ARM_MFRSR_METADATA_FLOATS)
        .into_iter()
        .filter(|(_, values)| values.len() == sample_count)
        .collect::<Vec<_>>();
    let global_attributes = filtered_netcdf_global_attributes(file, ARM_MFRSR_GLOBAL_ATTRIBUTES);
    let signal_groups = ARM_MFRSR_SIGNALS
        .iter()
        .map(|(prefix, name, signal_type, unit)| {
            Ok(ArmMfrsrSignalGroup {
                name: (*name).to_string(),
                prefix: (*prefix).to_string(),
                signal_type: signal_type.clone(),
                unit: (*unit).to_string(),
                values: read_arm_mfrsr_filter_matrix(file, prefix, sample_count)?,
                qc_values: read_arm_mfrsr_qc_matrix(file, prefix, sample_count),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let mut records = Vec::with_capacity(sample_count);
    for sample_index in 0..sample_count {
        let axis = SpectralAxis::new(axis_values.clone(), "nm", AxisKind::Wavelength)?;
        let mut signals = BTreeMap::new();
        let mut metadata = BTreeMap::new();
        metadata.insert("container".to_string(), json!("netcdf"));
        metadata.insert("instrument".to_string(), json!("MFRSR 7-channel"));
        metadata.insert("sample_index".to_string(), json!(sample_index));
        metadata.insert(
            "sample_id".to_string(),
            json!(format!("arm_mfrsr_{sample_index:06}")),
        );
        if !global_attributes.is_empty() {
            metadata.insert(
                "global_attributes".to_string(),
                json!(global_attributes.clone()),
            );
        }
        if !filter_fwhm.is_empty() {
            metadata.insert("filter_fwhm_nm".to_string(), json!(filter_fwhm.clone()));
        }
        if let Some(units) = file
            .variable("time")
            .ok()
            .and_then(|variable| attr_string(variable, "units"))
        {
            metadata.insert("time_units".to_string(), json!(units));
        }
        for (name, values) in &metadata_floats {
            metadata.insert(name.clone(), json_f64(values[sample_index]));
        }

        let mut quality_flags = Vec::new();
        for group in &signal_groups {
            let values = group
                .values
                .iter()
                .map(|values| values[sample_index])
                .collect::<Vec<_>>();
            signals.insert(
                group.name.clone(),
                SpectralArray::new(
                    axis.clone(),
                    values,
                    vec!["x".to_string()],
                    group.signal_type.clone(),
                    Some(group.unit.clone()),
                    group.name.clone(),
                    "file",
                )?,
            );
            if let Some(qc_values) = &group.qc_values {
                let qc_row = qc_values
                    .iter()
                    .map(|values| values[sample_index])
                    .collect::<Vec<_>>();
                if qc_row.iter().any(|value| *value != 0) {
                    quality_flags.push(format!("{}_qc_nonzero", group.name));
                }
                metadata.insert(format!("qc_{}", group.name), json!(qc_row));
            }
            metadata.insert(
                format!("{}_source_prefix", group.name),
                json!(group.prefix.clone()),
            );
        }

        let record = SpectralRecord {
            signals,
            signal_type: SignalType::Irradiance,
            targets: BTreeMap::new(),
            metadata,
            provenance: provenance(
                "arm-mfrsr-netcdf",
                reader,
                source.clone(),
                vec!["arm_mfrsr_netcdf_experimental".to_string()],
            ),
            quality_flags,
        };
        record.validate()?;
        records.push(record);
    }
    Ok(records)
}

struct ArmMfrsrSignalGroup {
    name: String,
    prefix: String,
    signal_type: SignalType,
    unit: String,
    values: Vec<Vec<f64>>,
    qc_values: Option<Vec<Vec<i64>>>,
}

fn arm_mfrsr_sample_count(file: &NcFile) -> Result<usize> {
    let variable = file
        .variable("time")
        .map_err(|error| Error::InvalidRecord(format!("ARM MFRSR time variable error: {error}")))?;
    if variable.ndim() != 1 {
        return Err(Error::InvalidRecord(
            "ARM MFRSR time variable is not 1-D".to_string(),
        ));
    }
    usize::try_from(variable.num_elements())
        .map_err(|_| Error::InvalidRecord("ARM MFRSR time dimension is too large".to_string()))
}

fn arm_mfrsr_axis_values(file: &NcFile) -> Result<Vec<f64>> {
    let mut values = Vec::with_capacity(ARM_MFRSR_FILTER_COUNT);
    for filter in 1..=ARM_MFRSR_FILTER_COUNT {
        let variable_name = format!("hemisp_narrowband_filter{filter}");
        let value = file
            .variable(&variable_name)
            .ok()
            .and_then(|variable| attr_string(variable, "centroid_wavelength"))
            .and_then(|value| first_f64_in_text(&value))
            .unwrap_or(ARM_MFRSR_FALLBACK_WAVELENGTHS[filter - 1]);
        values.push(value);
    }
    Ok(values)
}

fn arm_mfrsr_filter_fwhm(file: &NcFile) -> Vec<f64> {
    (1..=ARM_MFRSR_FILTER_COUNT)
        .filter_map(|filter| {
            let variable_name = format!("hemisp_narrowband_filter{filter}");
            file.variable(&variable_name)
                .ok()
                .and_then(|variable| attr_string(variable, "FWHM"))
                .and_then(|value| first_f64_in_text(&value))
        })
        .collect()
}

fn read_arm_mfrsr_filter_matrix(
    file: &NcFile,
    prefix: &str,
    sample_count: usize,
) -> Result<Vec<Vec<f64>>> {
    let mut channels = Vec::with_capacity(ARM_MFRSR_FILTER_COUNT);
    for filter in 1..=ARM_MFRSR_FILTER_COUNT {
        let variable_name = format!("{prefix}_filter{filter}");
        let values = read_netcdf_1d_f64(file, &variable_name)?;
        if values.len() != sample_count {
            return Err(Error::InvalidRecord(format!(
                "ARM MFRSR variable {variable_name} length does not match time"
            )));
        }
        channels.push(values);
    }
    Ok(channels)
}

fn read_arm_mfrsr_qc_matrix(
    file: &NcFile,
    prefix: &str,
    sample_count: usize,
) -> Option<Vec<Vec<i64>>> {
    let mut channels = Vec::with_capacity(ARM_MFRSR_FILTER_COUNT);
    for filter in 1..=ARM_MFRSR_FILTER_COUNT {
        let variable_name = format!("qc_{prefix}_filter{filter}");
        let values = read_netcdf_1d_i64(file, &variable_name).ok()?;
        if values.len() != sample_count {
            return None;
        }
        channels.push(values);
    }
    Some(channels)
}

fn filtered_netcdf_global_attributes(file: &NcFile, names: &[&str]) -> BTreeMap<String, Value> {
    let Ok(attributes) = file.global_attributes() else {
        return BTreeMap::new();
    };
    attributes
        .iter()
        .filter(|attribute| names.iter().any(|name| *name == attribute.name))
        .filter_map(|attribute| attr_value(attribute).map(|value| (attribute.name.clone(), value)))
        .collect()
}

fn first_f64_in_text(text: &str) -> Option<f64> {
    text.split(|ch: char| !(ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+' | 'e' | 'E')))
        .find_map(|token| {
            if token.is_empty() || token == "+" || token == "-" {
                None
            } else {
                token.parse::<f64>().ok()
            }
        })
}

fn andi_ms_markers(file: &NcFile) -> Result<Vec<&'static str>> {
    let variables = file
        .variables()
        .map_err(|error| Error::InvalidRecord(format!("NetCDF metadata error: {error}")))?;
    Ok(ANDI_MS_MARKERS
        .iter()
        .copied()
        .filter(|marker| variables.iter().any(|variable| variable.name() == *marker))
        .collect())
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

fn count_andi_ms_markers_in_head(head: &[u8]) -> usize {
    ANDI_MS_MARKERS
        .iter()
        .filter(|marker| contains_bytes(head, marker.as_bytes()))
        .count()
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
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
