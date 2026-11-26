//! Export of normalized records to exchange formats.
//!
//! Every reader feeds every writer through [`SpectralRecord`]. Writers never
//! resample, convert units or transform ordinates: what a target cannot carry
//! is refused or reported as a [`WriteLoss`]. [`plan_conversion`] tells a host
//! which conversions (computed by nirs4all-methods) make records writable.

mod conversions;
mod jcamp;
mod json;
mod native;
mod nsas;
mod plan;
mod requirements;
mod select;
mod software;
mod spc;
mod table;
mod text;
mod winisi;
mod xlsx;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use nirs4all_formats_core::SpectralRecord;
use serde::{Deserialize, Serialize};

pub use conversions::{AppliedConversion, CONVERSIONS_KEY};
pub use plan::{
    plan_conversion, ConversionPlan, PlanChoice, PlanOptions, PlanQuestion, PlanRefusal,
    PlanStatus, PlanStep, PlannedSignal, SignalCandidate,
};
pub use requirements::{
    target_requirements, writer_requirements, EvenAxis, Grid, OtherGrids, TargetRequirements,
};
pub use software::{software_catalog, SoftwareProfile};

/// Error raised when records cannot be written to a target.
#[derive(Debug)]
pub enum WriteError {
    /// The identifier is neither a writer nor a software profile.
    UnknownTarget(String),
    /// The records or options cannot be represented in the target.
    Refused(String),
    /// A produced file could not be saved.
    Io { path: PathBuf, message: String },
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTarget(target) => write!(
                f,
                "unknown export target '{target}'; see writer_catalog() and software_catalog()"
            ),
            Self::Refused(message) => write!(f, "export refused: {message}"),
            Self::Io { path, message } => {
                write!(f, "cannot save {}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for WriteError {}

pub(crate) fn refused(message: impl Into<String>) -> WriteError {
    WriteError::Refused(message.into())
}

/// Export controls. Defaults write every signal of every record.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WriteOptions {
    /// Restrict the export to one signal name.
    pub signal: Option<String>,
    /// Stem of the produced file names (default: the source file stem).
    pub base_name: Option<String>,
    /// File extension override, without the dot.
    pub extension: Option<String>,
    /// Largest absolute axis deviation, in axis units, accepted when an
    /// almost evenly spaced axis is written as first/last/count. The default
    /// `0` keeps explicit coordinates for anything but rounding noise.
    pub axis_tolerance: f64,
    /// Group spectra sharing one axis into multi-spectrum files when the
    /// target supports it (`galactic-spc`).
    pub multi: bool,
    /// Column separator of delimited targets: `,`, `;` or a tab.
    pub delimiter: Option<char>,
    /// Return one ZIP archive instead of separate files.
    pub zip: bool,
}

/// One produced file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrittenFile {
    pub name: String,
    pub media_type: &'static str,
    pub bytes: Vec<u8>,
}

/// Information the target does not carry, or carries approximately.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WriteLoss {
    pub code: &'static str,
    pub message: String,
    /// Number of spectra (or records) concerned.
    pub count: usize,
    /// Largest absolute error, when the loss is numerical.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_error: Option<f64>,
}

/// Files and declared losses of one export.
#[derive(Clone, Debug)]
pub struct WriteOutput {
    /// Writer that produced the files.
    pub format: &'static str,
    pub files: Vec<WrittenFile>,
    pub losses: Vec<WriteLoss>,
    /// Conversions the exported signals went through, from record metadata.
    pub conversions: Vec<AppliedConversion>,
}

impl WriteOutput {
    /// Save every file into `directory`, which must exist.
    ///
    /// Without `overwrite`, nothing is written when any file already exists.
    pub fn save(
        &self,
        directory: impl AsRef<Path>,
        overwrite: bool,
    ) -> Result<Vec<PathBuf>, WriteError> {
        let paths: Vec<PathBuf> = self
            .files
            .iter()
            .map(|file| directory.as_ref().join(&file.name))
            .collect();
        if !overwrite {
            if let Some(path) = paths.iter().find(|path| path.exists()) {
                return Err(WriteError::Io {
                    path: path.clone(),
                    message: "file already exists".to_string(),
                });
            }
        }
        for (path, file) in paths.iter().zip(&self.files) {
            std::fs::write(path, &file.bytes).map_err(|error| WriteError::Io {
                path: path.clone(),
                message: error.to_string(),
            })?;
        }
        Ok(paths)
    }
}

