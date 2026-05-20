use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use flate2::read::ZlibDecoder;
use nirs4all_io_core::{AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile};
use serde_json::{json, Value};

use crate::readers::util::{single_signal_record, SingleSignalSpec};
use crate::Reader;

const FORMAT: &str = "digitalsurf-sur-pro";
const MAGIC_UNCOMPRESSED: &[u8; 12] = b"DIGITAL SURF";
const MAGIC_COMPRESSED: &[u8; 12] = b"DSCOMPRESSED";
const FIXED_HEADER_LEN: usize = 512;

pub struct DigitalSurfReader;

impl Reader for DigitalSurfReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::digitalsurf"
    }

    fn sniff(&self, head: &[u8], _path: &Path) -> Option<FormatProbe> {
        if head.starts_with(MAGIC_UNCOMPRESSED) || head.starts_with(MAGIC_COMPRESSED) {
            return Some(FormatProbe::new(
                FORMAT,
                self.name(),
                Confidence::Definite,
                "Digital Surf MountainsMap SUR/PRO spectral container",
            ));
        }
        None
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let source = SourceFile::from_bytes(path, &bytes, "primary");
        parse_digitalsurf(&bytes, source, self.name())
    }
}

#[derive(Clone)]
struct DigitalSurfObject {
    header: DigitalSurfHeader,
    values: Vec<f64>,
}

#[derive(Clone)]
struct DigitalSurfHeader {
    signature: String,
    format: i16,
    number_of_objects: u16,
    version: i16,
    object_type: i16,
    object_name: String,
    operator_name: String,
    p_size: i16,
    acquisition_type: i16,
    range_type: i16,
    special_points: i16,
    absolute: i16,
    gauge_resolution: f32,
    w_size: u32,
    point_size_bits: i16,
    z_min: i32,
    z_max: i32,
    number_of_points: i32,
    number_of_lines: i32,
    total_number_of_points: i32,
    x_spacing: f32,
    y_spacing: f32,
    z_spacing: f32,
    x_axis_name: String,
    y_axis_name: String,
    z_axis_name: String,
    x_step_unit: String,
    y_step_unit: String,
    z_step_unit: String,
    x_length_unit: String,
    y_length_unit: String,
    z_length_unit: String,
    x_unit_ratio: f32,
    y_unit_ratio: f32,
    z_unit_ratio: f32,
    imprint: i16,
    inverted: i16,
    levelled: i16,
    seconds: i16,
    minutes: i16,
    hours: i16,
    day: i16,
    month: i16,
    year: i16,
    day_of_week: i16,
    measurement_duration: f32,
    compressed_data_size: u32,
    comment_size: i16,
    private_size: i16,
    x_offset: f32,
    y_offset: f32,
    z_offset: f32,
    t_spacing: f32,
    t_offset: f32,
    t_axis_name: String,
    t_step_unit: String,
    comment: String,
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

#[derive(Clone)]
struct AxisSpec {
    values: Vec<f64>,
    unit: String,
    kind: AxisKind,
    original_name: String,
    original_unit: String,
}

fn parse_digitalsurf(
    bytes: &[u8],
    source: SourceFile,
    reader_name: &str,
) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
    if !(bytes.starts_with(MAGIC_UNCOMPRESSED) || bytes.starts_with(MAGIC_COMPRESSED)) {
        return Err(Error::InvalidRecord(
            "missing DigitalSurf SUR/PRO signature".to_string(),
        ));
    }

    let objects = parse_objects(bytes)?;
    let Some(first) = objects.first() else {
        return Err(Error::InvalidRecord(
            "DigitalSurf file contains no object".to_string(),
        ));
    };

    match object_type_label(first.header.object_type) {
        "_SPECTRUM" => records_from_spectrum(&objects, source, reader_name),
        "_HYPCARD" => records_from_hyperspectral_map(first, source, reader_name),
        "_SURFACE" | "_INTENSITYIMAGE" => records_from_surface(first, source, reader_name),
        "_PROFILE" => records_from_profile(first, source, reader_name),
        label => Err(Error::InvalidRecord(format!(
            "DigitalSurf object type {label} ({}) is not supported yet",
            first.header.object_type
        ))),
    }
}

