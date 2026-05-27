use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nirs4all_formats_core::{
    AxisKind, Confidence, Error, FormatProbe, Provenance, Result, SidecarResolver, SignalType,
    SourceFile, SpectralArray, SpectralAxis, SpectralRecord,
};
use serde_json::json;

use crate::readers::util::parse_number;
use crate::registry::{cube_pixels, cube_region, ReadOptions};
use crate::sidecars::FsSidecars;
use crate::Reader;

const FORMAT: &str = "erdas-lan-aviris";
const MAGIC: &[u8] = b"HEAD74";
const HEADER_LEN: usize = 128;
const SUPPORTED_ROWS: usize = 145;
const SUPPORTED_COLS: usize = 145;
const SUPPORTED_BANDS: usize = 220;

pub struct ErdasLanReader;

impl Reader for ErdasLanReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::erdas_lan"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        if ext != "lan" || !head.starts_with(MAGIC) {
            return None;
        }
        let header = LanHeader::parse(head).ok()?;
        if header.rows == SUPPORTED_ROWS
            && header.cols == SUPPORTED_COLS
            && header.bands == SUPPORTED_BANDS
        {
            Some(FormatProbe::new(
                FORMAT,
                self.name(),
                Confidence::Definite,
                "ERDAS LAN AVIRIS BIL cube detected",
            ))
        } else {
            Some(FormatProbe::new(
                "erdas-lan",
                self.name(),
                Confidence::Likely,
                "ERDAS LAN container detected; only the 92AV3C AVIRIS layout is supported",
            ))
        }
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        self.read_path_with_options(path, &ReadOptions::default())
    }

    fn read_path_with_options(
        &self,
        path: &Path,
        options: &ReadOptions,
    ) -> Result<Vec<SpectralRecord>> {
        let base = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let sidecars: Arc<dyn SidecarResolver> = Arc::new(FsSidecars::new(base));
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        read_inner(self.name(), path, &bytes, &sidecars, options)
    }

    fn read_bytes_with_sidecars(
        &self,
        name: &Path,
        bytes: &[u8],
        sidecars: &Arc<dyn SidecarResolver>,
        options: &ReadOptions,
    ) -> Result<Vec<SpectralRecord>> {
        read_inner(self.name(), name, bytes, sidecars, options)
    }
}

fn read_inner(
    reader_name: &'static str,
    name: &Path,
    bytes: &[u8],
    sidecars: &Arc<dyn SidecarResolver>,
    options: &ReadOptions,
) -> Result<Vec<SpectralRecord>> {
    let header = LanHeader::parse(bytes)?;
    if header.rows != SUPPORTED_ROWS
        || header.cols != SUPPORTED_COLS
        || header.bands != SUPPORTED_BANDS
    {
        return Err(Error::InvalidRecord(format!(
            "unsupported ERDAS LAN layout: rows={}, cols={}, bands={}; only AVIRIS 92AV3C 145x145x220 is supported",
            header.rows, header.cols, header.bands
        )));
    }

    let expected_len = HEADER_LEN + header.rows * header.cols * header.bands * 2;
    if bytes.len() != expected_len {
        return Err(Error::InvalidRecord(format!(
            "unsupported ERDAS LAN payload length: got {}, expected {expected_len}",
            bytes.len()
        )));
    }

    let base = name
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(""));
    let stem = name
        .file_name()
        .map(PathBuf::from)
        .ok_or_else(|| Error::InvalidRecord("ERDAS LAN primary has no file name".to_string()))?;
    let spc_rel = stem.with_extension("spc");
    let (axis, spc_source, axis_warnings) = read_spc_axis(sidecars, &base, &spc_rel, header.bands)?;
    let (labels, gis_source) = read_optional_gis_labels(sidecars, &base, header.rows, header.cols)?;
    let lan_source = SourceFile::from_bytes(name, bytes, "primary");
    let sources = if let Some(gis_source) = gis_source {
        vec![lan_source, spc_source, gis_source]
    } else {
        vec![lan_source, spc_source]
    };
    if options.single_record_cube {
        return read_single_cube_record(
            reader_name,
            &header,
            bytes,
            axis,
            &axis_warnings,
            labels.as_deref(),
            sources,
            options,
        );
    }

    let pixels = cube_pixels(options, header.rows, header.cols, "ERDAS LAN cube")?;

    let mut records = Vec::with_capacity(pixels.len());
    for (index, (row, col)) in pixels.into_iter().enumerate() {
        let values = read_bil_pixel_spectrum(bytes, &header, row, col)?;
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "sample_id".to_string(),
            json!(format!("pixel_y{row}_x{col}")),
        );
        metadata.insert("x_index".to_string(), json!(col));
        metadata.insert("y_index".to_string(), json!(row));
        metadata.insert("spatial_x".to_string(), json!(col));
        metadata.insert("spatial_y".to_string(), json!(row));
        metadata.insert("spatial_unit".to_string(), json!("pixel"));
        metadata.insert("rows".to_string(), json!(header.rows));
        metadata.insert("cols".to_string(), json!(header.cols));
        metadata.insert("bands".to_string(), json!(header.bands));
        metadata.insert("interleave".to_string(), json!("bil"));

        let mut targets = BTreeMap::new();
        if let Some(labels) = &labels {
            let label = labels[row * header.cols + col];
            targets.insert("land_cover_class".to_string(), json!(label));
        }

        // The `erdas_lan_aviris_experimental` warning is a format-wide
        // status flag (the reader is scoped to AVIRIS 92AV3C 145x145x220
        // and not yet generalised to other ERDAS LAN layouts). Emitting
        // it on every one of the 21,025 pixel records wastes ~200 KiB
        // of provenance overhead; downstream tools that filter by
        // `provenance.format == "erdas-lan-aviris"` get the same
        // signal. We keep it on the first record so callers iterating
        // a single record still see the flag.
        records.push(make_record(
            reader_name,
            sources.clone(),
            axis.clone(),
            values,
            metadata,
            targets,
            axis_warnings.clone(),
            index == 0,
        )?);
    }
    Ok(records)
}