/// Static description of a writer.
#[derive(Clone, Debug, Serialize)]
pub struct WriterCatalogEntry {
    pub format: &'static str,
    pub label: &'static str,
    pub extension: &'static str,
    pub media_type: &'static str,
    /// Whether one file can hold several spectra.
    pub multi_spectrum: bool,
    pub notes: &'static str,
}

const WRITERS: &[WriterCatalogEntry] = &[
    WriterCatalogEntry {
        format: "jcamp-dx",
        label: "JCAMP-DX 4.24",
        extension: "jdx",
        media_type: "chemical/x-jcamp-dx",
        multi_spectrum: false,
        notes: "One spectrum per file. Evenly spaced axes use XYDATA with integer ordinates, others XYPOINTS.",
    },
    WriterCatalogEntry {
        format: "galactic-spc",
        label: "Galactic / GRAMS SPC",
        extension: "spc",
        media_type: "application/octet-stream",
        multi_spectrum: true,
        notes: "Little-endian 0x4B, 32-bit float ordinates; explicit abscissae are 32-bit floats.",
    },
    WriterCatalogEntry {
        format: "csv",
        label: "Delimited matrix (CSV)",
        extension: "csv",
        media_type: "text/csv",
        multi_spectrum: true,
        notes: "One row per spectrum: identifier, targets, then one column per axis value.",
    },
    WriterCatalogEntry {
        format: "xlsx",
        label: "Excel workbook",
        extension: "xlsx",
        media_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        multi_spectrum: true,
        notes: "Sheet 'spectra' holds the matrix; sheet 'info' holds axis, signal and source descriptions.",
    },
    WriterCatalogEntry {
        format: "xy-text",
        label: "Two-column text",
        extension: "txt",
        media_type: "text/plain",
        multi_spectrum: false,
        notes: "One spectrum per file, abscissa and ordinate separated by a tab, no header.",
    },
    WriterCatalogEntry {
        format: "bruker-dpt",
        label: "OPUS data point table",
        extension: "dpt",
        media_type: "text/plain",
        multi_spectrum: false,
        notes: "One spectrum per file, comma separated; wavenumber axes only.",
    },
    WriterCatalogEntry {
        format: "foss-winisi-nir",
        label: "FOSS WinISI spectra (.nir)",
        extension: "nir",
        media_type: "application/octet-stream",
        multi_spectrum: true,
        notes: "Absorbance along nm, evenly spaced segments, 32-bit floats; sample numbers of 12 characters. Reverse-engineered layout.",
    },
    WriterCatalogEntry {
        format: "foss-winisi-cal",
        label: "FOSS WinISI spectra and constituents (.cal)",
        extension: "cal",
        media_type: "application/octet-stream",
        multi_spectrum: true,
        notes: "As foss-winisi-nir with up to 32 constituents; missing values are written as 0. Reverse-engineered layout.",
    },
    WriterCatalogEntry {
        format: "foss-nsas-da",
        label: "FOSS NSAS spectra (.da) and constituents (.cn)",
        extension: "da",
        media_type: "application/octet-stream",
        multi_spectrum: true,
        notes: "Absorbance on the four known NSAS axes, 32-bit floats; complete constituents go to a .cn file of the same name. Reverse-engineered layout.",
    },
    WriterCatalogEntry {
        format: "records-json",
        label: "nirs4all records (JSON)",
        extension: "json",
        media_type: "application/json",
        multi_spectrum: true,
        notes: "Complete normalized records; non-finite numbers become null.",
    },
];

/// Writers compiled into this build.
pub fn writer_catalog() -> &'static [WriterCatalogEntry] {
    WRITERS
}

/// Export listing shared by every binding: `formats`, `software` and the
/// `requirements` of each format.
pub fn export_targets() -> serde_json::Value {
    serde_json::json!({
        "formats": writer_catalog(),
        "software": software_catalog(),
        "requirements": writer_requirements(),
    })
}