fn parse_objects(bytes: &[u8]) -> Result<Vec<DigitalSurfObject>> {
    let mut cursor = Cursor { bytes, offset: 0 };
    let first = parse_object(&mut cursor)?;
    let object_count = usize::from(first.header.number_of_objects.max(1));
    let channel_count = usize::try_from(first.header.p_size.max(1))
        .map_err(|_| Error::InvalidRecord("DigitalSurf channel count is negative".to_string()))?;
    let total = object_count.saturating_mul(channel_count).max(1);
    let mut objects = Vec::with_capacity(total);
    objects.push(first);

    for _ in 1..total {
        if cursor.offset >= bytes.len() {
            break;
        }
        objects.push(parse_object(&mut cursor)?);
    }

    Ok(objects)
}

fn parse_object(cursor: &mut Cursor<'_>) -> Result<DigitalSurfObject> {
    let header_start = cursor.offset;
    let signature = cursor.read_latin1_string(12)?;
    if signature != "DIGITAL SURF" && signature != "DSCOMPRESSED" {
        return Err(Error::InvalidRecord(format!(
            "invalid DigitalSurf object signature {signature:?}"
        )));
    }

    let format = cursor.read_i16()?;
    if format != 0 {
        return Err(Error::InvalidRecord(format!(
            "unsupported DigitalSurf endian/platform format {format}"
        )));
    }

    let number_of_objects = cursor.read_u16()?;
    let version = cursor.read_i16()?;
    let object_type = cursor.read_i16()?;
    let object_name = cursor.read_latin1_string(30)?;
    let operator_name = cursor.read_latin1_string(30)?;
    let p_size = cursor.read_i16()?;
    let acquisition_type = cursor.read_i16()?;
    let range_type = cursor.read_i16()?;
    let special_points = cursor.read_i16()?;
    let absolute = cursor.read_i16()?;
    let gauge_resolution = cursor.read_f32()?;
    let w_size = cursor.read_u32()?;
    let point_size_bits = cursor.read_i16()?;
    let z_min = cursor.read_i32()?;
    let z_max = cursor.read_i32()?;
    let number_of_points = cursor.read_i32()?;
    let number_of_lines = cursor.read_i32()?;
    let total_number_of_points = cursor.read_i32()?;
    let x_spacing = cursor.read_f32()?;
    let y_spacing = cursor.read_f32()?;
    let z_spacing = cursor.read_f32()?;
    let x_axis_name = cursor.read_latin1_string(16)?;
    let y_axis_name = cursor.read_latin1_string(16)?;
    let z_axis_name = cursor.read_latin1_string(16)?;
    let x_step_unit = cursor.read_latin1_string(16)?;
    let y_step_unit = cursor.read_latin1_string(16)?;
    let z_step_unit = cursor.read_latin1_string(16)?;
    let x_length_unit = cursor.read_latin1_string(16)?;
    let y_length_unit = cursor.read_latin1_string(16)?;
    let z_length_unit = cursor.read_latin1_string(16)?;
    let x_unit_ratio = cursor.read_f32()?;
    let y_unit_ratio = cursor.read_f32()?;
    let z_unit_ratio = cursor.read_f32()?;
    let imprint = cursor.read_i16()?;
    let inverted = cursor.read_i16()?;
    let levelled = cursor.read_i16()?;
    cursor.skip(12)?;
    let seconds = cursor.read_i16()?;
    let minutes = cursor.read_i16()?;
    let hours = cursor.read_i16()?;
    let day = cursor.read_i16()?;
    let month = cursor.read_i16()?;
    let year = cursor.read_i16()?;
    let day_of_week = cursor.read_i16()?;
    let measurement_duration = cursor.read_f32()?;
    let compressed_data_size = cursor.read_u32()?;
    cursor.skip(6)?;
    let comment_size = cursor.read_i16()?;
    let private_size = cursor.read_i16()?;
    cursor.skip(128)?;
    let x_offset = cursor.read_f32()?;
    let y_offset = cursor.read_f32()?;
    let z_offset = cursor.read_f32()?;
    let t_spacing = cursor.read_f32()?;
    let t_offset = cursor.read_f32()?;
    let t_axis_name = cursor.read_latin1_string(13)?;
    let t_step_unit = cursor.read_latin1_string(13)?;

    let fixed_len = cursor.offset - header_start;
    if fixed_len != FIXED_HEADER_LEN {
        return Err(Error::InvalidRecord(format!(
            "DigitalSurf fixed header length mismatch: parsed {fixed_len} bytes"
        )));
    }

    let comment = cursor.read_latin1_string(non_negative_len(comment_size, "comment")?)?;
    cursor.skip(non_negative_len(private_size, "private zone")?)?;

    let mut header = DigitalSurfHeader {
        signature,
        format,
        number_of_objects,
        version,
        object_type,
        object_name,
        operator_name,
        p_size,
        acquisition_type,
        range_type,
        special_points,
        absolute,
        gauge_resolution,
        w_size,
        point_size_bits,
        z_min,
        z_max,
        number_of_points,
        number_of_lines,
        total_number_of_points,
        x_spacing,
        y_spacing,
        z_spacing,
        x_axis_name,
        y_axis_name,
        z_axis_name,
        x_step_unit,
        y_step_unit,
        z_step_unit,
        x_length_unit,
        y_length_unit,
        z_length_unit,
        x_unit_ratio,
        y_unit_ratio,
        z_unit_ratio,
        imprint,
        inverted,
        levelled,
        seconds,
        minutes,
        hours,
        day,
        month,
        year,
        day_of_week,
        measurement_duration,
        compressed_data_size,
        comment_size,
        private_size,
        x_offset,
        y_offset,
        z_offset,
        t_spacing,
        t_offset,
        t_axis_name,
        t_step_unit,
        comment,
    };
    validate_header_dimensions(&header)?;

    let raw_points = read_raw_points(cursor, &header)?;
    let values = scale_points(&raw_points, &header);
    header.comment = trim_comment(&header.comment);
    Ok(DigitalSurfObject { header, values })
}

