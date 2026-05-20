use std::collections::BTreeMap;
use std::path::Path;

use hdf5_reader::error::ByteOrder;
use hdf5_reader::messages::datatype::Datatype as HdfDatatype;
use hdf5_reader::messages::layout::DataLayout;
use hdf5_reader::messages::HdfMessage;
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
const MICROTOPS_GLOBAL_STRING_ATTRS: &[&str] = &[
    "title",
    "source",
    "instrument",
    "platform",
    "doi",
    "conventions",
    "version",
    "comment",
    "_NCProperties",
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

#[derive(Clone, Debug)]
struct MicrotopsAotChannel {
    name: String,
    wavelength_nm: f64,
}

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
    let hdf5_file = Hdf5File::open(path)
        .map_err(|error| Error::InvalidRecord(format!("NetCDF4/HDF5 open error: {error}")))?;
    let hdf5_microtops_channels = discover_microtops_hdf5_aot_channels(&hdf5_file, path);
    let mut microtops_hdf5_error = None;
    if !hdf5_microtops_channels.is_empty() {
        match read_microtops_man_hdf5_records(
            &hdf5_file,
            &hdf5_microtops_channels,
            source.clone(),
            reader,
        ) {
            Ok(records) => return Ok(records),
            Err(error) => microtops_hdf5_error = Some(error),
        }
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
    let netcdf_microtops_channels = discover_microtops_netcdf_aot_channels(&file);
    let mut microtops_netcdf_error = None;
    if !netcdf_microtops_channels.is_empty() {
        match read_microtops_man_netcdf4_records(
            &file,
            &netcdf_microtops_channels,
            source.clone(),
            reader,
        ) {
            Ok(records) => return Ok(records),
            Err(error) => microtops_netcdf_error = Some(error),
        }
    }
    let mut detail = format!(
        "NetCDF4/HDF5 container is not a supported NIRS spectroscopy schema; no Microtops aot_* channel set was found. netcdf-reader fallback error: {original_error}"
    );
    if let Some(error) = microtops_hdf5_error {
        detail.push_str(&format!("; HDF5 Microtops read error: {error}"));
    }
    if let Some(error) = microtops_netcdf_error {
        detail.push_str(&format!("; NetCDF Microtops read error: {error}"));
    }
    Err(Error::InvalidRecord(detail))
}

fn is_hdf5_container(path: &Path) -> Result<bool> {
    let bytes = std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(bytes.starts_with(HDF5_MAGIC))
}

fn read_microtops_man_hdf5_records(
    file: &Hdf5File,
    channels: &[MicrotopsAotChannel],
    source: SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    let layout = Hdf5LayoutFallback::new(file)?;
    let mut layout_fallback_used = false;
    let mut channel_values = Vec::new();
    let mut axis = Vec::new();
    for channel in channels {
        let read = read_hdf5_1d_f64_with_layout_fallback(file, &layout, &channel.name)?;
        layout_fallback_used |= read.layout_fallback_used;
        channel_values.push(read.values);
        axis.push(channel.wavelength_nm);
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
                channels[index].name
            )));
        }
    }

    let std_values = read_microtops_hdf5_std_channels_with_fallback(
        file,
        &layout,
        channels,
        sample_count,
        &mut layout_fallback_used,
    )?;
    let raw_metadata_floats = read_optional_hdf5_float_series_with_fallback(
        file,
        &layout,
        MICROTOPS_METADATA_FLOATS,
        &mut layout_fallback_used,
    );
    let metadata_floats =
        validate_float_series_lengths(raw_metadata_floats, sample_count, "Microtops NetCDF/HDF5")?;
    let raw_metadata_ints = read_optional_hdf5_int_series_with_fallback(
        file,
        &layout,
        MICROTOPS_METADATA_INTS,
        &mut layout_fallback_used,
    );
    let metadata_ints =
        validate_int_series_lengths(raw_metadata_ints, sample_count, "Microtops NetCDF/HDF5")?;
    let mut extra_warnings: Vec<String> = Vec::new();
    let (global_attributes, global_attr_scan_used) =
        read_microtops_hdf5_global_attributes(file, &layout)?;
    if global_attr_scan_used {
        extra_warnings.push("microtops_man_netcdf_global_attributes_byte_scan".to_string());
    }
    if layout_fallback_used {
        extra_warnings.push("microtops_man_netcdf_contiguous_layout_fallback".to_string());
    }
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
        extra_warnings,
    })
}