/// Write records to `target`, a writer format or a software profile identifier.
pub fn write_records(
    records: &[SpectralRecord],
    target: &str,
    options: &WriteOptions,
) -> Result<WriteOutput, WriteError> {
    if records.is_empty() {
        return Err(refused("no records to write"));
    }
    if !options.axis_tolerance.is_finite() || options.axis_tolerance < 0.0 {
        return Err(refused("axis_tolerance must be a finite value >= 0"));
    }
    if !matches!(options.delimiter, None | Some(',' | ';' | '\t')) {
        return Err(refused("delimiter must be ',', ';' or a tab"));
    }
    if let Some(extension) = &options.extension {
        let valid = (1..=8).contains(&extension.len())
            && extension.bytes().all(|byte| byte.is_ascii_alphanumeric());
        if !valid {
            return Err(refused("extension must be 1 to 8 ASCII letters or digits"));
        }
    }

    let profile = software_catalog()
        .iter()
        .find(|profile| profile.id == target);
    let format = profile.map_or(target, |profile| profile.format);
    let entry = WRITERS
        .iter()
        .find(|entry| entry.format == format)
        .ok_or_else(|| WriteError::UnknownTarget(target.to_string()))?;
    let extension = options
        .extension
        .as_deref()
        .or(profile.map(|profile| profile.extension))
        .unwrap_or(entry.extension);

    let mut context = Context {
        options,
        extension,
        media_type: entry.media_type,
        default_base: default_base(records),
        names: FileNames::default(),
        losses: Losses::default(),
    };
    let applied = conversions::exported(records, options.signal.as_deref())?;
    let mut files = match entry.format {
        "jcamp-dx" => jcamp::write(records, &mut context)?,
        "galactic-spc" => spc::write(records, &mut context)?,
        "csv" => table::write_csv(records, &mut context)?,
        "xlsx" => table::write_xlsx(records, &mut context)?,
        "xy-text" => text::write_xy(records, &mut context)?,
        "bruker-dpt" => text::write_dpt(records, &mut context)?,
        "foss-winisi-nir" => winisi::write(records, false, &mut context)?,
        "foss-winisi-cal" => winisi::write(records, true, &mut context)?,
        "foss-nsas-da" => nsas::write(records, &mut context)?,
        "records-json" => json::write(records, &mut context)?,
        other => return Err(WriteError::UnknownTarget(other.to_string())),
    };
    if options.zip {
        files = vec![archive(&context.base_name(), &files)?];
    }
    let written = requirements::target_requirements(entry.format)
        .is_some_and(|requirements| requirements.records_conversions);
    if !applied.is_empty() && !written {
        context.losses.add(
            "conversions_not_written",
            "the conversions applied before export are not recorded in this format",
        );
    }
    Ok(WriteOutput {
        format: entry.format,
        files,
        losses: context.losses.finish(),
        conversions: applied,
    })
}

pub(crate) struct Context<'a> {
    pub options: &'a WriteOptions,
    pub extension: &'a str,
    pub media_type: &'static str,
    pub losses: Losses,
    default_base: String,
    names: FileNames,
}

impl Context<'_> {
    pub fn base_name(&self) -> String {
        file_stem(
            self.options
                .base_name
                .as_deref()
                .unwrap_or(&self.default_base),
        )
    }

    /// Build a uniquely named file from a list of name parts.
    pub fn file(&mut self, parts: &[&str], bytes: Vec<u8>) -> WrittenFile {
        let mut stem = self.base_name();
        for part in parts.iter().filter(|part| !part.is_empty()) {
            stem.push('_');
            stem.push_str(&file_stem(part));
        }
        WrittenFile {
            name: self.names.unique(&stem, self.extension),
            media_type: self.media_type,
            bytes,
        }
    }
}

/// Stem of the source shared by all records, or `spectra`.
fn default_base(records: &[SpectralRecord]) -> String {
    let mut stems = records.iter().map(source_stem);
    match stems.next().flatten() {
        Some(first) if stems.all(|stem| stem.as_ref() == Some(&first)) => first,
        _ => "spectra".to_string(),
    }
}

/// Source file name without its extension. Numeric extensions number
/// acquisitions (OPUS `a.0`, `a.1`) and stay in the stem.
pub(crate) fn source_stem(record: &SpectralRecord) -> Option<String> {
    let path = &record.provenance.sources.first()?.path;
    let name = path.rsplit(['/', '\\']).next()?;
    let stem = match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() && extension.parse::<u64>().is_err() => stem,
        _ => name,
    };
    (!stem.is_empty()).then(|| stem.to_string())
}

#[derive(Default)]
struct FileNames {
    used: BTreeSet<String>,
}

impl FileNames {
    /// Names are compared case-insensitively so archives unpack anywhere.
    fn unique(&mut self, stem: &str, extension: &str) -> String {
        let mut name = format!("{stem}.{extension}");
        let mut index = 2;
        while !self.used.insert(name.to_ascii_lowercase()) {
            name = format!("{stem}-{index}.{extension}");
            index += 1;
        }
        name
    }
}

