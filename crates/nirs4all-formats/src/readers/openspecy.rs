//! OpenSpecy RDS reader.
//!
//! [OpenSpecy](https://openspecy.org) is an R toolkit for Raman and (FT)IR
//! spectroscopy of microplastics and environmental particles. It serialises its
//! spectra with R's `saveRDS()` (a gzip-compressed XDR stream). This reader
//! decodes that container with the pure-Rust [`rds2rust`] parser and maps the
//! canonical `OpenSpecy` object onto [`SpectralRecord`]s - never re-deriving the
//! numbers, only translating them.
//!
//! ## Canonical object (OpenSpecy >= 1.0)
//!
//! `as_OpenSpecy()` builds an S3-classed, three-element list:
//!
//! - `wavenumber` - a numeric vector of length `W` (the shared x-axis, cm^-1).
//! - `spectra` - a `data.table`/`data.frame` with **one column per spectrum**
//!   and `W` rows; column names are spectrum identifiers.
//! - `metadata` - a `data.table`/`data.frame` with **one row per spectrum**
//!   (`spectrum_type` = `ftir`/`raman`, `spectrum_identity`, `sample_name`, ...).
//!
//! Each spectra column becomes one record (a 1-D spectrum over the shared
//! wavenumber axis) with its metadata row attached.
//!
//! ## Robustness
//!
//! - The legacy single-spectrum form (a `data.frame` with `wavenumber` and
//!   `intensity` columns) is also accepted.
//! - When the container's `names`/`class` attributes are absent, the three parts
//!   are located structurally by shape, so the spectra still load.
//! - `metadata` is optional; records still load (keyed by their column name)
//!   when it is `NULL` or missing. Per-spectrum identities are not fabricated
//!   when the RDS object does not contain them.
//!
//! ## Limitations
//!
//! - Only the default `saveRDS()` containers are decoded: gzip-compressed and
//!   uncompressed XDR. `bzip2`/`xz`-compressed `.rds` files are not decoded.
//! - Intensity semantics are reported as [`SignalType::Unknown`] unless the
//!   metadata carries an `intensity_units` field that names them.

use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_formats_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralArray,
    SpectralAxis, SpectralRecord,
};
use rds2rust::{DataFrameData, Logical, ParseConfig, RObject, VectorData};
use serde_json::{json, Value};

use crate::readers::util::{provenance, safe_signal_name, signal_type_from_label};
use crate::Reader;

const FORMAT: &str = "openspecy-rds";
const WAVENUMBER_UNIT: &str = "cm-1";
const GZIP_MAGIC: &[u8] = b"\x1f\x8b";
const RDS_XDR_MAGIC: &[u8] = b"X\n";

/// Metadata columns whose value is the modelling label (the polymer / material
/// identity). Surfaced into `targets` in addition to the metadata row.
const TARGET_FIELDS: &[&str] = &["spectrum_identity", "material_class"];

pub struct OpenSpecyReader;

impl OpenSpecyReader {
    fn reader_name() -> &'static str {
        "nirs4all_formats::readers::openspecy"
    }
}

impl Reader for OpenSpecyReader {
    fn name(&self) -> &'static str {
        Self::reader_name()
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext != "rds" {
            return None;
        }
        // saveRDS() defaults to a gzip-compressed XDR stream; uncompressed XDR is
        // the other common container. Both decode through rds2rust. The OpenSpecy
        // shape itself is validated on read (the header alone cannot confirm it).
        if head.starts_with(GZIP_MAGIC) || head.starts_with(RDS_XDR_MAGIC) {
            return Some(FormatProbe::new(
                FORMAT,
                self.name(),
                Confidence::Likely,
                "R RDS container (.rds); OpenSpecy spectra validated on read",
            ));
        }
        None
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        read_openspecy(path, &bytes)
    }

    fn read_bytes(&self, name: &Path, bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
        read_openspecy(name, bytes)
    }
}

fn read_openspecy(name: &Path, bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
    let source = SourceFile::from_bytes(name, bytes, "primary");
    let container = if bytes.starts_with(GZIP_MAGIC) {
        "rds_gzip"
    } else if bytes.starts_with(RDS_XDR_MAGIC) {
        "rds_xdr"
    } else {
        "rds"
    };
    let object = rds2rust::read_rds_with_config(bytes, ParseConfig::large_data())
        .map_err(|error| Error::InvalidRecord(format!("OpenSpecy RDS parse error: {error}")))?
        .object
        // Resolve REFSXP back-references so the tree is plain owned data.
        .into_concrete_deep();

    records_from_openspecy(&object, source, container)
}

