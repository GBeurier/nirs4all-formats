//! Conversion planning: which steps make records writable for a target.
//!
//! Formats decides what a target needs from the signal types, units, roles and
//! axes the readers expose. It never computes converted values: every
//! numerical step names the nirs4all-methods catalog method that computes it.
//! Pure selections of points name no method. Range checks only convert the two
//! limits of the measured axis to place a grid inside it.

use nirs4all_formats_core::{SignalType, SpectralArray, SpectralRecord};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use super::requirements::{target_requirements, EvenAxis, Grid, OtherGrids, TargetRequirements};
use super::select::{scale_of, signal_label, technique, Scale, Technique};
use super::software::software_catalog;
use super::{nsas, refused, winisi, WriteError, WRITERS};

const TO_ABSORBANCE: &str = "preprocessing.signal_conversion.to_absorbance";
const FROM_ABSORBANCE: &str = "preprocessing.signal_conversion.from_absorbance";
const KUBELKA_MUNK: &str = "preprocessing.signal_conversion.kubelka_munk";
const FROM_KUBELKA_MUNK: &str = "preprocessing.signal_conversion.from_kubelka_munk";
const PERCENT_TO_FRACTION: &str = "preprocessing.signal_conversion.percent_to_fraction";
const FROM_REFERENCES: &str = "preprocessing.signal_conversion.reflectance_from_references";
const SPLICE_CORRECTION: &str = "preprocessing.signal_conversion.splice_correction";
const AXIS_UNIT_CONVERSION: &str = "preprocessing.resampling.axis_unit_conversion";
const RESAMPLER: &str = "preprocessing.resampling.resampler";

/// Choices and answers of a conversion plan. Defaults plan automatically.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlanOptions {
    /// Signal to convert (default: the best candidate for the target).
    pub signal: Option<String>,
    /// Wanted signal type, for targets that accept several or no target.
    pub signal_type: Option<SignalType>,
    /// Wanted axis unit (`nm`, `um` or `cm-1`).
    pub axis_unit: Option<String>,
    /// `measured` (no resampling), `even`, or a grid id of the target.
    pub grid: Option<String>,
    /// Step of an `even` grid, in the output axis unit.
    pub grid_step: Option<f64>,
    /// Limits of an `even` grid; within the measured range.
    pub grid_min: Option<f64>,
    pub grid_max: Option<f64>,
    /// Resampling method: `linear` (default), `nearest` or `cubic`.
    pub interpolation: Option<String>,
    /// Answer to `source_scale`: `percent` or `fraction`.
    pub source_scale: Option<String>,
    /// Answer to `signal_type` for signals of unknown type.
    pub assume_signal_type: Option<SignalType>,
    /// Answer to `absorbance_origin`: `reflectance` or `transmittance`.
    pub absorbance_origin: Option<SignalType>,
    /// Axis deviation accepted as even, as in `WriteOptions`.
    pub axis_tolerance: f64,
    /// First axis value of each following detector, for the splice
    /// correction of reflectance computed from references.
    pub splices: Option<Vec<f64>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    /// Run the steps, then write the output signal.
    Ready,
    /// Answer the questions and plan again.
    NeedsAnswers,
    /// See the refusals.
    Refused,
    /// The signal is written as it is.
    Unchanged,
}

/// The signal the steps produce.
#[derive(Clone, Debug, Serialize)]
pub struct PlannedSignal {
    /// Suggested name, distinct from the existing signals.
    pub name: String,
    pub signal_type: SignalType,
    /// `%` for percent values, otherwise none.
    pub unit: Option<String>,
    pub axis_unit: String,
}

