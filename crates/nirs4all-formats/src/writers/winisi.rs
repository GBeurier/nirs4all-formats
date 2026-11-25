//! FOSS WinISI / ISIscan `.nir` (spectra) and `.cal` (spectra and constituents).
//!
//! Layout observed in ISIscan and WinISI files; see docs/formats/foss-winisi.md.

use nirs4all_formats_core::SpectralRecord;
use serde_json::Value;

use super::native::{
    grid_label, numeric_targets, require_absorbance_nm, short_labels, target_f32, Clock,
};
use super::select::{groups, report_record_losses, spectra, Spectrum, TargetSupport};
use super::{ascii_text, push_f32, refused, Context, WriteError, WrittenFile};

const HEADER: usize = 0x380;
const SAMPLE_HEADER: usize = 0x100;
const BLOCK: usize = 128;
const CONSTITUENTS: usize = 32;
const SEGMENTS: usize = 7;
const SAMPLE_NUMBER: usize = 12;
const CONSTITUENT_NAME: usize = 15;
/// DOS date 1980-01-01, the format's epoch.
const NO_DATE: u16 = 0x0021;

pub(crate) fn write(
    records: &[SpectralRecord],
    with_constituents: bool,
    context: &mut Context<'_>,
) -> Result<Vec<WrittenFile>, WriteError> {
    let spectra = spectra(records, context)?;
    let support = if with_constituents {
        TargetSupport::Columns
    } else {
        TargetSupport::None
    };
    report_record_losses(records, context, support);
    require_absorbance_nm(&spectra, "WinISI", context)?;
    let groups = groups(&spectra);
    let mut files = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        let names = if with_constituents {
            numeric_targets(group, context)
        } else {
            Vec::new()
        };
        if with_constituents && names.is_empty() {
            return Err(refused(
                "no numeric target value to write in a WinISI .cal file; use 'foss-winisi-nir'",
            ));
        }
        let bytes = file_bytes(group, &names, with_constituents, context)?;
        let label = group[0].signal_label.clone();
        files.push(context.file(&[&label, &grid_label(groups.len(), index)], bytes));
    }
    Ok(files)
}