/// One spectra/metadata container, normalised across the S3, attributed-list,
/// and bare-list encodings rds2rust can produce.
struct Container<'a> {
    elements: Vec<&'a RObject>,
    names: Vec<Option<String>>,
    class: Vec<String>,
}

fn records_from_openspecy(
    object: &RObject,
    source: SourceFile,
    container: &str,
) -> Result<Vec<SpectralRecord>> {
    // Legacy single-spectrum form: a data.frame carrying wavenumber + intensity.
    if let RObject::DataFrame(df) = object {
        if let Some(records) = legacy_dataframe_records(df, &source, container)? {
            return Ok(records);
        }
    }

    let parsed = as_container(object).ok_or_else(|| {
        Error::InvalidRecord(
            "not an OpenSpecy RDS: top-level R object is neither an OpenSpecy list nor a spectra data.frame"
                .to_string(),
        )
    })?;

    let spectra_idx = resolve_spectra_index(&parsed)?;
    let spectra = dataframe(parsed.elements[spectra_idx]).expect("spectra index is a data.frame");
    let band_count = df_nrows(spectra);
    if band_count == 0 || spectra.columns.is_empty() {
        return Err(Error::InvalidRecord(
            "OpenSpecy spectra table is empty".to_string(),
        ));
    }

    let wavenumber = resolve_wavenumber(&parsed, band_count, spectra_idx)?;
    let metadata = resolve_metadata(&parsed, spectra.columns.len(), spectra_idx);

    let mut records = Vec::with_capacity(spectra.columns.len());
    for (spectrum_index, (column_name, column)) in spectra.columns.iter().enumerate() {
        let Some(intensities) = real_values(column) else {
            // Non-numeric column inside the spectra table: skip but keep going.
            continue;
        };
        if intensities.len() != band_count {
            return Err(Error::InvalidRecord(format!(
                "OpenSpecy spectrum '{column_name}' has {} points but the wavenumber axis has {band_count}",
                intensities.len()
            )));
        }

        let meta_row = metadata
            .map(|m| metadata_row(m, spectrum_index))
            .unwrap_or_default();
        let record = build_record(
            &wavenumber,
            intensities,
            column_name,
            spectrum_index,
            &parsed.class,
            &meta_row,
            source.clone(),
            container,
        )?;
        records.push(record);
    }

    if records.is_empty() {
        return Err(Error::InvalidRecord(
            "OpenSpecy spectra table contains no numeric spectra".to_string(),
        ));
    }
    Ok(records)
}

#[allow(clippy::too_many_arguments)]
fn build_record(
    wavenumber: &[f64],
    intensities: Vec<f64>,
    column_name: &str,
    spectrum_index: usize,
    class: &[String],
    meta_row: &BTreeMap<String, Value>,
    source: SourceFile,
    container: &str,
) -> Result<SpectralRecord> {
    let axis = SpectralAxis::new(wavenumber.to_vec(), WAVENUMBER_UNIT, AxisKind::Wavenumber)?;

    let signal_type = meta_row
        .get("intensity_units")
        .and_then(Value::as_str)
        .map(signal_type_from_label)
        .unwrap_or(SignalType::Unknown);

    let signal = SpectralArray::new(
        axis,
        intensities,
        vec!["x".to_string()],
        signal_type.clone(),
        None,
        "intensity",
        column_name.to_string(),
    )?;
    let mut signals = BTreeMap::new();
    signals.insert("intensity".to_string(), signal);

    let mut metadata: BTreeMap<String, Value> = BTreeMap::new();
    metadata.insert("format".to_string(), json!("openspecy"));
    metadata.insert("container".to_string(), json!(container));
    metadata.insert("spectrum_index".to_string(), json!(spectrum_index));
    metadata.insert("spectrum_column".to_string(), json!(column_name));
    if !class.is_empty() {
        metadata.insert("openspecy_class".to_string(), json!(class));
    }
    if let Some(modality) = modality_of(meta_row) {
        metadata.insert("modality".to_string(), json!(modality));
    }
    if !meta_row.is_empty() {
        metadata.insert("fields".to_string(), json!(meta_row));
    }

    let mut targets: BTreeMap<String, Value> = BTreeMap::new();
    for field in TARGET_FIELDS {
        if let Some(value) = meta_row.get(*field) {
            if !value.is_null() {
                targets.insert((*field).to_string(), value.clone());
            }
        }
    }

    let record = SpectralRecord {
        signals,
        signal_type,
        targets,
        metadata,
        provenance: provenance(FORMAT, OpenSpecyReader::reader_name(), source, Vec::new()),
        quality_flags: Vec::new(),
    };
    record.validate()?;
    Ok(record)
}