/// One step, in execution order.
#[derive(Clone, Debug, Serialize)]
pub struct PlanStep {
    /// `reflectance_from_references`, `declare_signal_type`,
    /// `signal_conversion`, `select_points`, `axis_unit_conversion` or
    /// `resample`.
    pub step: &'static str,
    /// nirs4all-methods catalog method; none for selections and declarations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method_id: Option<&'static str>,
    /// Method parameters.
    pub params: Map<String, Value>,
    /// Other inputs: reference signal names, the measured axis, kept ranges.
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub inputs: Map<String, Value>,
    /// Signal type, percent flag and axis unit after the step.
    pub signal_type: SignalType,
    pub percent: bool,
    pub axis_unit: String,
    pub description: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlanChoice {
    pub value: String,
    pub label: String,
}

/// Information the records do not hold; answer through the options.
#[derive(Clone, Debug, Serialize)]
pub struct PlanQuestion {
    /// Option that answers it.
    pub id: &'static str,
    pub prompt: String,
    pub choices: Vec<PlanChoice>,
    pub why: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlanRefusal {
    pub code: &'static str,
    pub reason: String,
    pub fix: String,
}

/// A signal of the records and whether it can be converted.
#[derive(Clone, Debug, Serialize)]
pub struct SignalCandidate {
    pub name: String,
    pub signal_type: SignalType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub role: String,
    pub usable: bool,
    pub reason: String,
}

/// Steps that make a signal writable for a target.
#[derive(Clone, Debug, Serialize)]
pub struct ConversionPlan {
    pub target: Option<String>,
    pub format: Option<&'static str>,
    pub status: PlanStatus,
    pub source_signal: Option<String>,
    /// Records holding the source signal.
    pub records: usize,
    pub output: Option<PlannedSignal>,
    pub steps: Vec<PlanStep>,
    pub questions: Vec<PlanQuestion>,
    pub refusals: Vec<PlanRefusal>,
    pub candidates: Vec<SignalCandidate>,
    /// Measured axis limits in the output axis unit, when known.
    pub measured_range: Option<[f64; 2]>,
    /// Grid of the resampling step, when there is one.
    pub grid: Option<Grid>,
    /// Limits that apply to this plan.
    pub notes: Vec<String>,
}

/// Plan the conversion of `records` for `target` (a writer format or a
/// software preset), or for the options alone when `target` is `None`.
///
/// Invalid options and unknown targets are errors; what the records cannot
/// become is reported in the plan's refusals.
pub fn plan_conversion(
    records: &[SpectralRecord],
    target: Option<&str>,
    options: &PlanOptions,
) -> Result<ConversionPlan, WriteError> {
    check_options(options)?;
    if records.is_empty() {
        return Err(refused("no records to convert"));
    }
    let (format, requirements, profile) = match target {
        None => (None, None, None),
        Some(target) => {
            let profile = software_catalog()
                .iter()
                .find(|profile| profile.id == target);
            let format = profile.map_or(target, |profile| profile.format);
            let entry = WRITERS
                .iter()
                .find(|entry| entry.format == format)
                .ok_or_else(|| WriteError::UnknownTarget(target.to_string()))?;
            (
                Some(entry.format),
                target_requirements(entry.format),
                profile,
            )
        }
    };
    let mut planner = Planner {
        records,
        options,
        requirements: requirements.as_ref(),
        plan: ConversionPlan {
            target: target.map(str::to_string),
            format,
            status: PlanStatus::Unchanged,
            source_signal: None,
            records: 0,
            output: None,
            steps: Vec::new(),
            questions: Vec::new(),
            refusals: Vec::new(),
            candidates: Vec::new(),
            measured_range: None,
            grid: None,
            notes: Vec::new(),
        },
    };
    planner.run();
    let mut plan = planner.plan;
    if let Some(profile) = profile.filter(|profile| !profile.tested) {
        plan.notes.push(format!(
            "Files for {} are not tested in that software yet (basis: {}).",
            profile.label, profile.evidence
        ));
    }
    plan.status = if !plan.refusals.is_empty() {
        PlanStatus::Refused
    } else if !plan.questions.is_empty() {
        PlanStatus::NeedsAnswers
    } else if plan.steps.is_empty() {
        PlanStatus::Unchanged
    } else {
        PlanStatus::Ready
    };
    Ok(plan)
}

fn check_options(options: &PlanOptions) -> Result<(), WriteError> {
    let one_of = |name: &str, value: &Option<String>, allowed: &[&str]| match value {
        Some(value) if !allowed.contains(&value.as_str()) => Err(refused(format!(
            "{name} must be one of {allowed:?}, not '{value}'"
        ))),
        _ => Ok(()),
    };
    one_of(
        "interpolation",
        &options.interpolation,
        &["linear", "nearest", "cubic"],
    )?;
    one_of(
        "source_scale",
        &options.source_scale,
        &["percent", "fraction"],
    )?;
    if let Some(splices) = &options.splices {
        let increasing = splices.windows(2).all(|pair| pair[1] > pair[0]);
        if splices.is_empty() || !increasing || splices.iter().any(|x| !x.is_finite()) {
            return Err(refused(
                "splices must be finite, increasing axis values, one per following detector",
            ));
        }
    }
    if let Some(unit) = &options.axis_unit {
        if axis_unit(unit).is_none() {
            return Err(refused(format!(
                "axis_unit must be 'nm', 'um' or 'cm-1', not '{unit}'"
            )));
        }
    }
    if !options.axis_tolerance.is_finite() || options.axis_tolerance < 0.0 {
        return Err(refused("axis_tolerance must be a finite value >= 0"));
    }
    if options
        .grid_step
        .is_some_and(|step| !step.is_finite() || step <= 0.0)
    {
        return Err(refused("grid_step must be a finite value > 0"));
    }
    if [options.grid_min, options.grid_max]
        .iter()
        .flatten()
        .any(|limit| !limit.is_finite())
    {
        return Err(refused("grid_min and grid_max must be finite"));
    }
    if let (Some(low), Some(high)) = (options.grid_min, options.grid_max) {
        if low >= high {
            return Err(refused("grid_min must be lower than grid_max"));
        }
    }
    for (name, answer) in [
        ("absorbance_origin", &options.absorbance_origin),
        ("assume_signal_type", &options.assume_signal_type),
    ] {
        let allowed: &[SignalType] = if name == "absorbance_origin" {
            &[SignalType::Reflectance, SignalType::Transmittance]
        } else {
            &OPTICAL
        };
        if answer.as_ref().is_some_and(|kind| !allowed.contains(kind)) {
            return Err(refused(format!(
                "{name} must be one of {}",
                allowed
                    .iter()
                    .map(signal_label)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    }
    Ok(())
}

/// Signal types the conversion steps relate to each other.
const OPTICAL: [SignalType; 4] = [
    SignalType::Absorbance,
    SignalType::Reflectance,
    SignalType::Transmittance,
    SignalType::KubelkaMunk,
];

/// Canonical axis unit, for the units axis conversion handles.
fn axis_unit(unit: &str) -> Option<&'static str> {
    match unit.trim().to_ascii_lowercase().as_str() {
        "nm" => Some("nm"),
        "um" | "µm" | "micrometer" | "micrometers" => Some("um"),
        "cm-1" | "1/cm" | "cm^-1" => Some("cm-1"),
        _ => None,
    }
}

/// One axis limit in another unit (vacuum wavelengths).
fn convert_limit(value: f64, from: &str, to: &str) -> f64 {
    let nm = match from {
        "um" => value * 1e3,
        "cm-1" => 1e7 / value,
        _ => value,
    };
    match to {
        "um" => nm / 1e3,
        "cm-1" => 1e7 / nm,
        _ => nm,
    }
}

/// How a signal can serve as a source.
enum Source {
    /// Absorbance, reflectance, transmittance or Kubelka-Munk.
    Optical,
    /// Unknown signal type: written as it is, converted once declared.
    Unknown,
    /// Measurement turned into reflectance with references of its record.
    References {
        white: String,
        dark: Option<String>,
    },
    /// A measurement that needs references the records do not hold. It is
    /// written as it is when no conversion is needed.
    Measurement(PlanRefusal),
    Unusable(PlanRefusal),
}

struct Planner<'a> {
    records: &'a [SpectralRecord],
    options: &'a PlanOptions,
    requirements: Option<&'a TargetRequirements>,
    plan: ConversionPlan,
}

/// The signal as the steps leave it.
struct State {
    signal_type: SignalType,
    /// `None` while percent or fraction is not known.
    percent: Option<bool>,
    unit: &'static str,
    /// Measured axis values while they are unchanged (selection and sorting
    /// keep them); `None` once converted or resampled.
    axis: Option<Vec<f64>>,
    range: [f64; 2],
    points: usize,
}

impl<'a> Planner<'a> {
    fn run(&mut self) {
        let names = self.signal_names();
        let classified: Vec<(String, Source)> = names
            .iter()
            .map(|name| (name.clone(), self.classify(name)))
            .collect();
        self.plan.candidates = classified
            .iter()
            .map(|(name, source)| self.candidate(name, source))
            .collect();
        let Some((name, source)) = self.choose(&classified) else {
            return;
        };
        self.plan.source_signal = Some(name.clone());
        self.plan.records = self
            .records
            .iter()
            .filter(|record| record.signals.contains_key(&name))
            .count();
        let signal = self.first(&name);
        let wanted = self.wanted_type(signal, &source);
        let Some(mut state) = self.signal_steps(&name, signal, &source, wanted) else {
            return;
        };
        if !self.axis_steps(&name, signal, &mut state) {
            return;
        }
        if !self.plan.steps.is_empty() {
            self.check_output(&state);
            self.plan.output = Some(PlannedSignal {
                name: self.output_name(&name, signal, &state),
                signal_type: state.signal_type.clone(),
                unit: (state.percent == Some(true)).then(|| "%".to_string()),
                axis_unit: state.unit.to_string(),
            });
        }
    }

    fn signal_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for record in self.records {
            for name in record.signals.keys() {
                if !names.contains(name) {
                    names.push(name.clone());
                }
            }
        }
        names
    }

    fn first(&self, name: &str) -> &'a SpectralArray {
        self.records
            .iter()
            .find_map(|record| record.signals.get(name))
            .expect("signal names come from the records")
    }

    fn holders<'s>(
        &self,
        name: &'s str,
    ) -> impl Iterator<Item = (&'a SpectralRecord, &'a SpectralArray)> + 's
    where
        'a: 's,
    {
        self.records
            .iter()
            .filter_map(move |record| Some((record, record.signals.get(name)?)))
    }