fn read_microtops_man_netcdf4_records(
    file: &NcFile,
    channels: &[MicrotopsAotChannel],
    source: SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    let mut channel_values = Vec::new();
    let mut axis = Vec::new();
    for channel in channels {
        channel_values.push(read_netcdf_1d_f64(file, &channel.name)?);
        axis.push(channel.wavelength_nm);
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
                channels[index].name
            )));
        }
    }

    let std_values = read_microtops_std_channels(file, channels, sample_count)?;
    let metadata_floats = validate_float_series_lengths(
        read_optional_netcdf_float_series(file, MICROTOPS_METADATA_FLOATS),
        sample_count,
        "Microtops NetCDF",
    )?;
    let metadata_ints = validate_int_series_lengths(
        read_optional_netcdf_int_series(file, MICROTOPS_METADATA_INTS),
        sample_count,
        "Microtops NetCDF",
    )?;
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
        extra_warnings: Vec::new(),
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
    extra_warnings: Vec<String>,
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
                SignalType::AerosolOpticalThickness,
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
                    SignalType::Uncertainty,
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

        let mut warnings = vec!["microtops_man_netcdf_experimental".to_string()];
        warnings.extend(input.extra_warnings.iter().cloned());
        let record = SpectralRecord {
            signals,
            signal_type: SignalType::AerosolOpticalThickness,
            targets: BTreeMap::new(),
            metadata,
            provenance: provenance(
                "microtops-man-netcdf",
                input.reader,
                input.source.clone(),
                warnings,
            ),
            quality_flags: Vec::new(),
        };
        record.validate()?;
        records.push(record);
    }
    Ok(records)
}

fn discover_microtops_hdf5_aot_channels(file: &Hdf5File, path: &Path) -> Vec<MicrotopsAotChannel> {
    let Ok(root) = file.root_group() else {
        return discover_microtops_hdf5_aot_channels_from_bytes(path);
    };
    let channels = match root.datasets() {
        Ok(datasets) => sorted_microtops_channels(
            datasets
                .iter()
                .filter_map(|dataset| parse_microtops_aot_channel_name(dataset.name())),
        ),
        Err(_) => Vec::new(),
    };
    if channels.is_empty() {
        discover_microtops_hdf5_aot_channels_from_bytes(path)
    } else {
        channels
    }
}

fn discover_microtops_hdf5_aot_channels_from_bytes(path: &Path) -> Vec<MicrotopsAotChannel> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let mut channels = BTreeMap::new();
    let mut index = 0;
    while index + 4 <= bytes.len() {
        if &bytes[index..index + 4] != b"aot_" {
            index += 1;
            continue;
        }
        let mut end = index + 4;
        while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
            end += 1;
        }
        if end > index + 4 {
            if let Ok(name) = std::str::from_utf8(&bytes[index..end]) {
                if let Some(channel) = parse_microtops_aot_channel_name(name) {
                    channels.entry(channel.name.clone()).or_insert(channel);
                }
            }
        }
        index = end.max(index + 1);
    }
    sorted_microtops_channels(channels.into_values())
}

fn discover_microtops_netcdf_aot_channels(file: &NcFile) -> Vec<MicrotopsAotChannel> {
    let Ok(variables) = file.variables() else {
        return Vec::new();
    };
    sorted_microtops_channels(
        variables
            .iter()
            .filter_map(|variable| parse_microtops_aot_channel_name(variable.name())),
    )
}

fn sorted_microtops_channels(
    channels: impl Iterator<Item = MicrotopsAotChannel>,
) -> Vec<MicrotopsAotChannel> {
    let mut channels = channels.collect::<Vec<_>>();
    channels.sort_by(|left, right| {
        left.wavelength_nm
            .partial_cmp(&right.wavelength_nm)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(left.name.cmp(&right.name))
    });
    if channels.len() < 2 {
        return Vec::new();
    }
    channels
}

fn parse_microtops_aot_channel_name(name: &str) -> Option<MicrotopsAotChannel> {
    let normalized = name.trim_start_matches('/');
    let suffix = normalized.strip_prefix("aot_")?;
    if suffix.ends_with("_std")
        || suffix.is_empty()
        || !suffix
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.')
    {
        return None;
    }
    let wavelength_nm = suffix.parse::<f64>().ok()?;
    if !wavelength_nm.is_finite() || wavelength_nm <= 0.0 {
        return None;
    }
    Some(MicrotopsAotChannel {
        name: normalized.to_string(),
        wavelength_nm,
    })
}