/// Map the OpenSpecy `spectrum_type` field onto a lowercase `ftir`/`raman`
/// modality tag.
fn modality_of(meta_row: &BTreeMap<String, Value>) -> Option<String> {
    let raw = meta_row.get("spectrum_type").and_then(Value::as_str)?;
    let lower = raw.trim().to_ascii_lowercase();
    match lower.as_str() {
        "" => None,
        "ftir" | "ir" | "infrared" | "ft-ir" => Some("ftir".to_string()),
        "raman" => Some("raman".to_string()),
        other => Some(other.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Container normalisation + part resolution
// ---------------------------------------------------------------------------

fn as_container(object: &RObject) -> Option<Container<'_>> {
    match object {
        RObject::S3Object(s3) => {
            let elements = list_elements(&s3.base)?;
            let names = names_from(attribute(&s3.attributes, "names"), elements.len());
            let class = s3.class.iter().map(|c| c.to_string()).collect();
            Some(Container {
                elements,
                names,
                class,
            })
        }
        RObject::WithAttributes { object, attributes } => {
            let elements = list_elements(object)?;
            let names = names_from(attribute(attributes, "names"), elements.len());
            let class = match attribute(attributes, "class") {
                Some(RObject::Character(VectorData::Owned(values))) => values
                    .iter()
                    .filter_map(|value| value.as_ref().map(ToString::to_string))
                    .collect(),
                _ => Vec::new(),
            };
            Some(Container {
                elements,
                names,
                class,
            })
        }
        RObject::List(items) => Some(Container {
            elements: items.iter().collect(),
            names: vec![None; items.len()],
            class: Vec::new(),
        }),
        _ => None,
    }
}

fn resolve_spectra_index(container: &Container<'_>) -> Result<usize> {
    // 1. By name.
    if let Some(index) = named_index(container, "spectra") {
        if dataframe(container.elements[index]).is_some() {
            return Ok(index);
        }
    }
    // 2. Structural: a data.frame whose row count equals the length of some
    //    numeric vector element (the wavenumber axis).
    let numeric_lengths: Vec<usize> = container
        .elements
        .iter()
        .filter_map(|element| real_values(element).map(|values| values.len()))
        .collect();
    let mut best: Option<(usize, usize)> = None; // (index, column_count)
    for (index, element) in container.elements.iter().enumerate() {
        let Some(df) = dataframe(element) else {
            continue;
        };
        let rows = df_nrows(df);
        let matches_axis = numeric_lengths.iter().any(|&len| len == rows && len > 0);
        if matches_axis {
            let cols = df.columns.len();
            if best.map(|(_, best_cols)| cols > best_cols).unwrap_or(true) {
                best = Some((index, cols));
            }
        }
    }
    if let Some((index, _)) = best {
        return Ok(index);
    }
    // 3. Last resort: the data.frame with the most columns.
    container
        .elements
        .iter()
        .enumerate()
        .filter_map(|(index, element)| dataframe(element).map(|df| (index, df.columns.len())))
        .max_by_key(|&(_, cols)| cols)
        .map(|(index, _)| index)
        .ok_or_else(|| {
            Error::InvalidRecord("OpenSpecy object has no spectra data.frame".to_string())
        })
}

fn resolve_wavenumber(
    container: &Container<'_>,
    band_count: usize,
    spectra_idx: usize,
) -> Result<Vec<f64>> {
    // Prefer the explicitly named axis, otherwise any numeric vector whose
    // length matches the spectra row count.
    let candidate = named_index(container, "wavenumber")
        .and_then(|index| real_values(container.elements[index]))
        .filter(|values| values.len() == band_count)
        .or_else(|| {
            container
                .elements
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != spectra_idx)
                .find_map(|(_, element)| {
                    real_values(element).filter(|values| values.len() == band_count)
                })
        });

    let values = candidate.ok_or_else(|| {
        Error::InvalidRecord(format!(
            "OpenSpecy object has no wavenumber axis of length {band_count}"
        ))
    })?;
    if values.iter().any(|value| !value.is_finite()) {
        return Err(Error::InvalidRecord(
            "OpenSpecy wavenumber axis contains non-finite values".to_string(),
        ));
    }
    Ok(values)
}

fn resolve_metadata<'a>(
    container: &Container<'a>,
    spectrum_count: usize,
    spectra_idx: usize,
) -> Option<&'a DataFrameData> {
    if let Some(index) = named_index(container, "metadata") {
        if let Some(df) = dataframe(container.elements[index]) {
            return Some(df);
        }
    }
    container
        .elements
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != spectra_idx)
        .find_map(|(_, element)| {
            dataframe(element).filter(|df| df_nrows(df) == spectrum_count && !df.columns.is_empty())
        })
}