fn records_from_spectrum(
    objects: &[DigitalSurfObject],
    source: SourceFile,
    reader_name: &str,
) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
    let object = objects.first().expect("first object");
    let header = &object.header;
    let axis = axis_from_linear(
        &header.x_axis_name,
        &header.x_step_unit,
        header.x_offset,
        header.x_spacing,
        usize_from_i32(header.number_of_points, "number of points")?,
    );
    let rows = usize_from_i32(header.number_of_lines, "number of lines")?;
    let points = axis.values.len();
    ensure_len(
        &object.values,
        rows * points,
        "DigitalSurf spectrum payload",
    )?;

    let mut out = Vec::with_capacity(rows);
    for row in 0..rows {
        let mut metadata = base_metadata(header);
        metadata.insert("spectrum_index".to_string(), json!(row));
        metadata.insert("row_index".to_string(), json!(row));
        if rows > 1 {
            metadata.insert(
                "spectrum_position".to_string(),
                json!(axis_value(header.y_offset, header.y_spacing, row)),
            );
            metadata.insert(
                "spectrum_position_unit".to_string(),
                json!(header.y_step_unit),
            );
        }
        insert_axis_metadata(&mut metadata, "signal_axis", &axis);

        let start = row * points;
        out.push(build_record(
            source.clone(),
            reader_name,
            axis.clone(),
            object.values[start..start + points].to_vec(),
            header,
            metadata,
            Vec::new(),
        )?);
    }
    Ok(out)
}

fn records_from_profile(
    object: &DigitalSurfObject,
    source: SourceFile,
    reader_name: &str,
) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
    let header = &object.header;
    let axis = axis_from_linear(
        &header.x_axis_name,
        &header.x_step_unit,
        header.x_offset,
        header.x_spacing,
        usize_from_i32(header.number_of_points, "number of points")?,
    );
    ensure_len(
        &object.values,
        axis.values.len(),
        "DigitalSurf profile payload",
    )?;
    let mut metadata = base_metadata(header);
    metadata.insert("spectrum_index".to_string(), json!(0));
    insert_axis_metadata(&mut metadata, "signal_axis", &axis);
    build_record(
        source,
        reader_name,
        axis,
        object.values.clone(),
        header,
        metadata,
        vec!["digitalsurf_profile_axis_is_not_guaranteed_spectral".to_string()],
    )
    .map(|record| vec![record])
}