fn read_microtops_std_channels(
    file: &NcFile,
    channels: &[MicrotopsAotChannel],
    sample_count: usize,
) -> Result<Option<Vec<Vec<f64>>>> {
    let mut std_channels = Vec::new();
    for channel in channels {
        let std_name = format!("{}_std", channel.name);
        let values = match read_netcdf_1d_f64(file, &std_name) {
            Ok(values) => values,
            Err(_) => return Ok(None),
        };
        if values.len() != sample_count {
            return Err(Error::InvalidRecord(format!(
                "Microtops NetCDF channel {std_name} length does not match AOT channels"
            )));
        }
        std_channels.push(values);
    }
    Ok(Some(std_channels))
}

/// Result of reading a 1-D HDF5 primitive dataset.
///
/// `layout_fallback_used` is `true` when the high-level `hdf5-reader` API
/// failed and we resolved the dataset via the [`Hdf5LayoutFallback`] scan
/// instead. That signals the caller to emit a warning so consumers know the
/// values came from a bytewise contiguous-layout reader rather than the full
/// group/attribute resolution path.
struct Hdf5Read1D<T> {
    values: Vec<T>,
    layout_fallback_used: bool,
}

/// Generic layout-based fallback for NetCDF4/HDF5 files where `hdf5-reader`'s
/// shared-message resolution chokes on the variable's attribute set.
///
/// We do *not* hard-code per-file SHA-256 fixtures or byte offset tables.
/// The fallback works for any HDF5 file whose root group uses v2 dense links
/// and whose datasets use a contiguous data layout — the standard NetCDF4
/// shape produced by `nc-4`. It is keyed on:
///
/// 1. Scanning the file bytes for link records of the form
///    `<name_len:u8> <name:UTF-8> <hard_link_addr:u64_LE>` where
///    `<hard_link_addr>` points to an `OHDR` object header signature.
/// 2. Using `Hdf5File::get_or_parse_header(addr)` (which works in 0.5.0 even
///    when group/attribute iteration fails for this file) to retrieve the
///    `DataLayout::Contiguous { address, size }` and `Datatype` messages.
/// 3. Reading the raw contiguous block from the file storage and decoding it
///    according to the on-disk byte order and primitive datatype.
///
/// Strings, VLENs, compound types, chunked layouts, and non-primitive types
/// are intentionally rejected — this fallback is only meant to recover
/// numeric 1-D dataset payloads.
struct Hdf5LayoutFallback {
    bytes: Vec<u8>,
    addresses: BTreeMap<String, u64>,
}

impl Hdf5LayoutFallback {
    fn new(file: &Hdf5File) -> Result<Self> {
        let total = usize::try_from(file.storage().len()).map_err(|_| {
            Error::InvalidRecord(
                "NetCDF4/HDF5 file is larger than the platform's usize".to_string(),
            )
        })?;
        let buffer = file.storage().read_range(0, total).map_err(|error| {
            Error::InvalidRecord(format!(
                "NetCDF4/HDF5 layout fallback could not read file bytes: {error}"
            ))
        })?;
        let bytes = buffer.as_ref().to_vec();
        let addresses = scan_hdf5_link_records(&bytes);
        Ok(Self { bytes, addresses })
    }

    fn dataset_address(&self, name: &str) -> Option<u64> {
        self.addresses.get(name).copied()
    }

    fn header(
        &self,
        file: &Hdf5File,
        name: &str,
    ) -> Result<std::sync::Arc<hdf5_reader::object_header::ObjectHeader>> {
        let address = self.dataset_address(name).ok_or_else(|| {
            Error::InvalidRecord(format!(
                "NetCDF4/HDF5 layout fallback: no link record found for {name}"
            ))
        })?;
        file.get_or_parse_header(address).map_err(|error| {
            Error::InvalidRecord(format!(
                "NetCDF4/HDF5 layout fallback: failed to parse object header for {name} at {address:#x}: {error}"
            ))
        })
    }
}

