use serde::{Deserialize, Serialize};

/// Canonical signal type shared across all loaders and bindings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    Absorbance,
    Reflectance,
    Transmittance,
    Radiance,
    Irradiance,
    RawCounts,
    SingleBeam,
    Interferogram,
    KubelkaMunk,
    Derivative,
    Preprocessed,
    AerosolOpticalThickness,
    Uncertainty,
    Unknown,
}
