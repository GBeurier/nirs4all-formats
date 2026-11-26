//! What each writer accepts, as data that hosts can show and planners can use.
//!
//! The writers enforce these rules themselves; this table only describes them.

use nirs4all_formats_core::SignalType;
use serde::Serialize;

use super::nsas;

/// Evenly spaced axis rule of a writer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvenAxis {
    /// Any axis is written (explicit coordinates when uneven).
    NotRequired,
    /// Up to `max_segments` evenly spaced segments.
    Segments,
    /// Only one of the listed `grids`.
    KnownGrid,
}

/// How a writer treats an evenly spaced axis that is not one of its `grids`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OtherGrids {
    Accepted,
    /// Written with a declared loss: files of that size were never observed.
    Warned,
    Refused,
}

/// A spectral axis a target is known to hold.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Grid {
    pub id: String,
    pub label: String,
    pub unit: &'static str,
    pub first: f64,
    pub step: f64,
    pub last: f64,
    pub points: usize,
}

/// Requirements of one writer.
#[derive(Clone, Debug, Serialize)]
pub struct TargetRequirements {
    pub format: &'static str,
    /// Signal types written as such; empty when every type is written.
    pub signal_types: Vec<SignalType>,
    /// Signal types the writer refuses or leaves out.
    pub refused_signal_types: Vec<SignalType>,
    /// Whether percent ordinates are accepted (and labelled as percent).
    pub percent: bool,
    /// Axis units written; empty when every unit is written.
    pub axis_units: Vec<&'static str>,
    /// Whether the axis must be strictly increasing.
    pub ascending_axis: bool,
    pub even_axis: EvenAxis,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_segments: Option<usize>,
    /// Axes observed in files of the format.
    pub grids: Vec<Grid>,
    pub other_grids: OtherGrids,
    /// Largest number of points per spectrum.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_points: Option<usize>,
    /// Whether missing (non-finite) values can be stored.
    pub missing_values: bool,
    /// Whether the applied conversions are written into the file.
    pub records_conversions: bool,
    pub notes: &'static str,
}

impl TargetRequirements {
    fn any(format: &'static str, missing_values: bool) -> Self {
        Self {
            format,
            signal_types: Vec::new(),
            refused_signal_types: Vec::new(),
            percent: true,
            axis_units: Vec::new(),
            ascending_axis: false,
            even_axis: EvenAxis::NotRequired,
            max_segments: None,
            grids: Vec::new(),
            other_grids: OtherGrids::Accepted,
            max_points: None,
            missing_values,
            records_conversions: false,
            notes: "",
        }
    }

    fn foss(format: &'static str) -> Self {
        Self {
            signal_types: vec![SignalType::Absorbance],
            percent: false,
            axis_units: vec!["nm"],
            ascending_axis: true,
            ..Self::any(format, false)
        }
    }

    /// Whether a signal type is written as such.
    pub fn accepts_signal(&self, signal_type: &SignalType) -> bool {
        !self.refused_signal_types.contains(signal_type)
            && (self.signal_types.is_empty() || self.signal_types.contains(signal_type))
    }

    /// Whether an axis unit is written as such.
    pub fn accepts_unit(&self, unit: &str) -> bool {
        self.axis_units.is_empty() || self.axis_units.contains(&unit)
    }
}

/// WinISI files hold these axes; others are written with a warning.
pub(crate) fn winisi_grids() -> Vec<Grid> {
    [(1100.0, 700), (400.0, 1050)]
        .into_iter()
        .map(|(first, points)| {
            let last = first + 2.0 * (points - 1) as f64;
            Grid {
                id: format!("winisi-{points}"),
                label: format!("{points} points, {first}-{last} nm by 2 nm"),
                unit: "nm",
                first,
                step: 2.0,
                last,
                points,
            }
        })
        .collect()
}

/// Requirements of a writer format; `None` for an unknown format.
pub fn target_requirements(format: &str) -> Option<TargetRequirements> {
    let requirements = match format {
        "jcamp-dx" => TargetRequirements {
            refused_signal_types: vec![SignalType::Interferogram],
            records_conversions: true,
            notes: "Interferograms are not written; uneven axes use explicit XY points.",
            ..TargetRequirements::any("jcamp-dx", false)
        },
        "galactic-spc" => TargetRequirements {
            records_conversions: true,
            ..TargetRequirements::any("galactic-spc", false)
        },
        "csv" => TargetRequirements::any("csv", true),
        "xlsx" => TargetRequirements {
            max_points: Some(16_384),
            notes: "An Excel sheet holds 16384 columns, identifier and targets included.",
            ..TargetRequirements::any("xlsx", true)
        },
        "xy-text" => TargetRequirements::any("xy-text", false),
        "bruker-dpt" => TargetRequirements {
            axis_units: vec!["cm-1"],
            ..TargetRequirements::any("bruker-dpt", false)
        },
        "foss-winisi-nir" | "foss-winisi-cal" => TargetRequirements {
            even_axis: EvenAxis::Segments,
            max_segments: Some(7),
            grids: winisi_grids(),
            other_grids: OtherGrids::Warned,
            max_points: Some(65_535),
            notes: if format == "foss-winisi-cal" {
                "Absorbance (log 1/R) along nm; the .cal file needs numeric reference values."
            } else {
                "Absorbance (log 1/R) along nm."
            },
            ..TargetRequirements::foss(if format == "foss-winisi-cal" {
                "foss-winisi-cal"
            } else {
                "foss-winisi-nir"
            })
        },
        "foss-nsas-da" => TargetRequirements {
            even_axis: EvenAxis::KnownGrid,
            grids: nsas::grids(),
            other_grids: OtherGrids::Refused,
            notes: "Absorbance (log 1/R) on one of the four known NSAS axes.",
            ..TargetRequirements::foss("foss-nsas-da")
        },
        "records-json" => TargetRequirements {
            records_conversions: true,
            ..TargetRequirements::any("records-json", true)
        },
        _ => return None,
    };
    Some(requirements)
}

/// Requirements of every writer, in catalog order.
pub fn writer_requirements() -> Vec<TargetRequirements> {
    super::writer_catalog()
        .iter()
        .filter_map(|entry| target_requirements(entry.format))
        .collect()
}
