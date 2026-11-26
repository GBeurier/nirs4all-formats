//! FOSS NSAS `.da` spectra with an optional `.cn` constituent companion.
//!
//! Layout observed in NSAS and Vision files; see docs/formats/foss-nsas-da.md.

use std::collections::BTreeSet;

use nirs4all_formats_core::SpectralRecord;
use serde_json::Value;

use super::native::{
    grid_label, numeric_targets, require_absorbance_nm, short_labels, target_f32, Clock,
};
use super::requirements::Grid;
use super::select::{groups, report_record_losses, spectra, Spectrum, TargetSupport};
use super::{push_f32, refused, Context, WriteError, WrittenFile};

const FORMAT: &str = "foss-nsas-da";
const HEADER: usize = 128;
const CN_HEADER: usize = 384;
const CN_ROW: usize = 128;
const CONSTITUENTS: usize = 32;
const LABEL: usize = 10;

/// Spectral geometry with the acquisition words of a file that uses it.
struct Geometry {
    points: usize,
    start: u16,
    end: u16,
    words: &'static [(usize, u16)],
}

/// Words 20 (spacing), 22, 52 (amplifier) to 60 have partly unknown meaning;
/// they are those of an observed file of the same geometry.
const GEOMETRIES: &[Geometry] = &[
    Geometry {
        points: 700,
        start: 1100,
        end: 2500,
        words: &[
            (20, 1),
            (22, 2),
            (52, 1),
            (54, 1025),
            (56, 1),
            (58, 1),
            (60, 2),
        ],
    },
    Geometry {
        points: 1050,
        start: 400,
        end: 2500,
        words: &[
            (20, 1),
            (22, 2),
            (52, 1),
            (54, 1026),
            (56, 1),
            (58, 1),
            (60, 2),
        ],
    },
    Geometry {
        points: 1700,
        start: 800,
        end: 1650,
        words: &[
            (20, 11),
            (22, 2),
            (52, 2),
            (54, 65535),
            (58, 65535),
            (60, 65534),
            (96, 1),
        ],
    },
    Geometry {
        points: 4200,
        start: 400,
        end: 2500,
        words: &[(20, 11), (22, 2), (52, 1), (58, 1), (60, 2)],
    },
];

impl Geometry {
    fn at(&self, point: usize) -> f64 {
        let step = f64::from(self.end - self.start) / self.points as f64;
        f64::from(self.start) + step * point as f64
    }

    fn deviation(&self, axis: &[f64]) -> f64 {
        axis.iter()
            .enumerate()
            .map(|(point, x)| (self.at(point) - x).abs())
            .fold(0.0, f64::max)
    }

    fn padding(&self) -> usize {
        (HEADER + 4 * self.points).div_ceil(128) * 128 - HEADER - 4 * self.points
    }
}

/// Whether a measured axis is one of the known NSAS axes.
pub(crate) fn writable_axis(axis: &[f64], tolerance: f64) -> bool {
    let tolerance = tolerance.max(1e-9);
    GEOMETRIES
        .iter()
        .any(|geometry| axis.len() == geometry.points && geometry.deviation(axis) <= tolerance)
}

/// The known NSAS axes, as export requirements describe them.
pub(crate) fn grids() -> Vec<Grid> {
    GEOMETRIES
        .iter()
        .map(|geometry| {
            let (first, last) = (geometry.at(0), geometry.at(geometry.points - 1));
            Grid {
                id: format!("nsas-{}", geometry.points),
                label: format!("{} points, {first}-{last} nm", geometry.points),
                unit: "nm",
                first,
                step: geometry.at(1) - first,
                last,
                points: geometry.points,
            }
        })
        .collect()
}