/// Scan an HDF5 fractal-heap direct block for hard-link records.
///
/// HDF5 1.10+ NetCDF4 files store the root group's variable links in a
/// fractal heap referenced by the `LinkInfo` message. Each managed link
/// record begins with a 1-byte version (=1), a 1-byte flags field, optional
/// creation-order + character-set fields, a name-length field, the UTF-8
/// name, and (for hard links) an 8-byte object-header address. The name and
/// address are positionally adjacent regardless of the optional fields, so
/// we scan for the pattern `<name_len:u8> <name_bytes> <addr:u64>` and
/// accept matches whose `addr` points to an `OHDR` signature elsewhere in
/// the file.
///
/// This is robust against `hdf5-reader` 0.5.x iteration bugs because it
/// never invokes the high-level fractal-heap traversal. It still produces
/// false positives in pathological files (random bytes that happen to look
/// like a valid record); the caller validates each address via
/// `Hdf5File::get_or_parse_header` before using it.
fn scan_hdf5_link_records(bytes: &[u8]) -> BTreeMap<String, u64> {
    const MIN_NAME_LEN: usize = 2;
    const MAX_NAME_LEN: usize = 64;
    let mut map: BTreeMap<String, u64> = BTreeMap::new();
    if bytes.len() < 1 + MIN_NAME_LEN + 8 {
        return map;
    }
    let last = bytes.len() - (1 + MIN_NAME_LEN + 8);
    for i in 0..=last {
        let name_len = bytes[i] as usize;
        if !(MIN_NAME_LEN..=MAX_NAME_LEN).contains(&name_len) {
            continue;
        }
        let name_end = i + 1 + name_len;
        if name_end + 8 > bytes.len() {
            continue;
        }
        let name_bytes = &bytes[i + 1..name_end];
        if !name_bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        {
            continue;
        }
        let Ok(name) = std::str::from_utf8(name_bytes) else {
            continue;
        };
        let addr_bytes: [u8; 8] = bytes[name_end..name_end + 8].try_into().unwrap();
        let addr = u64::from_le_bytes(addr_bytes);
        let Ok(addr_usize) = usize::try_from(addr) else {
            continue;
        };
        if addr_usize + 4 > bytes.len() {
            continue;
        }
        if &bytes[addr_usize..addr_usize + 4] != b"OHDR" {
            continue;
        }
        // First write wins — link records appear in fractal-heap iteration
        // order, so the first match is the canonical (lowest-address) one.
        map.entry(name.to_string()).or_insert(addr);
    }
    map
}

/// Read a 1-D `f64` dataset, falling back to layout-based decoding on failure.
fn read_hdf5_1d_f64_with_layout_fallback(
    file: &Hdf5File,
    layout: &Hdf5LayoutFallback,
    name: &str,
) -> Result<Hdf5Read1D<f64>> {
    match read_hdf5_1d_f64(file, name) {
        Ok(values) => Ok(Hdf5Read1D {
            values,
            layout_fallback_used: false,
        }),
        Err(_) => {
            let values = read_hdf5_1d_f64_via_layout(file, layout, name)?;
            Ok(Hdf5Read1D {
                values,
                layout_fallback_used: true,
            })
        }
    }
}

/// Read a 1-D `i64` dataset, falling back to layout-based decoding on failure.
fn read_hdf5_1d_i64_with_layout_fallback(
    file: &Hdf5File,
    layout: &Hdf5LayoutFallback,
    name: &str,
) -> Result<Hdf5Read1D<i64>> {
    match read_hdf5_1d_i64(file, name) {
        Ok(values) => Ok(Hdf5Read1D {
            values,
            layout_fallback_used: false,
        }),
        Err(_) => {
            let values = read_hdf5_1d_i64_via_layout(file, layout, name)?;
            Ok(Hdf5Read1D {
                values,
                layout_fallback_used: true,
            })
        }
    }
}

fn read_hdf5_1d_f64_via_layout(
    file: &Hdf5File,
    layout: &Hdf5LayoutFallback,
    name: &str,
) -> Result<Vec<f64>> {
    let (dataspace, datatype, contiguous) = layout_dataset_messages(file, layout, name)?;
    let (size, byte_order) = match datatype {
        HdfDatatype::FloatingPoint { size, byte_order } => (size, byte_order),
        other => {
            return Err(Error::InvalidRecord(format!(
                "NetCDF4/HDF5 layout fallback: {name} datatype is {other:?}, expected 8-byte float"
            )));
        }
    };
    if size != 8 {
        return Err(Error::InvalidRecord(format!(
            "NetCDF4/HDF5 layout fallback: {name} float size is {size}, expected 8"
        )));
    }
    let element_count = expected_element_count(name, &dataspace)?;
    let raw = read_contiguous_bytes(file, name, contiguous, element_count, size as usize)?;
    let mut values = Vec::with_capacity(element_count);
    for chunk in raw.chunks_exact(8) {
        let bytes: [u8; 8] = chunk.try_into().unwrap();
        let value = match byte_order {
            ByteOrder::LittleEndian => f64::from_le_bytes(bytes),
            ByteOrder::BigEndian => f64::from_be_bytes(bytes),
        };
        values.push(value);
    }
    Ok(values)
}