    fn classify(&self, name: &str) -> Source {
        let signal = self.first(name);
        let refuse = |code, reason: String, fix: &str| {
            Source::Unusable(PlanRefusal {
                code,
                reason,
                fix: fix.to_string(),
            })
        };
        if is_reference(name, signal) {
            return refuse(
                "reference_signal",
                format!("'{name}' is a reference measurement, not a spectrum of the sample"),
                "Convert the sample signal; references are used to compute its reflectance.",
            );
        }
        if self
            .holders(name)
            .any(|(_, signal)| !signal.is_1d() || signal.dims.len() != 1)
        {
            return refuse(
                "multidimensional_signal",
                format!("'{name}' is multidimensional; only one-dimensional spectra are converted"),
                "Export it without conversion, or select one spectrum of it with your own tools.",
            );
        }
        if let Some(requirements) = self.requirements {
            if requirements
                .refused_signal_types
                .contains(&signal.signal_type)
            {
                return refuse(
                    "signal_type_refused",
                    format!(
                        "{} does not write {} signals such as '{name}'",
                        requirements.format,
                        signal_label(&signal.signal_type)
                    ),
                    "Pick another signal of the file, or another target.",
                );
            }
        }
        match &signal.signal_type {
            kind if OPTICAL.contains(kind) => Source::Optical,
            SignalType::Unknown => Source::Unknown,
            SignalType::RawCounts
            | SignalType::SingleBeam
            | SignalType::Radiance
            | SignalType::Irradiance => self.references(name, signal),
            SignalType::Interferogram => refuse(
                "interferogram",
                format!("'{name}' is an interferogram; turning it into a spectrum needs a Fourier transform with acquisition parameters, which is not a conversion"),
                "Export the spectrum the instrument software computed (absorbance, reflectance or single beam).",
            ),
            SignalType::Derivative | SignalType::Preprocessed => refuse(
                "preprocessed_signal",
                format!("'{name}' is a {} signal; it cannot be turned back into a measured spectrum", signal_label(&signal.signal_type)),
                "Export the measured spectrum, then preprocess it in the target software.",
            ),
            other => refuse(
                "not_a_sample_spectrum",
                format!("'{name}' holds {}, which no conversion turns into a sample spectrum", signal_label(other)),
                "Pick another signal of the file.",
            ),
        }
    }

    /// White (and dark) references measured with the sample, on its axis.
    fn references(&self, name: &str, signal: &SpectralArray) -> Source {
        let kind = &signal.signal_type;
        let mut white: Option<String> = None;
        let mut dark: Option<String> = None;
        for (record, sample) in self.holders(name) {
            let find = |dark_wanted: bool| {
                record.signals.iter().find(|(other, reference)| {
                    let dark = is_dark(other, reference);
                    other.as_str() != name
                        && is_reference(other, reference)
                        && dark == dark_wanted
                        && reference.signal_type != SignalType::Interferogram
                        && (dark || reference.signal_type == *kind)
                        && reference.axis.values == sample.axis.values
                        && reference.values.len() == sample.values.len()
                })
            };
            let Some((found, _)) = find(false) else {
                let reason = if self.records.iter().any(|record| {
                    record
                        .signals
                        .iter()
                        .any(|(n, s)| n != name && is_reference(n, s) && !is_dark(n, s))
                }) {
                    format!(
                        "'{name}' ({}) has no reference measured on the same axis in every record",
                        signal_label(kind)
                    )
                } else {
                    format!(
                        "'{name}' holds {}; reflectance needs a white reference measured with it, which these records do not hold",
                        signal_label(kind)
                    )
                };
                return Source::Measurement(PlanRefusal {
                    code: "references_missing",
                    reason,
                    fix: "Convert a processed signal of the file (reflectance, transmittance or absorbance), or load the file together with its reference measurements.".to_string(),
                });
            };
            if white.as_deref().is_some_and(|white| white != found) {
                return Source::Measurement(PlanRefusal {
                    code: "references_ambiguous",
                    reason: format!("records name the white reference of '{name}' differently"),
                    fix: "Convert the records of one file at a time.".to_string(),
                });
            }
            let found_dark = if *kind == SignalType::RawCounts {
                find(true).map(|(found, _)| found.clone())
            } else {
                None
            };
            if white.is_some() && found_dark != dark {
                return Source::Measurement(PlanRefusal {
                    code: "references_ambiguous",
                    reason: format!("only some records hold a dark reference for '{name}'"),
                    fix: "Convert the records of one file at a time.".to_string(),
                });
            }
            white = Some(found.clone());
            dark = found_dark;
        }
        match white {
            Some(white) => Source::References { white, dark },
            None => Source::Unusable(PlanRefusal {
                code: "signal_missing",
                reason: format!("no record holds '{name}'"),
                fix: "Pick a signal of the file.".to_string(),
            }),
        }
    }

    fn candidate(&self, name: &str, source: &Source) -> SignalCandidate {
        let signal = self.first(name);
        let (usable, reason) = match source {
            Source::Optical => (true, format!("{} signal", described(signal))),
            Source::Unknown => (
                true,
                "signal of unknown type; its type is asked before any conversion".to_string(),
            ),
            Source::References { white, dark } => (
                true,
                match dark {
                    Some(dark) => format!(
                        "reflectance from white reference '{white}' and dark reference '{dark}'"
                    ),
                    None => format!("reflectance from white reference '{white}'"),
                },
            ),
            Source::Measurement(refusal) => (
                self.wanted_for(signal).is_none(),
                format!("{}; written as it is only", refusal.reason),
            ),
            Source::Unusable(refusal) => (false, refusal.reason.clone()),
        };
        SignalCandidate {
            name: name.to_string(),
            signal_type: signal.signal_type.clone(),
            unit: signal.unit.clone(),
            role: signal.role.clone(),
            usable,
            reason,
        }
    }