// ---------------------------------------------------------------------------
// Legacy single-spectrum data.frame
// ---------------------------------------------------------------------------

fn legacy_dataframe_records(
    df: &DataFrameData,
    source: &SourceFile,
    container: &str,
) -> Result<Option<Vec<SpectralRecord>>> {
    let wavenumber = column_by_name(df, "wavenumber").and_then(real_values);
    let intensity = column_by_name(df, "intensity")
        .or_else(|| column_by_name(df, "spectra"))
        .or_else(|| column_by_name(df, "absorbance"))
        .and_then(real_values);

    let (Some(wavenumber), Some(intensity)) = (wavenumber, intensity) else {
        return Ok(None);
    };
    if wavenumber.len() != intensity.len() || wavenumber.is_empty() {
        return Ok(None);
    }
    if wavenumber.iter().any(|value| !value.is_finite()) {
        return Err(Error::InvalidRecord(
            "OpenSpecy wavenumber axis contains non-finite values".to_string(),
        ));
    }

    let meta_row = BTreeMap::new();
    let record = build_record(
        &wavenumber,
        intensity,
        "intensity",
        0,
        &[],
        &meta_row,
        source.clone(),
        container,
    )?;
    Ok(Some(vec![record]))
}

// ---------------------------------------------------------------------------
// rds2rust helpers
// ---------------------------------------------------------------------------

fn dataframe(object: &RObject) -> Option<&DataFrameData> {
    match object {
        RObject::DataFrame(df) => Some(df),
        _ => None,
    }
}

fn list_elements(object: &RObject) -> Option<Vec<&RObject>> {
    match object {
        RObject::List(items) => Some(items.iter().collect()),
        _ => None,
    }
}

fn attribute<'a>(attributes: &'a rds2rust::Attributes, key: &str) -> Option<&'a RObject> {
    attributes.get(key)
}

fn names_from(names: Option<&RObject>, len: usize) -> Vec<Option<String>> {
    match names {
        Some(RObject::Character(VectorData::Owned(values))) => {
            let mut out: Vec<Option<String>> = values
                .iter()
                .map(|value| {
                    value.as_ref().and_then(|value| {
                        let value = value.to_string();
                        (!value.is_empty()).then_some(value)
                    })
                })
                .collect();
            out.resize(len, None);
            out
        }
        _ => vec![None; len],
    }
}

fn named_index(container: &Container<'_>, name: &str) -> Option<usize> {
    container.names.iter().position(|candidate| {
        candidate
            .as_deref()
            .map(|value| value.eq_ignore_ascii_case(name))
            .unwrap_or(false)
    })
}

fn column_by_name<'a>(df: &'a DataFrameData, name: &str) -> Option<&'a RObject> {
    df.columns
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value)
}

/// Number of rows in a data.frame, taken as the longest column (robust to an
/// empty `row.names`).
fn df_nrows(df: &DataFrameData) -> usize {
    df.columns
        .values()
        .map(robject_len)
        .max()
        .unwrap_or(0)
        .max(df.row_names.len())
}

fn robject_len(object: &RObject) -> usize {
    match object {
        RObject::Real(VectorData::Owned(v)) => v.len(),
        RObject::Integer(VectorData::Owned(v)) => v.len(),
        RObject::Logical(VectorData::Owned(v)) => v.len(),
        RObject::Character(VectorData::Owned(v)) => v.len(),
        RObject::Factor(f) => f.values.len(),
        _ => 0,
    }
}

