//! Selection of one-dimensional spectra out of records.

use std::borrow::Cow;
use std::collections::BTreeMap;

use nirs4all_formats_core::{SignalType, SpectralArray, SpectralAxis, SpectralRecord};
use serde_json::Value;

use super::{refused, Context, WriteError};

/// One spectrum to write.
pub(crate) struct Spectrum<'a> {
    pub id: String,
    /// Signal suffix for file names; empty when the export holds one signal.
    pub signal_label: String,
    pub signal_name: &'a str,
    pub signal: &'a SpectralArray,
    pub axis: &'a SpectralAxis,
    pub values: Cow<'a, [f64]>,
    pub record: &'a SpectralRecord,
}

impl Spectrum<'_> {
    pub fn percent(&self) -> bool {
        self.signal
            .unit
            .as_deref()
            .is_some_and(|unit| unit.contains('%') || unit.eq_ignore_ascii_case("percent"))
    }

    pub fn type_label(&self) -> &'static str {
        signal_label(&self.signal.signal_type)
    }

    /// Refuse values a target without missing-value support cannot store.
    pub fn require_finite(&self, target: &str) -> Result<(), WriteError> {
        match self.values.iter().position(|value| !value.is_finite()) {
            Some(index) => Err(refused(format!(
                "spectrum '{}' signal '{}' has a missing or non-finite value at point {index}, which {target} cannot store",
                self.id, self.signal_name
            ))),
            None => Ok(()),
        }
    }
}

/// Collect every requested spectrum, flattening N-dimensional signals.
pub(crate) fn spectra<'a>(
    records: &'a [SpectralRecord],
    context: &mut Context<'_>,
) -> Result<Vec<Spectrum<'a>>, WriteError> {
    let wanted = context.options.signal.as_deref();
    let mut names: Vec<&str> = Vec::new();
    for record in records {
        for name in record.signals.keys() {
            if wanted.is_none_or(|wanted| wanted == name) && !names.contains(&name.as_str()) {
                names.push(name);
            }
        }
    }
    if names.is_empty() {
        let available: std::collections::BTreeSet<&String> = records
            .iter()
            .flat_map(|record| record.signals.keys())
            .collect();
        return Err(refused(match wanted {
            Some(wanted) => format!("no record has signal '{wanted}'; available: {available:?}"),
            None => "records hold no signal".to_string(),
        }));
    }

    let mut out = Vec::new();
    for (index, record) in records.iter().enumerate() {
        let id = record_id(record, index);
        let mut written = 0;
        for (name, signal) in &record.signals {
            if !names.contains(&name.as_str()) {
                context.losses.add(
                    "signals_omitted",
                    "signals other than the selected one are not written",
                );
                continue;
            }
            written += 1;
            check_layout(signal, &id, name)?;
            let signal_label = if names.len() > 1 {
                name.clone()
            } else {
                String::new()
            };
            if signal.is_1d() {
                out.push(Spectrum {
                    id: id.clone(),
                    signal_label,
                    signal_name: name,
                    signal,
                    axis: &signal.axis,
                    values: Cow::Borrowed(&signal.values),
                    record,
                });
                continue;
            }
            context.losses.add(
                "dimensions_flattened",
                "non-spectral dimensions are flattened; their coordinates remain only in spectrum names",
            );
            for (suffix, values) in flatten(signal) {
                out.push(Spectrum {
                    id: format!("{id}_{suffix}"),
                    signal_label: signal_label.clone(),
                    signal_name: name,
                    signal,
                    axis: &signal.axis,
                    values: Cow::Owned(values),
                    record,
                });
            }
        }
        if written == 0 {
            context.losses.add(
                "records_omitted",
                "records without the selected signal are not written",
            );
        }
    }
    Ok(out)
}

/// Records may come from a host binding, so their layout is checked here.
fn check_layout(signal: &SpectralArray, id: &str, name: &str) -> Result<(), WriteError> {
    let x_dims = signal.dims.iter().filter(|dim| *dim == "x").count();
    let extent = signal
        .shape
        .iter()
        .try_fold(1usize, |acc, extent| acc.checked_mul(*extent));
    let consistent = x_dims == 1
        && signal.shape.len() == signal.dims.len()
        && extent == Some(signal.values.len())
        && !signal.values.is_empty()
        && signal.shape[signal.x_dim_index()] == signal.axis.values.len()
        && signal.axis.values.iter().all(|value| value.is_finite());
    if consistent {
        Ok(())
    } else {
        Err(refused(format!(
            "spectrum '{id}' signal '{name}' has inconsistent axis, shape or values"
        )))
    }
}

fn flatten(signal: &SpectralArray) -> Vec<(String, Vec<f64>)> {
    let x = signal.x_dim_index();
    let stride: usize = signal.shape[x + 1..].iter().product();
    let points = signal.shape[x];
    let count = signal.values.len() / points;
    (0..count)
        .map(|flat| {
            // Split the flat index into the parts before and after the x dimension.
            let (outer, inner) = (flat / stride, flat % stride);
            let start = outer * points * stride + inner;
            let values = (0..points)
                .map(|point| signal.values[start + point * stride])
                .collect();
            (coordinate_suffix(signal, x, outer, inner), values)
        })
        .collect()
}