fn records_from_hyperspectral_map(
    object: &DigitalSurfObject,
    source: SourceFile,
    reader_name: &str,
) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
    let header = &object.header;
    let width = usize_from_i32(header.number_of_points, "number of points")?;
    let height = usize_from_i32(header.number_of_lines, "number of lines")?;
    let spectral_len = usize::try_from(header.w_size).map_err(|_| {
        Error::InvalidRecord("DigitalSurf W axis size does not fit usize".to_string())
    })?;
    if spectral_len == 0 {
        return Err(Error::InvalidRecord(
            "DigitalSurf hyperspectral map has zero W axis size".to_string(),
        ));
    }
    ensure_len(
        &object.values,
        width * height * spectral_len,
        "DigitalSurf hyperspectral payload",
    )?;

    let axis = axis_from_linear(
        &header.t_axis_name,
        &header.t_step_unit,
        header.t_offset,
        effective_t_spacing(header.t_spacing),
        spectral_len,
    );
    let mut out = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let mut metadata = base_metadata(header);
            let spectrum_index = y * width + x;
            metadata.insert("spectrum_index".to_string(), json!(spectrum_index));
            metadata.insert("map_width".to_string(), json!(width));
            metadata.insert("map_height".to_string(), json!(height));
            metadata.insert("map_axis_order".to_string(), json!("y_slowest_x_fastest"));
            metadata.insert("map_x_index".to_string(), json!(x));
            metadata.insert("map_y_index".to_string(), json!(y));
            metadata.insert("spatial_x_index".to_string(), json!(x));
            metadata.insert("spatial_y_index".to_string(), json!(y));
            metadata.insert(
                "spatial_x".to_string(),
                json!(axis_value(header.x_offset, header.x_spacing, x)),
            );
            metadata.insert(
                "spatial_y".to_string(),
                json!(axis_value(header.y_offset, header.y_spacing, y)),
            );
            metadata.insert("spatial_x_unit".to_string(), json!(header.x_step_unit));
            metadata.insert("spatial_y_unit".to_string(), json!(header.y_step_unit));
            insert_axis_metadata(&mut metadata, "signal_axis", &axis);

            let start = spectrum_index * spectral_len;
            out.push(build_record(
                source.clone(),
                reader_name,
                axis.clone(),
                object.values[start..start + spectral_len].to_vec(),
                header,
                metadata,
                Vec::new(),
            )?);
        }
    }
    Ok(out)
}

fn records_from_surface(
    object: &DigitalSurfObject,
    source: SourceFile,
    reader_name: &str,
) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
    let header = &object.header;
    let width = usize_from_i32(header.number_of_points, "number of points")?;
    let height = usize_from_i32(header.number_of_lines, "number of lines")?;
    ensure_len(
        &object.values,
        width * height,
        "DigitalSurf surface payload",
    )?;
    let axis = axis_from_linear(
        &header.x_axis_name,
        &header.x_step_unit,
        header.x_offset,
        header.x_spacing,
        width,
    );

    let mut out = Vec::with_capacity(height);
    for row in 0..height {
        let mut metadata = base_metadata(header);
        metadata.insert("spectrum_index".to_string(), json!(row));
        metadata.insert("surface_row_index".to_string(), json!(row));
        metadata.insert("spatial_y_index".to_string(), json!(row));
        metadata.insert("surface_width".to_string(), json!(width));
        metadata.insert("surface_height".to_string(), json!(height));
        metadata.insert(
            "surface_axis_order".to_string(),
            json!("row_profiles_y_slowest_x_fastest"),
        );
        metadata.insert(
            "spatial_y".to_string(),
            json!(axis_value(header.y_offset, header.y_spacing, row)),
        );
        metadata.insert("spatial_x_unit".to_string(), json!(&axis.unit));
        metadata.insert("spatial_y_unit".to_string(), json!(header.y_step_unit));
        insert_axis_metadata(&mut metadata, "profile_axis", &axis);

        let start = row * width;
        out.push(build_record(
            source.clone(),
            reader_name,
            axis.clone(),
            object.values[start..start + width].to_vec(),
            header,
            metadata,
            vec!["digitalsurf_surface_rows_exported_as_profiles".to_string()],
        )?);
    }
    Ok(out)
}

fn build_record(
    source: SourceFile,
    reader_name: &str,
    axis: AxisSpec,
    values: Vec<f64>,
    header: &DigitalSurfHeader,
    metadata: BTreeMap<String, Value>,
    mut extra_warnings: Vec<String>,
) -> Result<nirs4all_io_core::SpectralRecord> {
    let signal_type = signal_type_from_header(header);
    let signal_unit = clean_optional(&header.z_step_unit);
    let signal_name = if matches!(signal_type, SignalType::RawCounts) {
        "intensity"
    } else {
        "signal"
    };
    let mut warnings = vec!["digitalsurf_reverse_engineered_header".to_string()];
    if header.signature == "DSCOMPRESSED" {
        warnings.push("digitalsurf_zlib_stream_decompressed".to_string());
    }
    if axis.kind == AxisKind::Index {
        warnings.push("digitalsurf_axis_kind_index".to_string());
    }
    warnings.append(&mut extra_warnings);

    single_signal_record(
        FORMAT,
        reader_name,
        source,
        SingleSignalSpec {
            axis_values: axis.values,
            axis_unit: axis.unit,
            axis_kind: axis.kind,
            values,
            signal_name: signal_name.to_string(),
            signal_type,
            signal_unit,
            role: "intensity".to_string(),
        },
        BTreeMap::new(),
        metadata,
        warnings,
    )
}