#[derive(Clone, Copy)]
struct LanHeader {
    rows: usize,
    cols: usize,
    bands: usize,
}

impl LanHeader {
    fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_LEN || !bytes.starts_with(MAGIC) {
            return Err(Error::InvalidRecord(
                "ERDAS LAN header is missing HEAD74 magic".to_string(),
            ));
        }
        Ok(Self {
            bands: read_u32_le(bytes, 8)? as usize,
            rows: read_u32_le(bytes, 16)? as usize,
            cols: read_u32_le(bytes, 20)? as usize,
        })
    }
}

fn read_spc_axis(
    sidecars: &Arc<dyn SidecarResolver>,
    base: &Path,
    rel: &Path,
    expected_bands: usize,
) -> Result<(Vec<f64>, SourceFile, Vec<String>)> {
    let display = base.join(rel);
    let bytes = sidecars.read(rel)?;
    let source = SourceFile::from_bytes(&display, &bytes, "wavelength_sidecar");
    let text = String::from_utf8_lossy(&bytes);
    let axis = text
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("FILE")
                || trimmed.chars().all(|ch| ch == '-')
            {
                return None;
            }
            trimmed.split_whitespace().next().and_then(parse_number)
        })
        .collect::<Vec<_>>();
    if axis.len() != expected_bands {
        return Err(Error::InvalidRecord(format!(
            "ERDAS LAN SPC sidecar {} has {} wavelengths; expected {expected_bands}",
            display.display(),
            axis.len()
        )));
    }
    let warnings = if axis.windows(2).all(|pair| pair[0] < pair[1]) {
        Vec::new()
    } else {
        vec!["erdas_lan_spc_axis_non_monotonic_native_order".to_string()]
    };
    Ok((axis, source, warnings))
}

fn read_optional_gis_labels(
    sidecars: &Arc<dyn SidecarResolver>,
    base: &Path,
    rows: usize,
    cols: usize,
) -> Result<(Option<Vec<u8>>, Option<SourceFile>)> {
    let gis_rel = PathBuf::from("92AV3GT.GIS");
    if !sidecars.contains(&gis_rel) {
        return Ok((None, None));
    }
    let bytes = sidecars.read(&gis_rel)?;
    let display = base.join(&gis_rel);
    if bytes.len() != HEADER_LEN + rows * cols || !bytes.starts_with(MAGIC) {
        return Err(Error::InvalidRecord(format!(
            "ERDAS LAN GIS sidecar {} does not match the cube dimensions",
            display.display()
        )));
    }
    let source = SourceFile::from_bytes(&display, &bytes, "ground_truth_sidecar");
    Ok((Some(bytes[HEADER_LEN..].to_vec()), Some(source)))
}

fn read_bil_pixel_spectrum(
    bytes: &[u8],
    header: &LanHeader,
    row: usize,
    col: usize,
) -> Result<Vec<f64>> {
    let mut values = Vec::with_capacity(header.bands);
    for band in 0..header.bands {
        let offset = HEADER_LEN + ((row * header.bands + band) * header.cols + col) * 2;
        let raw = bytes.get(offset..offset + 2).ok_or_else(|| {
            Error::InvalidRecord("ERDAS LAN BIL pixel offset exceeds payload".to_string())
        })?;
        values.push(f64::from(u16::from_le_bytes([raw[0], raw[1]])));
    }
    Ok(values)
}