    /// Source signal: the requested one, or the best candidate.
    fn choose(&mut self, classified: &[(String, Source)]) -> Option<(String, Source)> {
        if let Some(wanted) = &self.options.signal {
            let Some(index) = classified.iter().position(|(name, _)| name == wanted) else {
                self.refuse(
                    "signal_missing",
                    format!("no record holds a signal named '{wanted}'"),
                    "Pick one of the candidate signals.",
                );
                return None;
            };
            let (name, source) = &classified[index];
            if let Source::Unusable(refusal) = source {
                self.plan.refusals.push(refusal.clone());
                return None;
            }
            return Some((name.clone(), self.classify(name)));
        }
        let rank = |name: &String, source: &Source| -> Option<(u8, u8)> {
            let signal = self.first(name);
            let wanted = self.wanted_for(signal);
            let main = self
                .holders(name)
                .next()
                .is_some_and(|(record, _)| record.signal_type == signal.signal_type);
            let rank = match source {
                Source::Optical if wanted.as_ref() == Some(&signal.signal_type) => 0,
                Source::Optical => 1,
                Source::Unknown => 2,
                Source::References { .. } => 3,
                Source::Measurement(_) if wanted.is_none() => 4,
                _ => return None,
            };
            Some((rank, u8::from(!main)))
        };
        let best = classified
            .iter()
            .filter_map(|(name, source)| Some((rank(name, source)?, name)))
            .min_by_key(|(rank, _)| *rank)
            .map(|(_, name)| name.clone());
        match best {
            Some(name) => {
                let source = self.classify(&name);
                Some((name, source))
            }
            None => {
                let refusals: Vec<PlanRefusal> = classified
                    .iter()
                    .filter_map(|(_, source)| match source {
                        Source::Unusable(refusal) | Source::Measurement(refusal) => {
                            Some(refusal.clone())
                        }
                        _ => None,
                    })
                    .collect();
                // References explain themselves once a sample signal is refused.
                let samples: Vec<PlanRefusal> = refusals
                    .iter()
                    .filter(|refusal| refusal.code != "reference_signal")
                    .cloned()
                    .collect();
                self.plan.refusals.extend(if samples.is_empty() {
                    refusals
                } else {
                    samples
                });
                if self.plan.refusals.is_empty() {
                    self.refuse(
                        "no_signal",
                        "the records hold no signal".to_string(),
                        "Load a file with spectra.",
                    );
                }
                None
            }
        }
    }

    /// Signal type the output must have, before any answer.
    fn wanted_for(&self, signal: &SpectralArray) -> Option<SignalType> {
        match self.requirements {
            Some(requirements) if !requirements.signal_types.is_empty() => {
                if requirements.accepts_signal(&signal.signal_type) {
                    None
                } else {
                    requirements.signal_types.first().cloned()
                }
            }
            _ => self
                .options
                .signal_type
                .clone()
                .filter(|kind| *kind != signal.signal_type),
        }
    }

    fn wanted_type(&mut self, signal: &SpectralArray, source: &Source) -> Option<SignalType> {
        if let (Some(kind), Some(requirements)) = (&self.options.signal_type, self.requirements) {
            if !requirements.accepts_signal(kind) {
                self.refuse(
                    "signal_type_refused",
                    format!(
                        "{} does not store {} spectra",
                        requirements.format,
                        signal_label(kind)
                    ),
                    "Choose a signal type the target stores, or another target.",
                );
            }
        }
        let wanted = self.wanted_for(signal);
        match (source, &wanted) {
            // An unknown type the target stores as it is needs no answer.
            (Source::Unknown, None) if self.options.assume_signal_type.is_none() => None,
            _ => wanted.or_else(|| self.options.signal_type.clone()),
        }
    }

    fn refuse(&mut self, code: &'static str, reason: String, fix: &str) {
        self.plan.refusals.push(PlanRefusal {
            code,
            reason,
            fix: fix.to_string(),
        });
    }