fn read_raw_points(cursor: &mut Cursor<'_>, header: &DigitalSurfHeader) -> Result<Vec<i32>> {
    let point_size = point_size_bytes(header)?;
    let expected_points = expected_point_count(header)?;
    let raw = if header.signature == "DSCOMPRESSED" {
        read_compressed_payload(cursor, point_size, expected_points)?
    } else {
        let byte_len = expected_points.checked_mul(point_size).ok_or_else(|| {
            Error::InvalidRecord("DigitalSurf payload byte count overflows usize".to_string())
        })?;
        cursor.read_bytes(byte_len)?.to_vec()
    };

    if raw.len() != expected_points * point_size {
        return Err(Error::InvalidRecord(format!(
            "DigitalSurf payload has {} bytes, expected {}",
            raw.len(),
            expected_points * point_size
        )));
    }
    decode_points(&raw, point_size)
}

fn read_compressed_payload(
    cursor: &mut Cursor<'_>,
    point_size: usize,
    expected_points: usize,
) -> Result<Vec<u8>> {
    let stream_count = cursor.read_u32()? as usize;
    if stream_count == 0 || stream_count > 64 {
        return Err(Error::InvalidRecord(format!(
            "DigitalSurf compressed stream count {stream_count} is invalid"
        )));
    }
    let mut directory = Vec::with_capacity(stream_count);
    for _ in 0..stream_count {
        let raw_len = cursor.read_u32()? as usize;
        let zip_len = cursor.read_u32()? as usize;
        directory.push((raw_len, zip_len));
    }

    let expected_bytes = expected_points * point_size;
    let declared_raw_bytes = directory.iter().map(|(raw_len, _)| *raw_len).sum::<usize>();
    if declared_raw_bytes != expected_bytes {
        return Err(Error::InvalidRecord(format!(
            "DigitalSurf compressed streams declare {declared_raw_bytes} raw bytes, expected {expected_bytes}"
        )));
    }

    let mut out = Vec::with_capacity(expected_bytes);
    for (raw_len, zip_len) in directory {
        let compressed = cursor.read_bytes(zip_len)?;
        let mut decoder = ZlibDecoder::new(compressed);
        let mut decoded = Vec::with_capacity(raw_len);
        decoder.read_to_end(&mut decoded).map_err(|error| {
            Error::InvalidRecord(format!("DigitalSurf zlib decode error: {error}"))
        })?;
        if decoded.len() != raw_len {
            return Err(Error::InvalidRecord(format!(
                "DigitalSurf zlib stream decoded {} bytes, expected {raw_len}",
                decoded.len()
            )));
        }
        out.extend(decoded);
    }
    Ok(out)
}

fn decode_points(raw: &[u8], point_size: usize) -> Result<Vec<i32>> {
    match point_size {
        2 => Ok(raw
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as i32)
            .collect()),
        4 => Ok(raw
            .chunks_exact(4)
            .map(|chunk| i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()),
        _ => Err(Error::InvalidRecord(format!(
            "unsupported DigitalSurf point size {point_size} bytes"
        ))),
    }
}

fn scale_points(raw_points: &[i32], header: &DigitalSurfHeader) -> Vec<f64> {
    if surface_can_stay_integer(header) {
        return raw_points.iter().map(|value| f64::from(*value)).collect();
    }

    let unit_ratio = if header.z_unit_ratio == 0.0 {
        1.0
    } else {
        header.z_unit_ratio
    };
    let scale = f64::from(header.z_spacing / unit_ratio);
    let offset = f64::from(header.z_offset);
    let z_min = f64::from(header.z_min);
    let non_measured = header.z_min.saturating_sub(2);
    raw_points
        .iter()
        .map(|value| {
            if header.special_points == 1 && *value == non_measured {
                f64::NAN
            } else {
                (f64::from(*value) - z_min) * scale + offset
            }
        })
        .collect()
}

fn surface_can_stay_integer(header: &DigitalSurfHeader) -> bool {
    if !matches!(
        object_type_label(header.object_type),
        "_SURFACE" | "_SURFACESERIE"
    ) {
        return false;
    }
    let unit_ratio = if header.z_unit_ratio == 0.0 {
        1.0
    } else {
        header.z_unit_ratio
    };
    let scale = header.z_spacing / unit_ratio;
    scale.fract() == 0.0 && header.z_offset.fract() == 0.0
}