pub(crate) fn write(
    records: &[SpectralRecord],
    context: &mut Context<'_>,
) -> Result<Vec<WrittenFile>, WriteError> {
    if context.extension.eq_ignore_ascii_case("cn") {
        return Err(refused(
            "'cn' is the extension of the NSAS constituent companion",
        ));
    }
    let spectra = spectra(records, context)?;
    report_record_losses(records, context, TargetSupport::Columns);
    require_absorbance_nm(&spectra, "NSAS", context)?;
    let groups = groups(&spectra);
    let mut files = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        let geometry = geometry(group[0], context)?;
        let columns = columns(group, context)?;
        let da = da_bytes(group, geometry, columns.len(), context)?;
        let label = group[0].signal_label.clone();
        let file = context.file(&[&label, &grid_label(groups.len(), index)], da);
        let companion = (!columns.is_empty()).then(|| WrittenFile {
            name: companion_name(&file.name),
            media_type: file.media_type,
            bytes: cn_bytes(group, &columns),
        });
        files.push(file);
        files.extend(companion);
    }
    Ok(files)
}

/// NSAS pairs the two files by stem.
fn companion_name(da: &str) -> String {
    let stem = da.rsplit_once('.').map_or(da, |(stem, _)| stem);
    format!("{stem}.cn")
}

fn geometry(
    spectrum: &Spectrum<'_>,
    context: &mut Context<'_>,
) -> Result<&'static Geometry, WriteError> {
    let axis = &spectrum.axis.values;
    let tolerance = context.options.axis_tolerance.max(1e-9);
    for geometry in GEOMETRIES {
        if axis.len() != geometry.points {
            continue;
        }
        let deviation = geometry.deviation(axis);
        if deviation > tolerance {
            continue;
        }
        if deviation > 1e-9 {
            context.losses.add_error(
                "axis_regularized",
                "axis coordinates are replaced by the NSAS axis within the requested tolerance",
                Some(deviation),
            );
        }
        return Ok(geometry);
    }
    Err(refused(format!(
        "known NSAS files hold 700 points (1100-2498 nm by 2), 1050 (400-2498 by 2), 1700 (800-1649.5 by 0.5) or 4200 (400-2499.5 by 0.5); spectrum '{}' has {} points from {} to {} and is never resampled",
        spectrum.id,
        axis.len(),
        axis[0],
        axis[axis.len() - 1]
    )))
}

struct Column {
    label: Vec<u8>,
    values: Vec<f32>,
}