fn coordinate_suffix(signal: &SpectralArray, x: usize, outer: usize, inner: usize) -> String {
    let mut indices = vec![0usize; signal.shape.len()];
    for (dims, mut flat) in [((0..x), outer), ((x + 1..signal.shape.len()), inner)] {
        for dim in dims.rev() {
            indices[dim] = flat % signal.shape[dim];
            flat /= signal.shape[dim];
        }
    }
    (0..signal.shape.len())
        .filter(|dim| *dim != x)
        .map(|dim| format!("{}{}", signal.dims[dim], indices[dim]))
        .collect::<Vec<_>>()
        .join("_")
}

fn record_id(record: &SpectralRecord, index: usize) -> String {
    for key in ["sample_id", "sample", "id", "sample_name", "name", "title"] {
        match record.metadata.get(key) {
            Some(Value::String(text)) if !text.trim().is_empty() => return text.trim().to_string(),
            Some(Value::Number(value)) => return value.to_string(),
            _ => {}
        }
    }
    format!("spectrum_{}", index + 1)
}

/// Report what per-spectrum exchange formats never carry.
pub(crate) fn report_record_losses(
    records: &[SpectralRecord],
    context: &mut Context<'_>,
    targets: TargetSupport,
) {
    for record in records {
        if !record.targets.is_empty() {
            match targets {
                TargetSupport::Comments => context.losses.add(
                    "targets_in_comments",
                    "target values are written as non-standard comments most software ignores",
                ),
                TargetSupport::None => context.losses.add(
                    "targets_omitted",
                    "target values are not written by this format",
                ),
                TargetSupport::Columns => {}
            }
        }
        if !record.metadata.is_empty() {
            context.losses.add(
                "metadata_omitted",
                "metadata other than the sample identifier is not written",
            );
        }
        if !record.quality_flags.is_empty() {
            context
                .losses
                .add("quality_flags_omitted", "quality flags are not written");
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum TargetSupport {
    Columns,
    Comments,
    None,
}

/// Scalar text of a target, or `None` when it is null or structured.
pub(crate) fn target_text(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) => Some(text.clone()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// Even spacing as first/last/count, when the axis allows it.
///
/// Returns the largest deviation between the stored coordinates and the
/// regenerated ones, or `None` when coordinates must stay explicit.
pub(crate) fn even_spacing(axis: &[f64], tolerance: f64) -> Option<f64> {
    let (first, last) = (*axis.first()?, *axis.last()?);
    if axis.len() < 2 || first == last {
        return None;
    }
    let step = (last - first) / (axis.len() - 1) as f64;
    let scale = first.abs().max(last.abs());
    let noise = scale * 1e-12;
    let deviation = axis
        .iter()
        .enumerate()
        .map(|(index, value)| (value - (first + step * index as f64)).abs())
        .fold(0.0, f64::max);
    (deviation <= noise.max(tolerance)).then_some(deviation)
}

pub(crate) fn report_even_spacing(context: &mut Context<'_>, axis: &[f64], deviation: f64) {
    let scale = axis.iter().fold(0.0f64, |acc, value| acc.max(value.abs()));
    if deviation > scale * 1e-12 {
        context.losses.add_error(
            "axis_regularized",
            "axis coordinates are replaced by an evenly spaced axis within the requested tolerance",
            Some(deviation),
        );
    }
}

pub(crate) fn signal_label(signal_type: &SignalType) -> &'static str {
    match signal_type {
        SignalType::Absorbance => "absorbance",
        SignalType::Reflectance => "reflectance",
        SignalType::Transmittance => "transmittance",
        SignalType::Radiance => "radiance",
        SignalType::Irradiance => "irradiance",
        SignalType::RawCounts => "raw counts",
        SignalType::SingleBeam => "single beam",
        SignalType::Interferogram => "interferogram",
        SignalType::KubelkaMunk => "Kubelka-Munk",
        SignalType::Derivative => "derivative",
        SignalType::Preprocessed => "preprocessed",
        SignalType::AerosolOpticalThickness => "aerosol optical thickness",
        SignalType::Uncertainty => "uncertainty",
        SignalType::Unknown => "unknown",
    }
}

/// Spectra sharing signal name, type, unit and identical axis.
pub(crate) fn groups<'s, 'a>(spectra: &'s [Spectrum<'a>]) -> Vec<Vec<&'s Spectrum<'a>>> {
    let mut groups: Vec<Vec<&Spectrum<'a>>> = Vec::new();
    // Candidate groups per signal name keep this linear for one shared axis.
    let mut by_name: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for spectrum in spectra {
        let candidates = by_name.entry(spectrum.signal_name).or_default();
        let found = candidates.iter().copied().find(|index| {
            let head = groups[*index][0];
            head.signal.signal_type == spectrum.signal.signal_type
                && head.signal.unit == spectrum.signal.unit
                && head.axis == spectrum.axis
        });
        match found {
            Some(index) => groups[index].push(spectrum),
            None => {
                candidates.push(groups.len());
                groups.push(vec![spectrum]);
            }
        }
    }
    groups
}