fn base_metadata(header: &DigitalSurfHeader) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    metadata.insert("container".to_string(), json!("digitalsurf"));
    metadata.insert("signature".to_string(), json!(header.signature));
    metadata.insert("format".to_string(), json!(header.format));
    metadata.insert("version".to_string(), json!(header.version));
    metadata.insert("object_type_code".to_string(), json!(header.object_type));
    metadata.insert(
        "object_type_label".to_string(),
        json!(object_type_label(header.object_type)),
    );
    metadata.insert(
        "number_of_objects".to_string(),
        json!(header.number_of_objects),
    );
    metadata.insert("channel_count".to_string(), json!(header.p_size));
    metadata.insert(
        "acquisition_type".to_string(),
        json!(header.acquisition_type),
    );
    metadata.insert("range_type".to_string(), json!(header.range_type));
    metadata.insert("special_points".to_string(), json!(header.special_points));
    metadata.insert("absolute".to_string(), json!(header.absolute));
    metadata.insert(
        "gauge_resolution".to_string(),
        json!(header.gauge_resolution),
    );
    metadata.insert("w_size".to_string(), json!(header.w_size));
    metadata.insert("point_size_bits".to_string(), json!(header.point_size_bits));
    metadata.insert("z_min_raw".to_string(), json!(header.z_min));
    metadata.insert("z_max_raw".to_string(), json!(header.z_max));
    metadata.insert(
        "number_of_points".to_string(),
        json!(header.number_of_points),
    );
    metadata.insert("number_of_lines".to_string(), json!(header.number_of_lines));
    metadata.insert(
        "total_number_of_points".to_string(),
        json!(header.total_number_of_points),
    );
    metadata.insert("x_axis_name".to_string(), json!(header.x_axis_name));
    metadata.insert("y_axis_name".to_string(), json!(header.y_axis_name));
    metadata.insert("z_axis_name".to_string(), json!(header.z_axis_name));
    metadata.insert("x_step_unit".to_string(), json!(header.x_step_unit));
    metadata.insert("y_step_unit".to_string(), json!(header.y_step_unit));
    metadata.insert("z_step_unit".to_string(), json!(header.z_step_unit));
    metadata.insert("x_length_unit".to_string(), json!(header.x_length_unit));
    metadata.insert("y_length_unit".to_string(), json!(header.y_length_unit));
    metadata.insert("z_length_unit".to_string(), json!(header.z_length_unit));
    metadata.insert("x_spacing".to_string(), json!(header.x_spacing));
    metadata.insert("y_spacing".to_string(), json!(header.y_spacing));
    metadata.insert("z_spacing".to_string(), json!(header.z_spacing));
    metadata.insert("x_offset".to_string(), json!(header.x_offset));
    metadata.insert("y_offset".to_string(), json!(header.y_offset));
    metadata.insert("z_offset".to_string(), json!(header.z_offset));
    metadata.insert("x_unit_ratio".to_string(), json!(header.x_unit_ratio));
    metadata.insert("y_unit_ratio".to_string(), json!(header.y_unit_ratio));
    metadata.insert("z_unit_ratio".to_string(), json!(header.z_unit_ratio));
    metadata.insert("imprint".to_string(), json!(header.imprint));
    metadata.insert("inverted".to_string(), json!(header.inverted));
    metadata.insert("levelled".to_string(), json!(header.levelled));
    metadata.insert("day_of_week".to_string(), json!(header.day_of_week));
    metadata.insert(
        "measurement_duration".to_string(),
        json!(header.measurement_duration),
    );
    metadata.insert(
        "compressed_data_size".to_string(),
        json!(header.compressed_data_size),
    );
    metadata.insert("comment_size".to_string(), json!(header.comment_size));
    metadata.insert("private_size".to_string(), json!(header.private_size));
    metadata.insert("t_axis_name".to_string(), json!(header.t_axis_name));
    metadata.insert("t_step_unit".to_string(), json!(header.t_step_unit));
    metadata.insert("t_spacing".to_string(), json!(header.t_spacing));
    metadata.insert("t_offset".to_string(), json!(header.t_offset));
    metadata.insert(
        "signal_quantity".to_string(),
        json!(signal_quantity(header)),
    );
    if !header.object_name.is_empty() {
        metadata.insert("object_name".to_string(), json!(header.object_name));
    }
    if !header.operator_name.is_empty() {
        metadata.insert("operator_name".to_string(), json!(header.operator_name));
    }
    if let Some(timestamp) = acquisition_timestamp(header) {
        metadata.insert("acquisition_timestamp".to_string(), json!(timestamp));
    }
    if !header.comment.is_empty() {
        metadata.insert("comment".to_string(), json!(header.comment));
    }
    metadata
}