fn read_hdf5_1d_i64_via_layout(
    file: &Hdf5File,
    layout: &Hdf5LayoutFallback,
    name: &str,
) -> Result<Vec<i64>> {
    let (dataspace, datatype, contiguous) = layout_dataset_messages(file, layout, name)?;
    let (size, signed, byte_order) = match datatype {
        HdfDatatype::FixedPoint {
            size,
            signed,
            byte_order,
        } => (size, signed, byte_order),
        other => {
            return Err(Error::InvalidRecord(format!(
                "NetCDF4/HDF5 layout fallback: {name} datatype is {other:?}, expected 8-byte int"
            )));
        }
    };
    if size != 8 || !signed {
        return Err(Error::InvalidRecord(format!(
            "NetCDF4/HDF5 layout fallback: {name} integer must be signed 8-byte (got size={size}, signed={signed})"
        )));
    }
    let element_count = expected_element_count(name, &dataspace)?;
    let raw = read_contiguous_bytes(file, name, contiguous, element_count, size as usize)?;
    let mut values = Vec::with_capacity(element_count);
    for chunk in raw.chunks_exact(8) {
        let bytes: [u8; 8] = chunk.try_into().unwrap();
        let value = match byte_order {
            ByteOrder::LittleEndian => i64::from_le_bytes(bytes),
            ByteOrder::BigEndian => i64::from_be_bytes(bytes),
        };
        values.push(value);
    }
    Ok(values)
}

fn layout_dataset_messages(
    file: &Hdf5File,
    layout: &Hdf5LayoutFallback,
    name: &str,
) -> Result<(
    hdf5_reader::messages::dataspace::DataspaceMessage,
    HdfDatatype,
    (u64, u64),
)> {
    let header = layout.header(file, name)?;
    let mut dataspace = None;
    let mut datatype = None;
    let mut contiguous = None;
    for message in &header.messages {
        match message {
            HdfMessage::Dataspace(d) => dataspace = Some(d.clone()),
            HdfMessage::Datatype(d) => datatype = Some(d.datatype.clone()),
            HdfMessage::DataLayout(l) => match &l.layout {
                DataLayout::Contiguous { address, size } => contiguous = Some((*address, *size)),
                DataLayout::Compact { .. } | DataLayout::Chunked { .. } => {
                    return Err(Error::InvalidRecord(format!(
                        "NetCDF4/HDF5 layout fallback: {name} layout is not contiguous"
                    )));
                }
            },
            _ => {}
        }
    }
    let dataspace = dataspace.ok_or_else(|| {
        Error::InvalidRecord(format!(
            "NetCDF4/HDF5 layout fallback: {name} has no Dataspace message"
        ))
    })?;
    let datatype = datatype.ok_or_else(|| {
        Error::InvalidRecord(format!(
            "NetCDF4/HDF5 layout fallback: {name} has no Datatype message"
        ))
    })?;
    let contiguous = contiguous.ok_or_else(|| {
        Error::InvalidRecord(format!(
            "NetCDF4/HDF5 layout fallback: {name} has no contiguous DataLayout message"
        ))
    })?;
    Ok((dataspace, datatype, contiguous))
}

fn expected_element_count(
    name: &str,
    dataspace: &hdf5_reader::messages::dataspace::DataspaceMessage,
) -> Result<usize> {
    if dataspace.dims.len() != 1 {
        return Err(Error::InvalidRecord(format!(
            "NetCDF4/HDF5 layout fallback: {name} is {}-D, expected 1-D",
            dataspace.dims.len()
        )));
    }
    usize::try_from(dataspace.num_elements()).map_err(|_| {
        Error::InvalidRecord(format!(
            "NetCDF4/HDF5 layout fallback: {name} element count overflows usize"
        ))
    })
}

fn read_contiguous_bytes(
    file: &Hdf5File,
    name: &str,
    contiguous: (u64, u64),
    element_count: usize,
    element_size: usize,
) -> Result<Vec<u8>> {
    let (address, size) = contiguous;
    let expected_bytes = element_count.checked_mul(element_size).ok_or_else(|| {
        Error::InvalidRecord(format!(
            "NetCDF4/HDF5 layout fallback: {name} byte length overflows usize"
        ))
    })?;
    let on_disk_bytes = usize::try_from(size).map_err(|_| {
        Error::InvalidRecord(format!(
            "NetCDF4/HDF5 layout fallback: {name} contiguous size overflows usize"
        ))
    })?;
    if on_disk_bytes < expected_bytes {
        return Err(Error::InvalidRecord(format!(
            "NetCDF4/HDF5 layout fallback: {name} contiguous size {on_disk_bytes} is smaller than {expected_bytes} required"
        )));
    }
    let buffer = file
        .storage()
        .read_range(address, expected_bytes)
        .map_err(|error| {
            Error::InvalidRecord(format!(
            "NetCDF4/HDF5 layout fallback: failed to read {name} payload at {address:#x}: {error}"
        ))
        })?;
    Ok(buffer.as_ref().to_vec())
}