/// Constituents every spectrum has a value for: no missing value is known.
fn columns(group: &[&Spectrum<'_>], context: &mut Context<'_>) -> Result<Vec<Column>, WriteError> {
    let mut names = numeric_targets(group, context);
    // NSAS sources keep the order and the raw labels of their own companion.
    let labels = source_labels(group[0]).filter(|labels| {
        let keys: BTreeSet<&str> = labels.iter().map(|(key, _)| key.as_str()).collect();
        labels.len() == names.len()
            && keys.into_iter().eq(names.iter().copied())
            && group
                .iter()
                .all(|spectrum| source_labels(spectrum).as_ref() == Some(labels))
    });
    if let Some(labels) = &labels {
        names.sort_by_key(|name| labels.iter().position(|(key, _)| key == name));
    }
    let mut kept = Vec::new();
    for name in names {
        let values: Option<Vec<f32>> = group
            .iter()
            .map(|spectrum| target_f32(spectrum, name, context))
            .collect::<Result<_, _>>()?;
        match values {
            Some(values) => kept.push((name, values)),
            None => context.losses.add(
                "incomplete_targets_omitted",
                format!("target '{name}' lacks values for some spectra, and NSAS has no known missing value"),
            ),
        }
    }
    if kept.len() > CONSTITUENTS {
        return Err(refused(format!(
            "NSAS files hold at most {CONSTITUENTS} constituents, found {}",
            kept.len()
        )));
    }
    let short = {
        let names: Vec<&str> = kept.iter().map(|(name, _)| *name).collect();
        let (short, shortened) = match labels {
            Some(_) => (Vec::new(), false),
            None => short_labels(&names, LABEL, context),
        };
        if shortened {
            context.losses.add(
                "target_names_shortened",
                "constituent names are shortened to 10 characters",
            );
        }
        short
    };
    Ok(kept
        .into_iter()
        .enumerate()
        .map(|(index, (name, values))| {
            let raw = labels
                .iter()
                .flatten()
                .find(|(key, _)| key == name)
                .map(|(_, raw)| raw.clone());
            Column {
                label: raw.unwrap_or_else(|| short[index].clone().into_bytes()),
                values,
            }
        })
        .collect())
}

/// Target keys and raw label bytes of an NSAS source.
fn source_labels(spectrum: &Spectrum<'_>) -> Option<Vec<(String, Vec<u8>)>> {
    let record = spectrum.record;
    if record.provenance.format != FORMAT {
        return None;
    }
    record
        .metadata
        .get("cn_constituents")?
        .as_array()?
        .iter()
        .map(|column| {
            let key = column.get("target_key")?.as_str()?.to_string();
            let raw = unhex(column.get("name_hex")?.as_str()?)?;
            (raw.len() == LABEL).then_some((key, raw))
        })
        .collect()
}

fn da_bytes(
    group: &[&Spectrum<'_>],
    geometry: &Geometry,
    constituents: usize,
    context: &mut Context<'_>,
) -> Result<Vec<u8>, WriteError> {
    let ids: Vec<&str> = group.iter().map(|spectrum| spectrum.id.as_str()).collect();
    let (labels, shortened) = short_labels(&ids, LABEL, context);
    if shortened {
        context.losses.add(
            "identifiers_shortened",
            "sample identifiers are shortened to 10 characters",
        );
    }
    let mut out = Vec::new();
    for (index, spectrum) in group.iter().enumerate() {
        let source = source_bytes(spectrum, "da_header_hex", HEADER)
            .filter(|header| header_matches(header, geometry));
        let mut header = match &source {
            Some(header) => updated_header(header, spectrum, &labels[index], context),
            None => new_header(spectrum, geometry, &labels[index], index, context)?,
        };
        put_u16(&mut header, 30, constituents as u16);
        out.extend_from_slice(&header);
        push_f32(
            &mut out,
            &spectrum.values,
            context,
            "ordinate_rounded_f32",
            "ordinates are stored as 32-bit floats",
        )?;
        let padding = source
            .and_then(|_| source_bytes(spectrum, "da_trailer_hex", geometry.padding()))
            .unwrap_or_else(|| vec![0; geometry.padding()]);
        out.extend_from_slice(&padding);
    }
    Ok(out)
}

fn header_matches(header: &[u8], geometry: &Geometry) -> bool {
    let word = |offset: usize| u16::from_le_bytes([header[offset], header[offset + 1]]);
    usize::from(word(14)) == geometry.points
        && word(16) == geometry.start
        && word(18) == geometry.end
}

/// Source header with the identifier and clock the record now has.
fn updated_header(
    header: &[u8],
    spectrum: &Spectrum<'_>,
    label: &str,
    context: &mut Context<'_>,
) -> Vec<u8> {
    let mut header = header.to_vec();
    let stored = String::from_utf8_lossy(&header[4..4 + LABEL]);
    if stored.trim_matches(['\0', ' ']) != spectrum.id.trim() {
        put_label(&mut header, label);
    }
    let words = std::array::from_fn(|word| {
        u16::from_le_bytes([header[64 + 2 * word], header[65 + 2 * word]])
    });
    let clock = Clock::of(spectrum);
    if clock != Clock::from_words(words) {
        put_clock(&mut header, clock.as_ref());
        if clock.is_none() {
            report_no_date(context);
        }
    }
    header
}

fn put_label(header: &mut [u8], label: &str) {
    header[4..4 + LABEL].fill(b' ');
    header[4..4 + label.len()].copy_from_slice(label.as_bytes());
}

fn put_clock(header: &mut [u8], clock: Option<&Clock>) {
    let words = clock.map_or([0; 6], Clock::words);
    for (word, value) in words.into_iter().enumerate() {
        put_u16(header, 64 + 2 * word, value);
    }
    put_u16(header, 78, words[2]);
}

fn report_no_date(context: &mut Context<'_>) {
    context.losses.add(
        "date_not_known",
        "spectra whose record holds no acquisition date have an empty date",
    );
}

fn new_header(
    spectrum: &Spectrum<'_>,
    geometry: &Geometry,
    label: &str,
    index: usize,
    context: &mut Context<'_>,
) -> Result<Vec<u8>, WriteError> {
    let number =
        u16::try_from(index + 1).map_err(|_| refused("NSAS files number at most 65535 spectra"))?;
    let mut header = vec![0u8; HEADER];
    let metadata = &spectrum.record.metadata;
    let model = match metadata.get("instrument_model").and_then(Value::as_str) {
        Some("NIRSystems 6500" | "NIRSYSTEMS 6500") => 6500,
        Some("NIRSystems 5000" | "NIRSYSTEMS 5000") => 5000,
        _ => 0,
    };
    put_u16(&mut header, 0, model);
    put_u16(&mut header, 2, number);
    put_label(&mut header, label);
    put_u16(&mut header, 14, geometry.points as u16);
    put_u16(&mut header, 16, geometry.start);
    put_u16(&mut header, 18, geometry.end);
    for (offset, word) in geometry.words {
        put_u16(&mut header, *offset, *word);
    }
    let clock = Clock::of(spectrum);
    put_clock(&mut header, clock.as_ref());
    if clock.is_none() {
        report_no_date(context);
    }
    let scans = metadata
        .get("scan_count")
        .and_then(Value::as_u64)
        .and_then(|scans| u16::try_from(scans).ok())
        .unwrap_or(1);
    put_u16(&mut header, 76, scans);
    header[92..96].copy_from_slice(&1f32.to_le_bytes());
    context.losses.add(
        "acquisition_words_defaulted",
        "amplifier and acquisition words are those of a reference file, transducer values are 0",
    );
    Ok(header)
}

fn cn_bytes(group: &[&Spectrum<'_>], columns: &[Column]) -> Vec<u8> {
    // Omitted columns must not survive in the source header and rows.
    let complete = source_labels(group[0]).is_some_and(|labels| labels.len() == columns.len());
    let header = source_bytes(group[0], "cn_header_hex", CN_HEADER).filter(|header| {
        complete
            && columns
                .iter()
                .enumerate()
                .all(|(index, column)| header[index * LABEL..][..LABEL] == column.label)
    });
    let reuse = header.is_some();
    let mut out = header.unwrap_or_else(|| {
        let mut header = vec![0u8; CN_HEADER];
        for (index, column) in columns.iter().enumerate() {
            let field = &mut header[index * LABEL..][..LABEL];
            field.fill(b' ');
            field[..column.label.len()].copy_from_slice(&column.label);
        }
        header
    });
    for (index, spectrum) in group.iter().enumerate() {
        let mut row = source_bytes(spectrum, "cn_record_hex", CN_ROW)
            .filter(|_| reuse)
            .unwrap_or_else(|| vec![0; CN_ROW]);
        for (slot, column) in columns.iter().enumerate() {
            row[slot * 4..slot * 4 + 4].copy_from_slice(&column.values[index].to_le_bytes());
        }
        out.extend_from_slice(&row);
    }
    out
}

/// Raw bytes the NSAS reader kept, for NSAS sources only.
fn source_bytes(spectrum: &Spectrum<'_>, key: &str, length: usize) -> Option<Vec<u8>> {
    let record = spectrum.record;
    if record.provenance.format != FORMAT {
        return None;
    }
    let bytes = unhex(record.metadata.get(key)?.as_str()?)?;
    (bytes.len() == length).then_some(bytes)
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.is_ascii() || !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).ok())
        .collect()
}

fn put_u16(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