fn file_bytes(
    group: &[&Spectrum<'_>],
    names: &[&str],
    with_constituents: bool,
    context: &mut Context<'_>,
) -> Result<Vec<u8>, WriteError> {
    let points = u16::try_from(group[0].axis.values.len())
        .map_err(|_| refused("WinISI files hold at most 65535 points per spectrum"))?;
    let samples = u16::try_from(group.len())
        .map_err(|_| refused("WinISI files hold at most 65535 spectra"))?;
    if names.len() > CONSTITUENTS {
        return Err(refused(format!(
            "WinISI files hold at most {CONSTITUENTS} constituents, found {}",
            names.len()
        )));
    }
    if !matches!(points, 700 | 1050) {
        context.losses.add(
            "point_count_not_observed",
            "known WinISI files hold 700 or 1050 points per spectrum; other sizes may not open",
        );
    }
    let instrument = Instrument::of(group);
    let clocks: Vec<Option<Clock>> = group.iter().map(|spectrum| Clock::of(spectrum)).collect();
    let dates: Vec<u16> = clocks
        .iter()
        .map(|clock| clock.as_ref().and_then(Clock::dos_date).unwrap_or(NO_DATE))
        .collect();
    let times: Vec<u32> = group.iter().map(|spectrum| unix_time(spectrum)).collect();
    if dates.contains(&NO_DATE) {
        context.losses.add(
            "date_not_known",
            "spectra without an acquisition date are dated 1980-01-01",
        );
    }

    let mut out = vec![0u8; HEADER];
    put_u16(&mut out, 0x00, if with_constituents { 2 } else { 1 });
    put_u16(&mut out, 0x02, samples);
    put_u16(&mut out, 0x06, points);
    put_u16(&mut out, 0x08, names.len() as u16);
    put_u16(&mut out, 0x0A, *dates.iter().max().unwrap_or(&NO_DATE));
    put_u32(&mut out, 0x0C, *times.iter().max().unwrap_or(&0));
    let file_id = format!("nirs4all-formats {}", env!("CARGO_PKG_VERSION"));
    put_text(&mut out, 0x12, 71, &file_id);
    if instrument.native {
        put_text(&mut out, 0x59, 9, &instrument.serial);
    }
    if with_constituents {
        // 1 = as received, 0 = dry matter.
        put_u16(&mut out, 0x62, 1);
        context.losses.add(
            "moisture_basis_assumed",
            "constituents are declared on an 'as received' basis",
        );
    }
    put_u16(&mut out, 0x80, instrument.code);
    put_text(&mut out, 0x82, 21, &instrument.model);
    put_text(&mut out, 0x97, 9, &instrument.serial);
    put_segments(&mut out, group[0], instrument.native, context)?;

    let (labels, shortened) = short_labels(names, CONSTITUENT_NAME, context);
    if shortened {
        context.losses.add(
            "target_names_shortened",
            "constituent names are shortened to 15 characters",
        );
    }
    for (index, label) in labels.iter().enumerate() {
        put_text(&mut out, 0x180 + index * 16, 16, label);
    }

    let ids: Vec<&str> = group.iter().map(|spectrum| spectrum.id.as_str()).collect();
    let (numbers, shortened) = short_labels(&ids, SAMPLE_NUMBER, context);
    if shortened {
        context.losses.add(
            "identifiers_shortened",
            "sample numbers are shortened to 12 characters; the sample ID field keeps up to 49",
        );
    }
    for (index, spectrum) in group.iter().enumerate() {
        let start = out.len();
        out.resize(start + SAMPLE_HEADER, 0);
        let header = &mut out[start..];
        put_index(header, &numbers[index], index);
        put_u16(header, 16, dates[index]);
        put_u16(header, 18, product_code(spectrum));
        if numbers[index] != spectrum.id {
            let id = ascii_text(&spectrum.id, context);
            put_text(header, 29, 50, &id);
        }
        put_u32(header, 214, times[index]);
        push_f32(
            &mut out,
            &spectrum.values,
            context,
            "ordinate_rounded_f32",
            "ordinates are stored as 32-bit floats",
        )?;
        out.resize(start + (out.len() - start).div_ceil(BLOCK) * BLOCK, 0);
        let mut values = [0f32; CONSTITUENTS];
        for (value, name) in values.iter_mut().zip(names) {
            match target_f32(spectrum, name, context)? {
                Some(0.0) => context.losses.add(
                    "zero_targets_read_as_missing",
                    "the format uses 0 for missing constituents, so reference values of 0 read as missing",
                ),
                Some(target) => *value = target,
                None => context.losses.add(
                    "missing_targets_as_zero",
                    "missing constituents are written as 0, the format's missing value",
                ),
            }
        }
        out.extend(values.iter().flat_map(|value| value.to_le_bytes()));
    }
    for (index, number) in numbers.iter().enumerate() {
        let mut entry = [0u8; 16];
        put_index(&mut entry, number, index);
        out.extend_from_slice(&entry);
    }
    Ok(out)
}

/// Sample number and 1-based sequence, common to sample headers and the index.
fn put_index(bytes: &mut [u8], number: &str, index: usize) {
    put_text(bytes, 0, 13, number);
    put_u16(bytes, 13, (index + 1) as u16);
}

struct Instrument {
    code: u16,
    model: String,
    serial: String,
    /// FOSS instruments store visible and near-infrared detectors as segments.
    native: bool,
}

impl Instrument {
    /// WinISI names instruments it does not know "Unknown" with type 11.
    fn of(group: &[&Spectrum<'_>]) -> Self {
        let common = |key: &str| {
            let mut values = group.iter().map(|spectrum| {
                spectrum
                    .record
                    .metadata
                    .get(key)
                    .and_then(Value::as_str)
                    .filter(|text| text.is_ascii())
            });
            let first = values.next().flatten()?;
            values.all(|value| value == Some(first)).then_some(first)
        };
        let foss = group
            .iter()
            .all(|spectrum| spectrum.record.provenance.format.starts_with("foss-"));
        let model = common("instrument_model").filter(|model| foss && model.len() <= 20);
        let code = match model {
            Some("NIRSystems 6500") => 9,
            Some("NIRSystems 5000") => 11,
            Some(model) if model.starts_with("NIRS DS") => 29,
            _ => {
                return Self {
                    code: 11,
                    model: "Unknown".to_string(),
                    serial: "00000000".to_string(),
                    native: false,
                }
            }
        };
        Self {
            code,
            model: model.unwrap_or_default().to_string(),
            serial: common("master_number")
                .filter(|serial| serial.len() <= 8)
                .unwrap_or("00000000")
                .to_string(),
            native: true,
        }
    }
}

fn put_segments(
    out: &mut [u8],
    spectrum: &Spectrum<'_>,
    native: bool,
    context: &mut Context<'_>,
) -> Result<(), WriteError> {
    let axis = &spectrum.axis.values;
    let segments = segments(axis, native, context.options.axis_tolerance)?;
    put_u16(out, 0xA0, segments.len() as u16);
    put_u16(out, 0xCA, 1);
    let mut deviation = 0.0f64;
    let mut first = 0;
    for (index, segment) in segments.iter().enumerate() {
        put_u16(out, 0xA2 + index * 2, segment.points as u16);
        for (offset, value) in [
            (0xCC, segment.start),
            (0xE8, segment.step),
            (0x104, segment.end),
        ] {
            out[offset + index * 4..offset + index * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (point, x) in axis[first..first + segment.points].iter().enumerate() {
            deviation = deviation.max((segment.at(point) - x).abs());
        }
        first += segment.points;
    }
    if deviation > rounding(axis) {
        context.losses.add_error(
            "axis_regularized",
            "axis coordinates are replaced by evenly spaced segments within the requested tolerance",
            Some(deviation),
        );
    } else if deviation > 0.0 {
        context.losses.add_error(
            "axis_rounded_f32",
            "axis segments are stored as 32-bit floats",
            Some(deviation),
        );
    }
    Ok(())
}

struct Segment {
    points: usize,
    start: f32,
    step: f32,
    end: f32,
}

impl Segment {
    fn of(axis: &[f64]) -> Self {
        let last = axis[axis.len() - 1];
        let step = if axis.len() > 1 {
            (last - axis[0]) / (axis.len() - 1) as f64
        } else {
            1.0
        };
        Self {
            points: axis.len(),
            start: axis[0] as f32,
            step: step as f32,
            end: last as f32,
        }
    }

    fn at(&self, point: usize) -> f64 {
        f64::from(self.start) + f64::from(self.step) * point as f64
    }

    fn matches(&self, axis: &[f64], tolerance: f64) -> bool {
        axis.iter()
            .enumerate()
            .all(|(point, x)| (self.at(point) - x).abs() <= tolerance)
    }
}

/// Axis error of storing start and step as 32-bit floats.
fn rounding(axis: &[f64]) -> f64 {
    2.0 * f64::from(f32::EPSILON) * axis[axis.len() - 1].abs()
}

fn segments(axis: &[f64], native: bool, tolerance: f64) -> Result<Vec<Segment>, WriteError> {
    if axis[0] <= 0.0 || axis.windows(2).any(|pair| pair[1] <= pair[0]) {
        return Err(refused(
            "WinISI axes are positive and strictly increasing; spectra are never reordered",
        ));
    }
    let tolerance = tolerance.max(rounding(axis));
    // 400-1098 and 1100-2498 nm by 2 nm: two detectors, one even axis.
    let detectors = native
        && axis.len() == 1050
        && axis
            .iter()
            .enumerate()
            .all(|(point, x)| *x == 400.0 + 2.0 * point as f64);
    if detectors {
        return Ok(vec![Segment::of(&axis[..350]), Segment::of(&axis[350..])]);
    }
    let mut out = Vec::new();
    let mut rest = axis;
    while !rest.is_empty() {
        let whole = Segment::of(rest);
        if whole.matches(rest, tolerance) {
            out.push(whole);
            break;
        }
        // Longest run keeping the first gap, then checked as a segment.
        let gap = rest[1] - rest[0];
        let run = 1 + rest
            .windows(2)
            .take_while(|pair| (pair[1] - pair[0] - gap).abs() <= 2.0 * tolerance)
            .count();
        let segment = Segment::of(&rest[..run]);
        if out.len() + 1 == SEGMENTS || !segment.matches(&rest[..run], tolerance) {
            return Err(refused(format!(
                "WinISI axes hold at most {SEGMENTS} evenly spaced segments and spectra are never resampled; this axis deviates from one even segment by up to {:.3} nm, which axis_tolerance can accept",
                whole_deviation(axis)
            )));
        }
        out.push(segment);
        rest = &rest[run..];
    }
    Ok(out)
}

/// Largest distance to an evenly spaced axis with the same limits, rounded up.
fn whole_deviation(axis: &[f64]) -> f64 {
    let segment = Segment::of(axis);
    let deviation = axis
        .iter()
        .enumerate()
        .map(|(point, x)| (segment.at(point) - x).abs())
        .fold(0.0, f64::max);
    (deviation * 1e3).ceil() / 1e3
}

fn unix_time(spectrum: &Spectrum<'_>) -> u32 {
    spectrum
        .record
        .metadata
        .get("acquired_unix_time")
        .and_then(Value::as_u64)
        .and_then(|time| u32::try_from(time).ok())
        .unwrap_or(0)
}

fn product_code(spectrum: &Spectrum<'_>) -> u16 {
    spectrum
        .record
        .metadata
        .get("product_code")
        .and_then(Value::as_u64)
        .and_then(|code| u16::try_from(code).ok())
        .unwrap_or(0)
}

fn put_u16(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(out: &mut [u8], offset: usize, value: u32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

/// NUL-terminated text.
fn put_text(out: &mut [u8], offset: usize, width: usize, text: &str) {
    let length = text.len().min(width - 1);
    out[offset..offset + length].copy_from_slice(&text.as_bytes()[..length]);
}