/// Materialise a numeric vector (`Real` directly, `Integer` widened, R `NA`
/// mapped to `NaN`).
fn real_values(object: &RObject) -> Option<Vec<f64>> {
    match object {
        RObject::Real(VectorData::Owned(values)) => Some(values.clone()),
        RObject::Integer(VectorData::Owned(values)) => Some(
            values
                .iter()
                .map(|&value| {
                    if value == i32::MIN {
                        f64::NAN
                    } else {
                        value as f64
                    }
                })
                .collect(),
        ),
        _ => None,
    }
}

/// Extract row `index` from a metadata data.frame as a JSON map keyed by
/// normalised column name.
fn metadata_row(df: &DataFrameData, index: usize) -> BTreeMap<String, Value> {
    let mut row = BTreeMap::new();
    for (column_name, column) in df.columns.iter() {
        let key = safe_signal_name(column_name, "field");
        row.insert(key, cell_value(column, index));
    }
    row
}

fn cell_value(column: &RObject, index: usize) -> Value {
    match column {
        RObject::Character(VectorData::Owned(values)) => values
            .get(index)
            .and_then(|value| value.as_ref())
            .map(|value| {
                let s = value.to_string();
                // R serialises NA character as the literal "NA"; treat empty as null.
                if s.is_empty() {
                    Value::Null
                } else {
                    json!(s)
                }
            })
            .unwrap_or(Value::Null),
        RObject::Real(VectorData::Owned(values)) => values
            .get(index)
            .map(|&value| {
                if value.is_finite() {
                    json!(value)
                } else {
                    Value::Null
                }
            })
            .unwrap_or(Value::Null),
        RObject::Integer(VectorData::Owned(values)) => values
            .get(index)
            .map(|&value| {
                if value == i32::MIN {
                    Value::Null
                } else {
                    json!(value)
                }
            })
            .unwrap_or(Value::Null),
        RObject::Logical(VectorData::Owned(values)) => match values.get(index) {
            Some(Logical::True) => json!(true),
            Some(Logical::False) => json!(false),
            _ => Value::Null,
        },
        RObject::Factor(factor) => factor
            .values
            .get(index)
            .and_then(|&code| {
                if code >= 1 {
                    factor.levels.get((code - 1) as usize)
                } else {
                    None
                }
            })
            .and_then(|level| level.as_ref())
            .map(|level| json!(level.to_string()))
            .unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use std::sync::Arc;

    fn real(values: Vec<f64>) -> RObject {
        RObject::Real(VectorData::Owned(values))
    }

    fn chr(values: &[&str]) -> RObject {
        RObject::Character(VectorData::Owned(
            values.iter().map(|s| Some(Arc::from(*s))).collect(),
        ))
    }

    fn row_names(n: usize) -> Vec<Option<Arc<str>>> {
        (1..=n)
            .map(|i| Some(Arc::from(i.to_string().as_str())))
            .collect()
    }

    fn frame(columns: Vec<(&str, RObject)>, rows: usize) -> RObject {
        let mut map: IndexMap<Arc<str>, RObject> = IndexMap::new();
        for (name, column) in columns {
            map.insert(Arc::from(name), column);
        }
        RObject::DataFrame(Box::new(DataFrameData {
            columns: map,
            row_names: row_names(rows),
        }))
    }

    fn canonical_object() -> RObject {
        let wavenumber = real(vec![650.0, 1000.0, 1500.0, 2000.0]);
        let spectra = frame(
            vec![
                ("ps_001", real(vec![0.1, 0.4, 0.8, 0.2])),
                ("pe_002", real(vec![0.0, 0.1, 0.9, 0.8])),
                ("pet_r03", real(vec![0.5, 0.3, 0.1, 0.6])),
            ],
            4,
        );
        let metadata = frame(
            vec![
                ("col_id", chr(&["ps_001", "pe_002", "pet_r03"])),
                ("spectrum_type", chr(&["ftir", "ftir", "raman"])),
                (
                    "spectrum_identity",
                    chr(&["Polystyrene", "Polyethylene", "PET"]),
                ),
            ],
            3,
        );
        let base = RObject::List(vec![wavenumber, spectra, metadata]);
        let mut attrs = rds2rust::Attributes::new();
        attrs.insert(
            Arc::from("names"),
            chr(&["wavenumber", "spectra", "metadata"]),
        );
        RObject::S3Object(Box::new(rds2rust::S3ObjectData {
            base: Box::new(base),
            class: vec![Arc::from("OpenSpecy")],
            attributes: attrs,
        }))
    }

    fn source() -> SourceFile {
        SourceFile::from_bytes(Path::new("x.rds"), b"x", "primary")
    }

    #[test]
    fn reads_canonical_object_with_metadata() {
        let records = records_from_openspecy(&canonical_object(), source(), "rds_gzip").unwrap();
        assert_eq!(records.len(), 3);

        let first = &records[0];
        let signal = first.signals.get("intensity").expect("intensity signal");
        assert_eq!(signal.axis.kind, AxisKind::Wavenumber);
        assert_eq!(signal.axis.unit, "cm-1");
        assert_eq!(signal.axis.values.len(), 4);
        assert_eq!(signal.values, vec![0.1, 0.4, 0.8, 0.2]);
        assert_eq!(first.metadata.get("modality").unwrap(), &json!("ftir"));
        assert_eq!(
            first.metadata.get("spectrum_column").unwrap(),
            &json!("ps_001")
        );
        assert_eq!(
            first.targets.get("spectrum_identity").unwrap(),
            &json!("Polystyrene")
        );

        assert_eq!(
            records[2].metadata.get("modality").unwrap(),
            &json!("raman")
        );
    }

    #[test]
    fn falls_back_to_structural_detection_without_names() {
        // Bare list [wavenumber, spectra] - no names, no class, no metadata.
        let wavenumber = real(vec![650.0, 1000.0, 1500.0, 2000.0]);
        let spectra = frame(
            vec![
                ("a", real(vec![0.1, 0.4, 0.8, 0.2])),
                ("b", real(vec![0.0, 0.1, 0.9, 0.8])),
            ],
            4,
        );
        let object = RObject::List(vec![wavenumber, spectra]);

        let records = records_from_openspecy(&object, source(), "rds_gzip").unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].signals["intensity"].axis.values.len(), 4);
        assert_eq!(
            records[0].metadata.get("spectrum_column").unwrap(),
            &json!("a")
        );
        assert!(!records[0].metadata.contains_key("fields"));
        assert_eq!(records[0].signal_type, SignalType::Unknown);
    }

    #[test]
    fn accepts_null_metadata_slot() {
        let wavenumber = real(vec![650.0, 1000.0, 1500.0]);
        let spectra = frame(
            vec![
                ("s1", real(vec![0.1, 0.4, 0.8])),
                ("s2", real(vec![0.0, 0.1, 0.9])),
            ],
            3,
        );
        let object = RObject::List(vec![wavenumber, spectra, RObject::Null]);

        let records = records_from_openspecy(&object, source(), "rds_gzip").unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(
            records[1].metadata.get("spectrum_column").unwrap(),
            &json!("s2")
        );
        assert!(!records[0].metadata.contains_key("fields"));
        assert!(records[0].targets.is_empty());
    }

    #[test]
    fn reads_legacy_single_spectrum_dataframe() {
        let df = frame(
            vec![
                ("wavenumber", real(vec![400.0, 800.0, 1200.0])),
                ("intensity", real(vec![0.2, 0.5, 0.9])),
            ],
            3,
        );
        let records = records_from_openspecy(&df, source(), "rds_gzip").unwrap();
        assert_eq!(records.len(), 1);
        let signal = &records[0].signals["intensity"];
        assert_eq!(signal.axis.values, vec![400.0, 800.0, 1200.0]);
        assert_eq!(signal.values, vec![0.2, 0.5, 0.9]);
    }

    #[test]
    fn rejects_non_openspecy_object() {
        let object = real(vec![1.0, 2.0, 3.0]);
        assert!(records_from_openspecy(&object, source(), "rds").is_err());
    }

    #[test]
    fn maps_intensity_units_to_signal_type() {
        let wavenumber = real(vec![650.0, 1000.0]);
        let spectra = frame(vec![("s1", real(vec![0.1, 0.2]))], 2);
        let metadata = frame(vec![("intensity_units", chr(&["absorbance"]))], 1);
        let object = RObject::List(vec![wavenumber, spectra, metadata]);
        let records = records_from_openspecy(&object, source(), "rds").unwrap();
        assert_eq!(records[0].signal_type, SignalType::Absorbance);
    }
}