fn read_microtops_hdf5_std_channels_with_fallback(
    file: &Hdf5File,
    layout: &Hdf5LayoutFallback,
    channels: &[MicrotopsAotChannel],
    sample_count: usize,
    layout_fallback_used: &mut bool,
) -> Result<Option<Vec<Vec<f64>>>> {
    let mut std_channels = Vec::new();
    for channel in channels {
        let std_name = format!("{}_std", channel.name);
        let read = match read_hdf5_1d_f64_with_layout_fallback(file, layout, &std_name) {
            Ok(read) => read,
            Err(_) => return Ok(None),
        };
        *layout_fallback_used |= read.layout_fallback_used;
        if read.values.len() != sample_count {
            return Err(Error::InvalidRecord(format!(
                "Microtops NetCDF channel {std_name} length does not match AOT channels"
            )));
        }
        std_channels.push(read.values);
    }
    Ok(Some(std_channels))
}

fn read_optional_hdf5_float_series_with_fallback(
    file: &Hdf5File,
    layout: &Hdf5LayoutFallback,
    names: &[&str],
    layout_fallback_used: &mut bool,
) -> Vec<(String, Vec<f64>)> {
    let mut series = Vec::new();
    for name in names {
        if let Ok(read) = read_hdf5_1d_f64_with_layout_fallback(file, layout, name) {
            *layout_fallback_used |= read.layout_fallback_used;
            series.push(((*name).to_string(), read.values));
        }
    }
    series
}

fn read_optional_hdf5_int_series_with_fallback(
    file: &Hdf5File,
    layout: &Hdf5LayoutFallback,
    names: &[&str],
    layout_fallback_used: &mut bool,
) -> Vec<(String, Vec<i64>)> {
    let mut series = Vec::new();
    for name in names {
        if let Ok(read) = read_hdf5_1d_i64_with_layout_fallback(file, layout, name) {
            *layout_fallback_used |= read.layout_fallback_used;
            series.push(((*name).to_string(), read.values));
        }
    }
    series
}

/// Read the root group's global attributes, falling back to a byte-scan of
/// the known string attribute names when the high-level resolution fails.
///
/// Returns `(attributes, byte_scan_used)`. When `byte_scan_used` is true, the
/// caller should emit a warning so downstream consumers know the attribute
/// map comes from a positional decoder rather than the full HDF5 attribute
/// resolver, which is less robust to non-standard attribute storage.
fn read_microtops_hdf5_global_attributes(
    file: &Hdf5File,
    layout: &Hdf5LayoutFallback,
) -> Result<(BTreeMap<String, Value>, bool)> {
    if let Ok(attributes) = hdf5_global_attributes(file) {
        return Ok((attributes, false));
    }
    let attributes =
        scan_hdf5_global_string_attributes(&layout.bytes, MICROTOPS_GLOBAL_STRING_ATTRS);
    Ok((attributes, true))
}

/// Byte-scan known NetCDF4 v3 global string attribute records.
///
/// Each v3 attribute encodes its value inline immediately after the
/// `<name>\0` bytes as `<class_word:u32_LE><size:u32_LE><dataspace:4>
/// <data:size bytes>`. We only recover fixed-length ASCII strings written
/// with class word `0x13` (string class 3, version 1) and a scalar
/// dataspace (`0x02000000`). VLEN strings (global heap references) and
/// other non-trivial attribute types are intentionally skipped.
fn scan_hdf5_global_string_attributes(bytes: &[u8], names: &[&str]) -> BTreeMap<String, Value> {
    const STRING_CLASS_WORD: u32 = 0x0000_0013;
    const SCALAR_DATASPACE_WORD: u32 = 0x0000_0002;
    let mut out = BTreeMap::new();
    for name in names {
        let needle = format!("{name}\0").into_bytes();
        let mut search_start = 0;
        while let Some(found) = find_subslice(bytes, &needle, search_start) {
            search_start = found + 1;
            let mut cursor = found + needle.len();
            if cursor + 12 > bytes.len() {
                continue;
            }
            let class_word = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
            if class_word != STRING_CLASS_WORD {
                continue;
            }
            cursor += 4;
            let size = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            let dataspace_word = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
            if dataspace_word != SCALAR_DATASPACE_WORD {
                continue;
            }
            cursor += 4;
            if cursor + size > bytes.len() || size == 0 {
                continue;
            }
            let raw = &bytes[cursor..cursor + size];
            let trimmed_end = raw
                .iter()
                .rposition(|byte| *byte != 0)
                .map(|i| i + 1)
                .unwrap_or(0);
            let trimmed = &raw[..trimmed_end];
            if let Ok(value) = std::str::from_utf8(trimmed) {
                out.entry((*name).to_string()).or_insert(json!(value));
                break;
            }
        }
    }
    out
}

