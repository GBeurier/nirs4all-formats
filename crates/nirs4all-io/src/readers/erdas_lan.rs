use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Provenance, Result, SignalType, SourceFile,
    SpectralArray, SpectralAxis, SpectralRecord,
};
use serde_json::json;

use crate::readers::util::parse_number;
use crate::registry::{cube_pixels, ReadOptions};
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
        "nirs4all_io::readers::erdas_lan"
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
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let header = LanHeader::parse(&bytes)?;
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

        let spc_path = path.with_extension("spc");
        let (axis, spc_source, axis_warnings) = read_spc_axis(&spc_path, header.bands)?;
        let (labels, gis_source) = read_optional_gis_labels(path, header.rows, header.cols)?;
        let lan_source = SourceFile::from_bytes(path, &bytes, "primary");
        let sources = if let Some(gis_source) = gis_source {
            vec![lan_source, spc_source, gis_source]
        } else {
            vec![lan_source, spc_source]
        };
        let pixels = cube_pixels(options, header.rows, header.cols, "ERDAS LAN cube")?;

        let mut records = Vec::with_capacity(pixels.len());
        for (row, col) in pixels {
            let values = read_bil_pixel_spectrum(&bytes, &header, row, col)?;
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

            records.push(make_record(
                self.name(),
                sources.clone(),
                axis.clone(),
                values,
                metadata,
                targets,
                axis_warnings.clone(),
            )?);
        }
        Ok(records)
    }
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
    path: &Path,
    expected_bands: usize,
) -> Result<(Vec<f64>, SourceFile, Vec<String>)> {
    let bytes = std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let source = SourceFile::from_bytes(path, &bytes, "wavelength_sidecar");
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
            path.display(),
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
    cube_path: &Path,
    rows: usize,
    cols: usize,
) -> Result<(Option<Vec<u8>>, Option<SourceFile>)> {
    let gis_path = gis_sidecar_path(cube_path);
    if !gis_path.exists() {
        return Ok((None, None));
    }
    let bytes = std::fs::read(&gis_path).map_err(|source| Error::Io {
        path: gis_path.clone(),
        source,
    })?;
    if bytes.len() != HEADER_LEN + rows * cols || !bytes.starts_with(MAGIC) {
        return Err(Error::InvalidRecord(format!(
            "ERDAS LAN GIS sidecar {} does not match the cube dimensions",
            gis_path.display()
        )));
    }
    let source = SourceFile::from_bytes(&gis_path, &bytes, "ground_truth_sidecar");
    Ok((Some(bytes[HEADER_LEN..].to_vec()), Some(source)))
}

fn gis_sidecar_path(cube_path: &Path) -> PathBuf {
    let mut path = cube_path.to_path_buf();
    path.set_file_name("92AV3GT.GIS");
    path
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

fn make_record(
    reader: &str,
    sources: Vec<SourceFile>,
    axis_values: Vec<f64>,
    values: Vec<f64>,
    metadata: BTreeMap<String, serde_json::Value>,
    targets: BTreeMap<String, serde_json::Value>,
    mut warnings: Vec<String>,
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
    warnings.insert(0, "erdas_lan_aviris_experimental".to_string());
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
            record_schema_version: "0.1.0".to_string(),
            warnings,
        },
        quality_flags: Vec::new(),
    };
    record.validate()?;
    Ok(record)
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| Error::InvalidRecord("ERDAS LAN header is truncated".to_string()))?;
    Ok(u32::from_le_bytes(
        raw.try_into().expect("slice length checked"),
    ))
}