fn insert_axis_metadata(metadata: &mut BTreeMap<String, Value>, prefix: &str, axis: &AxisSpec) {
    metadata.insert(format!("{prefix}_name"), json!(axis.original_name));
    metadata.insert(format!("{prefix}_original_unit"), json!(axis.original_unit));
    metadata.insert(format!("{prefix}_unit"), json!(axis.unit));
    metadata.insert(format!("{prefix}_kind"), json!(axis.kind));
}

fn axis_from_linear(name: &str, unit: &str, offset: f32, scale: f32, size: usize) -> AxisSpec {
    let lower_name = name.to_ascii_lowercase();
    let lower_unit = unit.to_ascii_lowercase();
    let kind = if lower_name.contains("wavelength") {
        AxisKind::Wavelength
    } else if lower_name.contains("wavenumber")
        || lower_unit.contains("cm-1")
        || lower_unit.contains("1/cm")
    {
        AxisKind::Wavenumber
    } else if lower_name.contains("frequency") {
        AxisKind::Frequency
    } else {
        AxisKind::Index
    };
    let (multiplier, canonical_unit) = canonical_axis_unit(&kind, unit);
    let values = (0..size)
        .map(|index| axis_value(offset, scale, index) * multiplier)
        .collect();
    AxisSpec {
        values,
        unit: canonical_unit,
        kind,
        original_name: name.to_string(),
        original_unit: unit.to_string(),
    }
}

fn canonical_axis_unit(kind: &AxisKind, unit: &str) -> (f64, String) {
    let normalized = unit.trim().to_ascii_lowercase();
    match kind {
        AxisKind::Wavelength => match normalized.as_str() {
            "nm" => (1.0, "nm".to_string()),
            "um" | "µm" => (1_000.0, "nm".to_string()),
            "mm" => (1_000_000.0, "nm".to_string()),
            "m" => (1_000_000_000.0, "nm".to_string()),
            _ => (1.0, unit.to_string()),
        },
        AxisKind::Wavenumber => {
            if normalized == "1/cm" {
                (1.0, "cm-1".to_string())
            } else {
                (1.0, unit.to_string())
            }
        }
        AxisKind::Frequency | AxisKind::Energy | AxisKind::Time | AxisKind::Index => {
            (1.0, unit.to_string())
        }
    }
}

fn axis_value(offset: f32, spacing: f32, index: usize) -> f64 {
    f64::from(offset) + f64::from(spacing) * index as f64
}

fn effective_t_spacing(value: f32) -> f32 {
    if value == 0.0 {
        1.0
    } else {
        value
    }
}

fn signal_type_from_header(header: &DigitalSurfHeader) -> SignalType {
    let label = format!(
        "{} {}",
        header.z_axis_name.to_ascii_lowercase(),
        header.z_step_unit.to_ascii_lowercase()
    );
    if label.contains("abs") {
        SignalType::Absorbance
    } else if label.contains("reflect") {
        SignalType::Reflectance
    } else if label.contains("trans") {
        SignalType::Transmittance
    } else if label.contains("count") || label.contains("intensity") || label.contains("a.u.") {
        SignalType::RawCounts
    } else {
        SignalType::Unknown
    }
}

fn signal_quantity(header: &DigitalSurfHeader) -> String {
    match clean_optional(&header.z_step_unit) {
        Some(unit) => format!("{} ({unit})", header.z_axis_name)
            .trim()
            .to_string(),
        None => header.z_axis_name.clone(),
    }
}

fn clean_optional(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn acquisition_timestamp(header: &DigitalSurfHeader) -> Option<String> {
    if header.year == 0 && header.month == 0 && header.day == 0 {
        return None;
    }
    Some(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        header.year, header.month, header.day, header.hours, header.minutes, header.seconds
    ))
}

fn validate_header_dimensions(header: &DigitalSurfHeader) -> Result<()> {
    if header.point_size_bits != 16 && header.point_size_bits != 32 {
        return Err(Error::InvalidRecord(format!(
            "DigitalSurf point size is {}, expected 16 or 32 bits",
            header.point_size_bits
        )));
    }
    usize_from_i32(header.number_of_points, "number of points")?;
    usize_from_i32(header.number_of_lines, "number of lines")?;
    usize_from_i32(header.total_number_of_points, "total number of points")?;
    expected_point_count(header)?;
    Ok(())
}