fn find_subslice(haystack: &[u8], needle: &[u8], start: usize) -> Option<usize> {
    if needle.is_empty() || start >= haystack.len() {
        return None;
    }
    haystack[start..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|relative| start + relative)
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

fn validate_float_series_lengths(
    series: Vec<(String, Vec<f64>)>,
    sample_count: usize,
    context: &str,
) -> Result<Vec<(String, Vec<f64>)>> {
    for (name, values) in &series {
        if values.len() != sample_count {
            return Err(Error::InvalidRecord(format!(
                "{context} metadata series {name} length {} does not match AOT sample count {sample_count}",
                values.len()
            )));
        }
    }
    Ok(series)
}

fn validate_int_series_lengths(
    series: Vec<(String, Vec<i64>)>,
    sample_count: usize,
    context: &str,
) -> Result<Vec<(String, Vec<i64>)>> {
    for (name, values) in &series {
        if values.len() != sample_count {
            return Err(Error::InvalidRecord(format!(
                "{context} metadata series {name} length {} does not match AOT sample count {sample_count}",
                values.len()
            )));
        }
    }
    Ok(series)
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
    let qc_sidecar = load_arm_mfrsr_qc_sidecar(&source.path)?;
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
        if let Some(sidecar) = &qc_sidecar {
            if let Some(time_seconds) = metadata.get("time").and_then(Value::as_f64) {
                let matching_rules = sidecar
                    .rules
                    .iter()
                    .filter(|rule| rule.matches(time_seconds))
                    .collect::<Vec<_>>();
                if !matching_rules.is_empty() {
                    metadata.insert(
                        "arm_mfrsr_qc_sidecar_flags".to_string(),
                        json!(matching_rules
                            .iter()
                            .map(|rule| rule.metadata_value(&sidecar.source.path))
                            .collect::<Vec<_>>()),
                    );
                    for rule in matching_rules {
                        quality_flags.push(rule.quality_flag());
                    }
                }
            }
        }

        let mut warnings = vec!["arm_mfrsr_netcdf_experimental".to_string()];
        if qc_sidecar.is_some() {
            warnings.push("arm_mfrsr_qc_sidecar_loaded".to_string());
        }
        let mut provenance = provenance("arm-mfrsr-netcdf", reader, source.clone(), warnings);
        if let Some(sidecar) = &qc_sidecar {
            provenance.sources.push(sidecar.source.clone());
        }
        let record = SpectralRecord {
            signals,
            signal_type: SignalType::Irradiance,
            targets: BTreeMap::new(),
            metadata,
            provenance,
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

#[derive(Clone, Debug)]
struct ArmMfrsrQcSidecar {
    source: SourceFile,
    rules: Vec<ArmMfrsrQcRule>,
}

#[derive(Clone, Debug)]
struct ArmMfrsrQcRule {
    variable: String,
    signal_name: String,
    filter: usize,
    severity: String,
    reason: String,
    start_seconds: f64,
    end_seconds: f64,
}

impl ArmMfrsrQcRule {
    fn matches(&self, time_seconds: f64) -> bool {
        time_seconds >= self.start_seconds && time_seconds <= self.end_seconds
    }

    fn quality_flag(&self) -> String {
        format!(
            "arm_mfrsr_sidecar_{}_filter{}_{}",
            self.signal_name,
            self.filter,
            normalize_qc_token(&self.severity)
        )
    }

    fn metadata_value(&self, sidecar_path: &str) -> Value {
        json!({
            "sidecar_path": sidecar_path,
            "variable": self.variable,
            "signal": self.signal_name,
            "filter": self.filter,
            "severity": self.severity,
            "reason": self.reason,
            "start_seconds": self.start_seconds,
            "end_seconds": self.end_seconds,
        })
    }
}

fn load_arm_mfrsr_qc_sidecar(primary_path: &str) -> Result<Option<ArmMfrsrQcSidecar>> {
    let Some(path) = arm_mfrsr_qc_sidecar_path(Path::new(primary_path)) else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(&path).map_err(|source| Error::Io {
        path: path.clone(),
        source,
    })?;
    let rules = parse_arm_mfrsr_qc_sidecar(&text)?;
    if rules.is_empty() {
        return Ok(None);
    }
    let source = SourceFile::from_path(&path, "qc_sidecar")?;
    Ok(Some(ArmMfrsrQcSidecar { source, rules }))
}

fn arm_mfrsr_qc_sidecar_path(primary_path: &Path) -> Option<std::path::PathBuf> {
    let direct = primary_path.with_extension("yaml");
    if direct.exists() {
        return Some(direct);
    }
    let stem = primary_path.file_stem()?.to_string_lossy();
    let (prefix, suffix) = stem.rsplit_once('_')?;
    if suffix.len() == 8 && suffix.chars().all(|ch| ch.is_ascii_digit()) {
        let dated = primary_path.with_file_name(format!("{prefix}.yaml"));
        if dated.exists() {
            return Some(dated);
        }
    }
    None
}

fn parse_arm_mfrsr_qc_sidecar(text: &str) -> Result<Vec<ArmMfrsrQcRule>> {
    let mut rules = Vec::new();
    let mut variable = None::<String>;
    let mut severity = None::<String>;
    let mut reason = None::<String>;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.chars().take_while(|ch| *ch == ' ').count();
        match indent {
            0 if trimmed.ends_with(':') => {
                variable = Some(trimmed.trim_end_matches(':').to_string());
                severity = None;
                reason = None;
            }
            2 if trimmed.ends_with(':') => {
                severity = Some(trimmed.trim_end_matches(':').to_string());
                reason = None;
            }
            4 if trimmed.ends_with(':') => {
                reason = Some(trimmed.trim_end_matches(':').to_string());
            }
            _ if trimmed.starts_with("- ") => {
                let variable = variable.clone().ok_or_else(|| {
                    Error::InvalidRecord("ARM MFRSR QC sidecar range without variable".to_string())
                })?;
                let severity = severity.clone().ok_or_else(|| {
                    Error::InvalidRecord("ARM MFRSR QC sidecar range without severity".to_string())
                })?;
                let reason = reason.clone().unwrap_or_default();
                let (start, end) = trimmed[2..].split_once(',').ok_or_else(|| {
                    Error::InvalidRecord("ARM MFRSR QC sidecar range is not start,end".to_string())
                })?;
                let (signal_name, filter) = parse_arm_mfrsr_qc_variable(&variable)?;
                rules.push(ArmMfrsrQcRule {
                    variable,
                    signal_name,
                    filter,
                    severity,
                    reason,
                    start_seconds: parse_qc_timestamp_seconds(start)?,
                    end_seconds: parse_qc_timestamp_seconds(end)?,
                });
            }
            _ => {}
        }
    }
    Ok(rules)
}

fn parse_arm_mfrsr_qc_variable(variable: &str) -> Result<(String, usize)> {
    let (prefix, filter) = variable.rsplit_once("_filter").ok_or_else(|| {
        Error::InvalidRecord(format!(
            "ARM MFRSR QC sidecar variable {variable} has no filter"
        ))
    })?;
    let filter = filter.parse::<usize>().map_err(|_| {
        Error::InvalidRecord(format!(
            "ARM MFRSR QC sidecar variable {variable} has invalid filter"
        ))
    })?;
    let signal_name = ARM_MFRSR_SIGNALS
        .iter()
        .find(|(candidate, _, _, _)| *candidate == prefix)
        .map(|(_, name, _, _)| (*name).to_string())
        .ok_or_else(|| {
            Error::InvalidRecord(format!(
                "ARM MFRSR QC sidecar variable {variable} is not a known signal"
            ))
        })?;
    Ok((signal_name, filter))
}

fn parse_qc_timestamp_seconds(value: &str) -> Result<f64> {
    let (_, time) = value.trim().rsplit_once(' ').ok_or_else(|| {
        Error::InvalidRecord(format!("ARM MFRSR QC timestamp {value} has no time"))
    })?;
    let parts = time.split(':').collect::<Vec<_>>();
    if !(2..=3).contains(&parts.len()) {
        return Err(Error::InvalidRecord(format!(
            "ARM MFRSR QC timestamp {value} has unsupported time"
        )));
    }
    let hours = parts[0].parse::<u32>().map_err(|_| {
        Error::InvalidRecord(format!("ARM MFRSR QC timestamp {value} has invalid hour"))
    })?;
    let minutes = parts[1].parse::<u32>().map_err(|_| {
        Error::InvalidRecord(format!("ARM MFRSR QC timestamp {value} has invalid minute"))
    })?;
    let seconds = parts
        .get(2)
        .map(|raw| {
            raw.parse::<u32>().map_err(|_| {
                Error::InvalidRecord(format!("ARM MFRSR QC timestamp {value} has invalid second"))
            })
        })
        .transpose()?
        .unwrap_or(0);
    Ok(f64::from(hours * 3600 + minutes * 60 + seconds))
}

fn normalize_qc_token(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
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