#[allow(clippy::too_many_arguments)]
fn make_record(
    reader: &str,
    sources: Vec<SourceFile>,
    axis_values: Vec<f64>,
    values: Vec<f64>,
    metadata: BTreeMap<String, serde_json::Value>,
    targets: BTreeMap<String, serde_json::Value>,
    mut warnings: Vec<String>,
    emit_experimental_warning: bool,
) -> Result<SpectralRecord> {
    let axis = SpectralAxis::new(axis_values, "nm", AxisKind::Wavelength)?;
    let signal = SpectralArray::new(
        axis,
        values,
        vec!["x".to_string()],
        SignalType::RawCounts,
        Some("dn".to_string()),
        "raw_counts",
        "file",
    )?;
    let mut signals = BTreeMap::new();
    signals.insert("raw_counts".to_string(), signal);
    if emit_experimental_warning {
        warnings.insert(0, "erdas_lan_aviris_experimental".to_string());
    }
    let record = SpectralRecord {
        signals,
        signal_type: SignalType::RawCounts,
        targets,
        metadata,
        provenance: Provenance {
            format: FORMAT.to_string(),
            reader: reader.to_string(),
            reader_version: env!("CARGO_PKG_VERSION").to_string(),
            sources,
            parsed_at_utc: None,
            record_schema_version: "0.2.0".to_string(),
            warnings,
        },
        quality_flags: Vec::new(),
    };
    record.validate()?;
    Ok(record)
}

/// Single N-dimensional cube record: `dims = ["row", "col", "x"]`,
/// `shape = [n_rows, n_cols, bands]`, values in C-order. Row/col coordinates
/// carry the absolute pixel indices (so a windowed read keeps its position).
/// The optional ground-truth label grid is preserved as a 2-D nested array in
/// `metadata.land_cover_class_grid` (it does not fit the scalar-per-record
/// `targets` shape — use the per-pixel layout for pixel-as-sample modelling).
#[allow(clippy::too_many_arguments)]
fn read_single_cube_record(
    reader: &str,
    header: &LanHeader,
    bytes: &[u8],
    axis_values: Vec<f64>,
    axis_warnings: &[String],
    labels: Option<&[u8]>,
    sources: Vec<SourceFile>,
    options: &ReadOptions,
) -> Result<Vec<SpectralRecord>> {
    let (row_range, col_range) = cube_region(options, header.rows, header.cols, "ERDAS LAN cube")?;
    let n_rows = row_range.len();
    let n_cols = col_range.len();

    let mut values = Vec::with_capacity(n_rows * n_cols * header.bands);
    let mut label_grid: Vec<Vec<u8>> = Vec::with_capacity(n_rows);
    for row in row_range.clone() {
        let mut label_row = Vec::with_capacity(n_cols);
        for col in col_range.clone() {
            values.extend(read_bil_pixel_spectrum(bytes, header, row, col)?);
            if let Some(labels) = labels {
                label_row.push(labels[row * header.cols + col]);
            }
        }
        if labels.is_some() {
            label_grid.push(label_row);
        }
    }

    let axis = SpectralAxis::new(axis_values, "nm", AxisKind::Wavelength)?;
    let row_coord = SpectralAxis::new(
        row_range.clone().map(|r| r as f64).collect(),
        "pixel",
        AxisKind::Index,
    )?;
    let col_coord = SpectralAxis::new(
        col_range.clone().map(|c| c as f64).collect(),
        "pixel",
        AxisKind::Index,
    )?;
    let mut coords = BTreeMap::new();
    coords.insert("row".to_string(), row_coord);
    coords.insert("col".to_string(), col_coord);
    let signal = SpectralArray::new_nd(
        vec![n_rows, n_cols, header.bands],
        vec!["row".to_string(), "col".to_string(), "x".to_string()],
        axis,
        coords,
        values,
        SignalType::RawCounts,
        Some("dn".to_string()),
        "raw_counts",
        "file",
    )?;
    let mut signals = BTreeMap::new();
    signals.insert("raw_counts".to_string(), signal);

    let mut metadata = BTreeMap::new();
    metadata.insert("rows".to_string(), json!(header.rows));
    metadata.insert("cols".to_string(), json!(header.cols));
    metadata.insert("bands".to_string(), json!(header.bands));
    metadata.insert("interleave".to_string(), json!("bil"));
    metadata.insert("spatial_unit".to_string(), json!("pixel"));
    if !label_grid.is_empty() {
        metadata.insert("land_cover_class_grid".to_string(), json!(label_grid));
    }

    let mut warnings = axis_warnings.to_vec();
    warnings.insert(0, "erdas_lan_aviris_experimental".to_string());
    let record = SpectralRecord {
        signals,
        signal_type: SignalType::RawCounts,
        targets: BTreeMap::new(),
        metadata,
        provenance: Provenance {
            format: FORMAT.to_string(),
            reader: reader.to_string(),
            reader_version: env!("CARGO_PKG_VERSION").to_string(),
            sources,
            parsed_at_utc: None,
            record_schema_version: "0.2.0".to_string(),
            warnings,
        },
        quality_flags: Vec::new(),
    };
    record.validate()?;
    Ok(vec![record])
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| Error::InvalidRecord("ERDAS LAN header is truncated".to_string()))?;
    Ok(u32::from_le_bytes(
        raw.try_into().expect("slice length checked"),
    ))
}