fn point_size_bytes(header: &DigitalSurfHeader) -> Result<usize> {
    match header.point_size_bits {
        16 => Ok(2),
        32 => Ok(4),
        other => Err(Error::InvalidRecord(format!(
            "DigitalSurf point size is {other}, expected 16 or 32 bits"
        ))),
    }
}

fn expected_point_count(header: &DigitalSurfHeader) -> Result<usize> {
    let total = usize_from_i32(header.total_number_of_points, "total number of points")?;
    let w_size = usize::try_from(header.w_size.max(1)).map_err(|_| {
        Error::InvalidRecord("DigitalSurf W axis size does not fit usize".to_string())
    })?;
    total.checked_mul(w_size).ok_or_else(|| {
        Error::InvalidRecord("DigitalSurf expected point count overflows usize".to_string())
    })
}

fn ensure_len(values: &[f64], expected: usize, label: &str) -> Result<()> {
    if values.len() != expected {
        return Err(Error::InvalidRecord(format!(
            "{label} has {} values, expected {expected}",
            values.len()
        )));
    }
    Ok(())
}

fn usize_from_i32(value: i32, label: &str) -> Result<usize> {
    if value <= 0 {
        return Err(Error::InvalidRecord(format!(
            "DigitalSurf {label} must be positive, got {value}"
        )));
    }
    usize::try_from(value)
        .map_err(|_| Error::InvalidRecord(format!("DigitalSurf {label} does not fit usize")))
}

fn non_negative_len(value: i16, label: &str) -> Result<usize> {
    if value < 0 {
        return Err(Error::InvalidRecord(format!(
            "DigitalSurf {label} size is negative ({value})"
        )));
    }
    Ok(value as usize)
}

fn object_type_label(code: i16) -> &'static str {
    match code {
        -1 => "_ERROR",
        0 => "_UNKNOWN",
        1 => "_PROFILE",
        2 => "_SURFACE",
        3 => "_BINARYIMAGE",
        4 => "_PROFILESERIE",
        5 => "_SURFACESERIE",
        6 => "_MERIDIANDISC",
        7 => "_MULTILAYERPROFILE",
        8 => "_MULTILAYERSURFACE",
        9 => "_PARALLELDISC",
        10 => "_INTENSITYIMAGE",
        11 => "_INTENSITYSURFACE",
        12 => "_RGBIMAGE",
        13 => "_RGBSURFACE",
        14 => "_FORCECURVE",
        15 => "_SERIEOFFORCECURVE",
        16 => "_RGBINTENSITYSURFACE",
        17 => "_CONTOURPROFILE",
        18 => "_SERIESOFRGBIMAGES",
        20 => "_SPECTRUM",
        21 => "_HYPCARD",
        _ => "_UNKNOWN_VENDOR_TYPE",
    }
}

fn trim_comment(comment: &str) -> String {
    comment
        .trim_matches(|ch| matches!(ch, '\0' | ' ' | '\t' | '\n' | '\r'))
        .to_string()
}

impl<'a> Cursor<'a> {
    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.offset.checked_add(len).ok_or_else(|| {
            Error::InvalidRecord("DigitalSurf cursor offset overflow".to_string())
        })?;
        if end > self.bytes.len() {
            return Err(Error::InvalidRecord(format!(
                "DigitalSurf truncated file: need {len} bytes at offset {}",
                self.offset
            )));
        }
        let out = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(out)
    }

    fn skip(&mut self, len: usize) -> Result<()> {
        self.read_bytes(len).map(|_| ())
    }

    fn read_i16(&mut self) -> Result<i16> {
        let bytes = self.read_bytes(2)?;
        Ok(i16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u16(&mut self) -> Result<u16> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_i32(&mut self) -> Result<i32> {
        let bytes = self.read_bytes(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u32(&mut self) -> Result<u32> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_f32(&mut self) -> Result<f32> {
        let bytes = self.read_bytes(4)?;
        Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_latin1_string(&mut self, len: usize) -> Result<String> {
        let bytes = self.read_bytes(len)?;
        Ok(bytes
            .iter()
            .map(|byte| char::from(*byte))
            .collect::<String>()
            .trim_matches(|ch| matches!(ch, '\0' | ' ' | '\t' | '\n' | '\r'))
            .to_string())
    }
}