/// Reduce arbitrary text to a portable file-name component.
fn file_stem(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars().take(80) {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches(|ch| matches!(ch, '.' | '_'));
    if trimmed.is_empty() {
        "x".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Losses merged by code and message.
#[derive(Default)]
pub(crate) struct Losses {
    entries: BTreeMap<(&'static str, String), (usize, Option<f64>)>,
}

impl Losses {
    pub fn add(&mut self, code: &'static str, message: impl Into<String>) {
        self.add_error(code, message, None);
    }

    pub fn add_error(
        &mut self,
        code: &'static str,
        message: impl Into<String>,
        error: Option<f64>,
    ) {
        let entry = self
            .entries
            .entry((code, message.into()))
            .or_insert((0, None));
        entry.0 += 1;
        entry.1 = match (entry.1, error) {
            (Some(left), Some(right)) => Some(left.max(right)),
            (left, right) => left.or(right),
        };
    }

    fn finish(self) -> Vec<WriteLoss> {
        self.entries
            .into_iter()
            .map(|((code, message), (count, max_error))| WriteLoss {
                code,
                message,
                count,
                max_error,
            })
            .collect()
    }
}

/// Shortest decimal text that parses back to exactly the same number.
///
/// Exponents carry a sign and at least two digits, as JCAMP-DX AFFN requires.
pub(crate) fn number(value: f64) -> String {
    let magnitude = value.abs();
    if magnitude == 0.0 || (1e-5..1e15).contains(&magnitude) {
        return format!("{value}");
    }
    let text = format!("{value:E}");
    let (mantissa, exponent) = text.split_once('E').expect("exponent form");
    let exponent: i32 = exponent.parse().expect("integer exponent");
    format!("{mantissa}E{}", exponent_text(exponent))
}

pub(crate) fn exponent_text(exponent: i32) -> String {
    let sign = if exponent < 0 { '-' } else { '+' };
    format!("{sign}{:02}", exponent.unsigned_abs())
}

/// Append little-endian 32-bit floats and declare the rounding.
pub(crate) fn push_f32(
    out: &mut Vec<u8>,
    values: &[f64],
    context: &mut Context<'_>,
    code: &'static str,
    message: &str,
) -> Result<(), WriteError> {
    let mut error = 0.0f64;
    for value in values {
        let stored = *value as f32;
        if !stored.is_finite() {
            return Err(refused(format!("{value} exceeds the 32-bit float range")));
        }
        error = error.max((f64::from(stored) - value).abs());
        out.extend_from_slice(&stored.to_le_bytes());
    }
    if error > 0.0 {
        context.losses.add_error(code, message, Some(error));
    }
    Ok(())
}

/// Plain single-line ASCII text; legacy binary fields have no declared encoding.
pub(crate) fn ascii_text(text: &str, context: &mut Context<'_>) -> String {
    let clean: String = text
        .chars()
        .map(|ch| if (' '..='~').contains(&ch) { ch } else { '_' })
        .collect();
    if clean != text {
        context.losses.add(
            "text_sanitized",
            "characters a label cannot hold are replaced by '_'",
        );
    }
    clean
}

fn archive(base_name: &str, files: &[WrittenFile]) -> Result<WrittenFile, WriteError> {
    let bytes = zip_bytes(
        files
            .iter()
            .map(|file| (file.name.as_str(), file.bytes.as_slice())),
    )?;
    Ok(WrittenFile {
        name: format!("{base_name}.zip"),
        media_type: "application/zip",
        bytes,
    })
}

/// Deterministic deflate archive (fixed timestamps).
pub(crate) fn zip_bytes<'a>(
    entries: impl Iterator<Item = (&'a str, &'a [u8])>,
) -> Result<Vec<u8>, WriteError> {
    let failed = |error: &dyn fmt::Display| refused(format!("ZIP archive: {error}"));
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());
    for (name, bytes) in entries {
        writer
            .start_file(name, options)
            .map_err(|error| failed(&error))?;
        writer.write_all(bytes).map_err(|error| failed(&error))?;
    }
    Ok(writer
        .finish()
        .map_err(|error| failed(&error))?
        .into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_round_trip() {
        assert_eq!(number(2e16), "2E+16");
        assert_eq!(number(-1.5e-7), "-1.5E-07");
        for value in [
            0.1,
            1.0 / 3.0,
            1e-30,
            -2.5e18,
            0.0,
            12345.678,
            f64::MIN_POSITIVE,
        ] {
            assert_eq!(number(value).parse::<f64>().unwrap(), value);
        }
    }

    #[test]
    fn file_names_are_portable_and_unique() {
        let mut names = FileNames::default();
        assert_eq!(file_stem("../a b/é:c"), "a_b_c");
        assert_eq!(names.unique("A", "spc"), "A.spc");
        assert_eq!(names.unique("a", "spc"), "a-2.spc");
    }
}