    fn ask(&mut self, id: &'static str, prompt: String, choices: &[(&str, &str)], why: &str) {
        self.plan.questions.push(PlanQuestion {
            id,
            prompt,
            choices: choices
                .iter()
                .map(|(value, label)| PlanChoice {
                    value: value.to_string(),
                    label: label.to_string(),
                })
                .collect(),
            why: why.to_string(),
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        step: &'static str,
        method_id: Option<&'static str>,
        params: Value,
        inputs: Value,
        state: &State,
        description: String,
        notes: &[&str],
    ) {
        let object = |value: Value| match value {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        self.plan.steps.push(PlanStep {
            step,
            method_id,
            params: object(params),
            inputs: object(inputs),
            signal_type: state.signal_type.clone(),
            percent: state.percent == Some(true),
            axis_unit: state.unit.to_string(),
            description,
            notes: notes.iter().map(|note| note.to_string()).collect(),
        });
    }

    /// Signal steps; `None` when a refusal or a question stops the plan.
    fn signal_steps(
        &mut self,
        name: &str,
        signal: &SpectralArray,
        source: &Source,
        wanted: Option<SignalType>,
    ) -> Option<State> {
        let unit = axis_unit(&signal.axis.unit).unwrap_or("");
        let (range, points) = self.measured(name);
        let mut state = State {
            signal_type: signal.signal_type.clone(),
            percent: None,
            unit,
            axis: Some(signal.axis.values.clone()),
            range,
            points,
        };
        match scale_of(signal) {
            Scale::Percent => state.percent = Some(true),
            Scale::Plain
                if signal
                    .unit
                    .as_deref()
                    .is_some_and(|unit| !unit.trim().is_empty())
                    || !matches!(
                        signal.signal_type,
                        SignalType::Reflectance | SignalType::Transmittance
                    ) =>
            {
                state.percent = Some(false)
            }
            Scale::Plain => {}
            Scale::Other(other) => {
                if wanted.is_some() {
                    self.refuse(
                        "scaled_unit",
                        format!(
                            "'{name}' is stored in '{other}', which the conversions do not rescale"
                        ),
                        "Export it without conversion, or rescale it to plain units first.",
                    );
                    return None;
                }
                state.percent = Some(false);
            }
        }
        let Some(wanted) = wanted else {
            return Some(state);
        };

        match source {
            Source::References { white, dark } => {
                state.signal_type = SignalType::Reflectance;
                state.percent = Some(false);
                let mut notes = vec![
                    "The white reference is taken as 100 % reflectance at every point; panel calibration factors are not applied.",
                ];
                notes.push(if dark.is_some() {
                    "Points where white - dark <= 0 become missing values."
                } else if signal.signal_type == SignalType::RawCounts {
                    "No dark reference: R = sample / white, so the detector dark signal is not removed."
                } else {
                    "Points where the reference is <= 0 become missing values."
                });
                let (formula, inputs) = match dark {
                    Some(dark) => (
                        format!("R = ({name} - {dark}) / ({white} - {dark})"),
                        json!({ "sample": name, "white_reference": white, "dark_reference": dark }),
                    ),
                    None => (
                        format!("R = {name} / {white}"),
                        json!({ "sample": name, "white_reference": white }),
                    ),
                };
                if signal.signal_type == SignalType::RawCounts
                    && !has_word(name, signal, &["norm", "normalized"])
                {
                    match self.integration_times(name) {
                        Some(false) => {
                            self.refuse(
                                "integration_times_differ",
                                format!("'{name}' and its white reference were measured with different integration times; a ratio of raw counts is then wrong"),
                                "Convert the vendor-computed reflectance of the file, or counts normalized by integration time.",
                            );
                            return None;
                        }
                        Some(true) => {}
                        None => notes.push("The file records no integration times: the ratio assumes the sample and the references were measured with the same one."),
                    }
                }
                let vendor_note = self.vendor_signal(name).map(|vendor| {
                    format!("The file also holds '{vendor}', computed by the instrument software; prefer it unless you need to recompute reflectance.")
                });
                if let Some(note) = &vendor_note {
                    notes.push(note);
                }
                self.push(
                    "reflectance_from_references",
                    Some(FROM_REFERENCES),
                    json!({ "as_percent": false }),
                    inputs,
                    &state,
                    format!("Reflectance from the references measured with the sample: {formula}."),
                    &notes,
                );
                self.splice_correction(name, &mut state)?;
            }
            Source::Measurement(refusal) => {
                self.plan.refusals.push(refusal.clone());
                return None;
            }
            Source::Unknown => {
                let Some(assumed) = self.options.assume_signal_type.clone() else {
                    self.ask(
                        "assume_signal_type",
                        format!("What does '{name}' hold? The file does not say."),
                        &[
                            ("absorbance", "Absorbance (log 1/R or log 1/T)"),
                            ("reflectance", "Reflectance"),
                            ("transmittance", "Transmittance"),
                            ("kubelka_munk", "Kubelka-Munk"),
                        ],
                        "The conversion depends on the signal type; a signal type detector can only suggest one.",
                    );
                    return None;
                };
                state.signal_type = assumed.clone();
                if matches!(assumed, SignalType::Reflectance | SignalType::Transmittance)
                    && signal
                        .unit
                        .as_deref()
                        .is_none_or(|unit| unit.trim().is_empty())
                {
                    state.percent = None;
                }
                self.push(
                    "declare_signal_type",
                    None,
                    json!({ "signal_type": assumed }),
                    json!({}),
                    &state,
                    format!(
                        "'{name}' is declared {} as you answered; values are not changed.",
                        signal_label(&assumed)
                    ),
                    &[],
                );
            }
            Source::Optical | Source::Unusable(_) => {}
        }
        self.convert_signal(name, &mut state, &wanted)?;
        Some(state)
    }

    /// Whether the records state equal sample and reference integration
    /// times; `None` when they do not record them.
    fn integration_times(&self, name: &str) -> Option<bool> {
        let mut equal = None;
        for (record, _) in self.holders(name) {
            let target = record.metadata.get("integration_time_target_ms");
            let reference = record.metadata.get("integration_time_reference_ms");
            if let (Some(target), Some(reference)) = (target, reference) {
                equal = Some(equal.unwrap_or(true) && target == reference);
            }
        }
        equal
    }

    /// A vendor-computed reflectance or transmittance next to `name`.
    fn vendor_signal(&self, name: &str) -> Option<String> {
        let (record, _) = self.holders(name).next()?;
        record
            .signals
            .iter()
            .find(|(other, signal)| {
                other.as_str() != name
                    && matches!(
                        signal.signal_type,
                        SignalType::Reflectance | SignalType::Transmittance
                    )
            })
            .map(|(other, _)| other.clone())
    }

    /// Detector joins of reflectance computed from references, corrected
    /// where the axis (overlapping detectors) or the options tell where
    /// detectors meet. `None` when a refusal stops the plan.
    fn splice_correction(&mut self, name: &str, state: &mut State) -> Option<()> {
        let Some(axis) = state.axis.clone() else {
            return Some(());
        };
        let ordered = axis.windows(2).all(|pair| pair[1] > pair[0]);
        if !ordered && !self.select_points(name, state, &axis) {
            return None;
        }
        let splices = match &self.options.splices {
            Some(splices) => splices.clone(),
            None if !ordered => {
                // Each detector after the first starts a kept range.
                let kept = state.axis.clone().unwrap_or_default();
                let keep: Vec<[usize; 2]> = self
                    .plan
                    .steps
                    .last()
                    .and_then(|step| step.inputs.get("keep"))
                    .and_then(|keep| serde_json::from_value(keep.clone()).ok())
                    .unwrap_or_default();
                let mut position = 0;
                let mut starts = Vec::new();
                for (index, range) in keep.iter().enumerate() {
                    if index > 0 {
                        starts.push(kept[position]);
                    }
                    position += range[1] - range[0];
                }
                starts
            }
            None => {
                let detectors = self
                    .holders(name)
                    .next()
                    .and_then(|(record, _)| record.metadata.get("detector_channels"))
                    .and_then(Value::as_array)
                    .is_some_and(|channels| channels.len() > 1);
                if detectors {
                    self.plan.notes.push("Detector joins are not corrected: the file does not tell where the detectors meet. Give the first wavelength of each following detector as splices to correct them.".to_string());
                }
                return Some(());
            }
        };
        if splices.is_empty() {
            return Some(());
        }
        if splices
            .iter()
            .any(|x| *x <= state.range[0] || *x > state.range[1])
        {
            self.refuse(
                "splices_outside_axis",
                format!(
                    "splices {splices:?} must lie inside the measured axis {:.6}-{:.6}",
                    state.range[0], state.range[1]
                ),
                "Give the first wavelength of each following detector.",
            );
            return None;
        }
        let at = splices
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        self.push(
            "splice_correction",
            Some(SPLICE_CORRECTION),
            json!({
                "splices": splices,
                "reference_segment": 1,
                "method": "multiplicative",
                "window": 0.0,
                "taper": false,
            }),
            json!({ "axis": "current" }),
            state,
            format!("Detector jumps at {at} are removed by scaling each detector to the first one at the join."),
            &["Vendor software may correct detector joins differently (Spectral Evolution DARWin uses its own correction); compare with the vendor reflectance when the file has one."],
        );
        Some(())
    }

    /// Percent flag, asking for it when a step needs it.
    fn percent(&mut self, name: &str, state: &mut State) -> Option<bool> {
        if let Some(percent) = state.percent {
            return Some(percent);
        }
        match self.options.source_scale.as_deref() {
            Some(answer) => {
                state.percent = Some(answer == "percent");
                state.percent
            }
            None => {
                self.ask(
                    "source_scale",
                    format!("Is '{name}' stored in percent (0-100) or as a fraction (0-1)?",),
                    &[
                        ("percent", "Percent (0-100)"),
                        ("fraction", "Fraction (0-1)"),
                    ],
                    "The file gives no unit. A wrong answer shifts absorbance by 2 (log10 of 100).",
                );
                None
            }
        }
    }

    fn convert_signal(&mut self, name: &str, state: &mut State, wanted: &SignalType) -> Option<()> {
        use SignalType::{Absorbance, KubelkaMunk, Reflectance, Transmittance};
        let from = state.signal_type.clone();
        if from == *wanted {
            return self.fraction_if_required(name, state);
        }
        let to_absorbance = |planner: &mut Self, state: &mut State| -> Option<()> {
            let percent = planner.percent(name, state)?;
            let kind = state.signal_type.clone();
            state.signal_type = Absorbance;
            state.percent = Some(false);
            let mut notes = vec![
                "Values at or below 1e-10 (R or T as a fraction) are clamped before the logarithm, giving A = 10; the summary counts them.",
            ];
            notes.push(if kind == Reflectance {
                "log10(1/R) is apparent absorbance: it is not corrected for scattering."
            } else {
                "log10(1/T) is stored as absorbance; FOSS files call this log 1/R."
            });
            planner.push(
                "signal_conversion",
                Some(TO_ABSORBANCE),
                json!({ "is_percent": percent }),
                json!({}),
                state,
                format!(
                    "Absorbance A = log10(1/{}) from {}{}.",
                    if kind == Reflectance { "R" } else { "T" },
                    if percent { "percent " } else { "" },
                    signal_label(&kind)
                ),
                &notes,
            );
            Some(())
        };
        match (&from, wanted) {
            (Reflectance | Transmittance, Absorbance) => to_absorbance(self, state)?,
            (Reflectance, KubelkaMunk) => {
                let percent = self.percent(name, state)?;
                state.signal_type = KubelkaMunk;
                state.percent = Some(false);
                self.push(
                    "signal_conversion",
                    Some(KUBELKA_MUNK),
                    json!({ "is_percent": percent }),
                    json!({}),
                    state,
                    "Kubelka-Munk F(R) = (1 - R)^2 / (2R).".to_string(),
                    &["R is clamped to [1e-10, 1 - 1e-10] before the function."],
                );
            }
            (Absorbance, Reflectance | Transmittance) => {
                state.signal_type = wanted.clone();
                state.percent = Some(false);
                self.push(
                    "signal_conversion",
                    Some(FROM_ABSORBANCE),
                    json!({ "is_percent": false }),
                    json!({}),
                    state,
                    format!("{} = 10^-A, as a fraction.", if *wanted == Reflectance { "R" } else { "T" }),
                    &["This assumes the absorbance was computed as log10(1/R) or log10(1/T) without offsets."],
                );
            }
            (Absorbance, KubelkaMunk) => {
                match self.options.absorbance_origin.clone() {
                    None => {
                        self.ask(
                            "absorbance_origin",
                            format!(
                                "Was '{name}' computed from reflectance or from transmittance?"
                            ),
                            &[
                                ("reflectance", "Reflectance (log 1/R)"),
                                ("transmittance", "Transmittance (log 1/T)"),
                            ],
                            "Kubelka-Munk is defined for diffuse reflectance only.",
                        );
                        return None;
                    }
                    Some(Transmittance) => {
                        self.refuse(
                            "kubelka_munk_needs_reflectance",
                            format!("'{name}' comes from transmittance; Kubelka-Munk is defined for diffuse reflectance only"),
                            "Keep absorbance, or choose transmittance as the output.",
                        );
                        return None;
                    }
                    Some(_) => {}
                }
                state.signal_type = Reflectance;
                state.percent = Some(false);
                self.push(
                    "signal_conversion",
                    Some(FROM_ABSORBANCE),
                    json!({ "is_percent": false }),
                    json!({}),
                    state,
                    "R = 10^-A, as a fraction.".to_string(),
                    &[],
                );
                self.convert_signal(name, state, wanted)?;
            }
            (KubelkaMunk, Reflectance | Absorbance) => {
                state.signal_type = Reflectance;
                state.percent = Some(false);
                self.push(
                    "signal_conversion",
                    Some(FROM_KUBELKA_MUNK),
                    json!({ "is_percent": false }),
                    json!({}),
                    state,
                    "R = 1 / (1 + F + sqrt(F (F + 2))), as a fraction.".to_string(),
                    &["Negative Kubelka-Munk values become missing values."],
                );
                if *wanted == Absorbance {
                    to_absorbance(self, state)?;
                }
            }
            _ => {
                self.refuse(
                    "no_conversion",
                    format!(
                        "'{name}' is {}; no conversion gives {} from it",
                        signal_label(&from),
                        signal_label(wanted)
                    ),
                    "Choose another output signal type or another target.",
                );
                return None;
            }
        }
        Some(())
    }

    /// Percent values become fractions for targets that store no percent.
    fn fraction_if_required(&mut self, name: &str, state: &mut State) -> Option<()> {
        let percent_refused = self
            .requirements
            .is_some_and(|requirements| !requirements.percent);
        if !percent_refused
            || !matches!(
                state.signal_type,
                SignalType::Reflectance | SignalType::Transmittance
            )
        {
            return Some(());
        }
        if self.percent(name, state)? {
            state.percent = Some(false);
            self.push(
                "signal_conversion",
                Some(PERCENT_TO_FRACTION),
                json!({}),
                json!({}),
                state,
                "Percent values divided by 100.".to_string(),
                &[],
            );
        }
        Some(())
    }

    /// Range intersection and point count of the measured axes.
    fn measured(&self, name: &str) -> ([f64; 2], usize) {
        let mut range = [f64::NEG_INFINITY, f64::INFINITY];
        let mut points = usize::MAX;
        for (_, signal) in self.holders(name) {
            let values = &signal.axis.values;
            let (low, high) = values
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), x| {
                    (low.min(*x), high.max(*x))
                });
            range = [range[0].max(low), range[1].min(high)];
            points = points.min(values.len());
        }
        (range, points)
    }

    /// Axis steps; `false` when a refusal stops the plan.
    fn axis_steps(&mut self, name: &str, signal: &SpectralArray, state: &mut State) -> bool {
        let source_unit = state.unit;
        let wanted_unit = match (&self.options.axis_unit, self.requirements) {
            (Some(unit), Some(requirements))
                if !requirements.accepts_unit(axis_unit(unit).unwrap_or("")) =>
            {
                self.refuse(
                    "axis_unit_refused",
                    format!("{} does not store axes in '{unit}'", requirements.format),
                    "Choose an axis unit the target stores.",
                );
                return false;
            }
            (Some(unit), _) => axis_unit(unit),
            (None, Some(requirements)) if !requirements.accepts_unit(&signal.axis.unit) => {
                requirements.axis_units.first().copied()
            }
            _ => None,
        }
        .filter(|unit| *unit != source_unit);
        let grid_choice = self.options.grid.as_deref();
        let explicit_grid = grid_choice.is_some_and(|grid| grid != "measured");
        let needs_order = wanted_unit.is_some()
            || explicit_grid
            || self.requirements.is_some_and(|requirements| {
                requirements.ascending_axis || requirements.even_axis != EvenAxis::NotRequired
            });
        if needs_order && source_unit.is_empty() {
            self.refuse(
                "axis_not_convertible",
                format!(
                    "'{name}' has a {:?} axis in '{}', which is neither a wavelength nor a wavenumber",
                    signal.axis.kind, signal.axis.unit
                ),
                "Pick a target that stores this axis as it is, such as JCAMP-DX, SPC, CSV or JSON.",
            );
            return false;
        }
        if let Some(unit) = wanted_unit {
            let raman = self.holders(name).any(|(record, signal)| {
                technique(record, &signal.axis, &signal.signal_type) == Technique::Raman
            });
            if raman && (source_unit == "cm-1") != (unit == "cm-1") {
                self.refuse(
                    "raman_shift",
                    format!("'{name}' is a Raman spectrum: its cm-1 axis is a shift from the laser line, not a wavenumber, so it has no wavelength"),
                    "Pick a target that stores Raman shifts in cm-1 (JCAMP-DX, SPC, CSV, JSON).",
                );
                return false;
            }
        }

        // Points out of order are dropped (overlapping detectors) or sorted.
        if needs_order {
            if let Some(axis) = state.axis.clone() {
                let increasing = axis.windows(2).all(|pair| pair[1] > pair[0]);
                let decreasing = axis.windows(2).all(|pair| pair[1] < pair[0]);
                if decreasing && wanted_unit.is_none() {
                    state.axis = Some(axis.iter().rev().copied().collect());
                    self.push(
                        "axis_unit_conversion",
                        Some(AXIS_UNIT_CONVERSION),
                        json!({ "source_unit": source_unit, "target_unit": source_unit, "spectral_density": false }),
                        json!({ "axis": "measured" }),
                        state,
                        "Points are put in increasing axis order; values are unchanged.".to_string(),
                        &[],
                    );
                } else if !increasing && !decreasing && !self.select_points(name, state, &axis) {
                    return false;
                }
            }
        }

        if let Some(unit) = wanted_unit {
            let density = matches!(
                state.signal_type,
                SignalType::Radiance | SignalType::Irradiance
            );
            let limits = [
                convert_limit(state.range[0], source_unit, unit),
                convert_limit(state.range[1], source_unit, unit),
            ];
            state.range = [limits[0].min(limits[1]), limits[0].max(limits[1])];
            state.unit = unit;
            state.axis = None;
            let mut notes = vec![
                "Wavelengths are vacuum wavelengths (nm = 10^7 / cm-1); no correction for air is applied.",
            ];
            if density {
                notes.push("Values are a spectral density, so they are scaled by the derivative of the axis conversion.");
            }
            self.push(
                "axis_unit_conversion",
                Some(AXIS_UNIT_CONVERSION),
                json!({ "source_unit": source_unit, "target_unit": unit, "spectral_density": density }),
                json!({ "axis": "measured" }),
                state,
                format!("Axis converted from {source_unit} to {unit}; points are put in increasing order."),
                &notes,
            );
        }
        self.plan.measured_range = Some(state.range);
        self.resample(state)
    }

    /// Keep the points that extend the axis: the earlier detector up to its
    /// last point, the next one beyond it.
    fn select_points(&mut self, name: &str, state: &mut State, axis: &[f64]) -> bool {
        if self
            .holders(name)
            .any(|(_, signal)| signal.axis.values.as_slice() != axis)
        {
            self.refuse(
                "axes_differ",
                format!("records hold '{name}' on different unordered axes"),
                "Convert the records one file at a time.",
            );
            return false;
        }
        let mut keep: Vec<[usize; 2]> = Vec::new();
        let mut last = f64::NEG_INFINITY;
        for (index, x) in axis.iter().enumerate() {
            if *x > last {
                match keep.last_mut() {
                    Some(range) if range[1] == index => range[1] = index + 1,
                    _ => keep.push([index, index + 1]),
                }
                last = *x;
            }
        }
        let kept: usize = keep.iter().map(|range| range[1] - range[0]).sum();
        if kept * 2 < axis.len() {
            self.refuse(
                "axis_order",
                format!("the axis of '{name}' is not in a repairable order: only {kept} of {} points increase", axis.len()),
                "Pick a target that keeps axes as they are, such as JCAMP-DX, CSV or JSON.",
            );
            return false;
        }
        let selected: Vec<f64> = keep
            .iter()
            .flat_map(|range| axis[range[0]..range[1]].iter().copied())
            .collect();
        state.points = selected.len();
        state.range = [selected[0], selected[selected.len() - 1]];
        state.axis = Some(selected);
        let dropped = axis.len() - kept;
        self.push(
            "select_points",
            None,
            json!({}),
            json!({ "keep": keep }),
            state,
            format!("{dropped} overlapping point(s) are dropped so that the axis increases: each detector is kept beyond the last point of the previous one."),
            &["Values are not changed; the overlap of the next detector is not used."],
        );
        true
    }

    fn resample(&mut self, state: &mut State) -> bool {
        let grid_choice = self.options.grid.clone();
        let requirements = self.requirements;
        let tolerance = self.options.axis_tolerance;
        let measured_ok = |state: &State| -> bool {
            let Some(axis) = &state.axis else {
                return requirements
                    .is_none_or(|requirements| requirements.even_axis == EvenAxis::NotRequired);
            };
            match requirements.map(|requirements| requirements.even_axis) {
                Some(EvenAxis::Segments) => winisi::writable_axis(axis, tolerance),
                Some(EvenAxis::KnownGrid) => nsas::writable_axis(axis, tolerance),
                _ => true,
            }
        };
        let grid = match grid_choice.as_deref() {
            Some("measured") => {
                if !measured_ok(state) {
                    self.refuse(
                        "axis_needs_grid",
                        "the measured axis is not evenly spaced as the target requires".to_string(),
                        "Choose one of the target's grids or an even grid.",
                    );
                    return false;
                }
                return true;
            }
            Some("even") => match self.even_grid(state) {
                Some(grid) => grid,
                None => return false,
            },
            Some(id) => {
                let known = requirements
                    .map(|requirements| requirements.grids.clone())
                    .unwrap_or_default();
                let Some(grid) = known.into_iter().find(|grid| grid.id == id) else {
                    self.refuse(
                        "grid_unknown",
                        format!("'{id}' is not a grid of this target"),
                        "Choose 'measured', 'even' or one of the target's grids.",
                    );
                    return false;
                };
                if !fits(&grid, state.range) {
                    self.refuse(
                        "grid_outside_range",
                        format!(
                            "grid {} ({}-{} {}) is outside the measured range {:.6}-{:.6} {}, and values are never extrapolated",
                            grid.id, grid.first, grid.last, grid.unit, state.range[0], state.range[1], state.unit
                        ),
                        "Choose a grid inside the measured range, or an even grid.",
                    );
                    return false;
                }
                grid
            }
            None => {
                if measured_ok(state) {
                    return true;
                }
                match self.automatic_grid(state) {
                    Some(grid) => grid,
                    None => return false,
                }
            }
        };
        let method = self
            .options
            .interpolation
            .clone()
            .unwrap_or_else(|| "linear".to_string());
        let mut notes =
            vec!["Values are never extrapolated: the grid lies within the measured range."];
        if grid.points < state.points {
            notes.push("The grid has fewer points than the spectrum: interpolation does not average neighbouring points, so narrow bands of high-resolution spectra (FTIR) can alias or be missed.");
        } else {
            notes.push("Interpolation adds no information; it smooths sharp features slightly.");
        }
        if requirements.is_some_and(|requirements| {
            requirements.other_grids == OtherGrids::Warned
                && !requirements.grids.iter().any(|known| known.id == grid.id)
        }) {
            notes.push("Files of this size were never observed for this target and may not open.");
        }
        state.axis = None;
        state.points = grid.points;
        state.range = [grid.first, grid.last];
        self.push(
            "resample",
            Some(RESAMPLER),
            json!({
                "tgt_min": grid.first,
                "tgt_step": grid.step,
                "tgt_n": grid.points,
                "method": method,
                "extrapolate": false,
                "bounds_error": true,
            }),
            json!({ "axis": "current" }),
            state,
            format!(
                "Resampled by {method} interpolation onto {} ({}).",
                grid.id, grid.label
            ),
            &notes,
        );
        self.plan.grid = Some(grid);
        true
    }

    /// Known grid of the target inside the measured range, or an even grid
    /// when the target accepts other grids.
    fn automatic_grid(&mut self, state: &State) -> Option<Grid> {
        let requirements = self.requirements;
        let known: Vec<Grid> = requirements
            .map(|requirements| requirements.grids.clone())
            .unwrap_or_default();
        let spacing = (state.range[1] - state.range[0]) / (state.points.max(2) - 1) as f64;
        let best = known
            .iter()
            .filter(|grid| fits(grid, state.range))
            .min_by(|left, right| {
                let score =
                    |grid: &Grid| ((grid.step / spacing).ln().abs(), -(grid.last - grid.first));
                score(left)
                    .partial_cmp(&score(right))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned();
        if best.is_some() {
            return best;
        }
        let refused = requirements
            .is_some_and(|requirements| requirements.other_grids == OtherGrids::Refused);
        if refused {
            let names: Vec<String> = known.iter().map(|grid| grid.label.clone()).collect();
            self.refuse(
                "no_known_grid",
                format!(
                    "the measured range {:.6}-{:.6} {} covers none of the target's axes ({}), and values are never extrapolated",
                    state.range[0], state.range[1], state.unit, names.join("; ")
                ),
                "Choose a target that accepts other axes, such as FOSS WinISI (.nir) or JCAMP-DX.",
            );
            return None;
        }
        self.even_grid(state)
    }

    fn even_grid(&mut self, state: &State) -> Option<Grid> {
        let step = self.options.grid_step.unwrap_or(match state.unit {
            "um" => 0.002,
            "cm-1" => 4.0,
            _ => 2.0,
        });
        let low = self.options.grid_min.unwrap_or(state.range[0]);
        let high = self.options.grid_max.unwrap_or(state.range[1]);
        if low < state.range[0] || high > state.range[1] {
            self.refuse(
                "grid_outside_range",
                format!(
                    "the grid limits {low}-{high} {} leave the measured range {:.6}-{:.6} {}, and values are never extrapolated",
                    state.unit, state.range[0], state.range[1], state.unit
                ),
                "Choose grid limits inside the measured range.",
            );
            return None;
        }
        let mut first = (low / step).ceil() * step;
        if first < low {
            first += step;
        }
        let mut last = (high / step).floor() * step;
        if last > high {
            last -= step;
        }
        let points = ((last - first) / step).round() as i64 + 1;
        if points < 2 {
            self.refuse(
                "grid_too_short",
                format!("an even grid by {step} {} holds fewer than two points between {low} and {high}", state.unit),
                "Choose a smaller grid step or wider limits.",
            );
            return None;
        }
        let points = points as usize;
        let last = first + step * (points - 1) as f64;
        Some(Grid {
            id: "even".to_string(),
            label: format!(
                "{points} points, {first}-{last} {} by {step} {}",
                state.unit, state.unit
            ),
            unit: state.unit,
            first,
            step,
            last,
            points,
        })
    }

    fn check_output(&mut self, state: &State) {
        let Some(requirements) = self.requirements else {
            return;
        };
        if let Some(max) = requirements.max_points {
            if state.points > max {
                self.refuse(
                    "too_many_points",
                    format!(
                        "{} holds at most {max} points per spectrum; this one has {}",
                        requirements.format, state.points
                    ),
                    "Choose a coarser even grid.",
                );
            }
        }
        if !requirements.missing_values {
            self.plan.notes.push(format!(
                "{} cannot store missing values: if a step produces any, the export is refused; narrow the grid range to exclude them.",
                requirements.format
            ));
        }
    }

    fn output_name(&self, name: &str, signal: &SpectralArray, state: &State) -> String {
        let base = if state.signal_type != signal.signal_type {
            format!("{} (from {name})", signal_label(&state.signal_type))
        } else {
            format!("{name} (converted)")
        };
        let names = self.signal_names();
        let mut candidate = base.clone();
        let mut index = 2;
        while names.contains(&candidate) {
            candidate = format!("{base} {index}");
            index += 1;
        }
        candidate
    }
}

/// Whether a grid lies within a measured range.
fn fits(grid: &Grid, range: [f64; 2]) -> bool {
    grid.first >= range[0] && grid.last <= range[1]
}

/// Lower-case words of a signal's name and role.
fn words(name: &str, signal: &SpectralArray) -> Vec<String> {
    format!("{} {}", name, signal.role)
        .to_ascii_lowercase()
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}

fn has_word(name: &str, signal: &SpectralArray, wanted: &[&str]) -> bool {
    words(name, signal)
        .iter()
        .any(|word| wanted.contains(&word.as_str()))
}

/// References are named or given roles such as `white_reference`,
/// `dark_reference`, `reference`, `Reference Spectrum` or `Norm. DN (Ref.)`.
fn is_reference(name: &str, signal: &SpectralArray) -> bool {
    has_word(name, signal, &["reference", "ref", "white", "dark"])
}

fn is_dark(name: &str, signal: &SpectralArray) -> bool {
    has_word(name, signal, &["dark"])
}

fn described(signal: &SpectralArray) -> String {
    let label = signal_label(&signal.signal_type);
    match scale_of(signal) {
        Scale::Percent => format!("percent {label}"),
        Scale::Other(unit) => format!("{label} ({unit})"),
        Scale::Plain => label.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_limits_convert_both_ways() {
        assert_eq!(convert_limit(1000.0, "nm", "cm-1"), 10_000.0);
        assert_eq!(convert_limit(10_000.0, "cm-1", "nm"), 1000.0);
        assert_eq!(convert_limit(2.5, "um", "nm"), 2500.0);
        assert_eq!(axis_unit("1/cm"), Some("cm-1"));
        assert_eq!(axis_unit("index"), None);
    }
}
