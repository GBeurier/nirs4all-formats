use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::SignalType;

/// Semantic kind of a spectral axis.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AxisKind {
    Wavelength,
    Wavenumber,
    Frequency,
    Time,
    Index,
}

/// Native ordering of the axis values as stored by the instrument.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AxisOrder {
    Ascending,
    Descending,
    NonMonotonic,
}

/// A spectral axis local to one signal channel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpectralAxis {
    pub values: Vec<f64>,
    pub unit: String,
    pub kind: AxisKind,
    pub order: AxisOrder,
}

impl SpectralAxis {
    pub fn new(values: Vec<f64>, unit: impl Into<String>, kind: AxisKind) -> Result<Self> {
        if values.is_empty() {
            return Err(Error::InvalidRecord("spectral axis is empty".to_string()));
        }

        let order = detect_order(&values);
        Ok(Self {
            values,
            unit: unit.into(),
            kind,
            order,
        })
    }
}

/// One named signal channel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpectralArray {
    pub axis: SpectralAxis,
    pub values: Vec<f64>,
    pub dims: Vec<String>,
    pub signal_type: SignalType,
    pub unit: Option<String>,
    pub role: String,
    pub source: String,
}

impl SpectralArray {
    pub fn new(
        axis: SpectralAxis,
        values: Vec<f64>,
        dims: Vec<String>,
        signal_type: SignalType,
        unit: Option<String>,
        role: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<Self> {
        let x_dims = dims.iter().filter(|dim| dim.as_str() == "x").count();
        if x_dims != 1 {
            return Err(Error::InvalidRecord(
                "spectral array dims must contain exactly one 'x' dimension".to_string(),
            ));
        }
        if values.is_empty() {
            return Err(Error::InvalidRecord(
                "spectral array values are empty".to_string(),
            ));
        }
        if values.len() < axis.values.len() {
            return Err(Error::InvalidRecord(
                "spectral array has fewer values than its axis".to_string(),
            ));
        }

        Ok(Self {
            axis,
            values,
            dims,
            signal_type,
            unit,
            role: role.into(),
            source: source.into(),
        })
    }
}

/// One physical input file or archive member.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceFile {
    pub path: String,
    pub archive: Option<String>,
    pub sha256: String,
    pub role: String,
}

impl SourceFile {
    pub fn from_bytes(path: impl AsRef<Path>, bytes: &[u8], role: impl Into<String>) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Self {
            path: path.as_ref().to_string_lossy().into_owned(),
            archive: None,
            sha256: format!("{:x}", hasher.finalize()),
            role: role.into(),
        }
    }

    pub fn from_path(path: impl AsRef<Path>, role: impl Into<String>) -> Result<Self> {
        let path_ref = path.as_ref();
        let bytes = std::fs::read(path_ref).map_err(|source| Error::Io {
            path: PathBuf::from(path_ref),
            source,
        })?;
        Ok(Self::from_bytes(path_ref, &bytes, role))
    }
}

/// Parser provenance attached to every record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    pub format: String,
    pub reader: String,
    pub reader_version: String,
    pub sources: Vec<SourceFile>,
    pub parsed_at_utc: Option<String>,
    pub record_schema_version: String,
    pub warnings: Vec<String>,
}

/// Normalized representation emitted by every loader.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpectralRecord {
    pub signals: BTreeMap<String, SpectralArray>,
    pub signal_type: SignalType,
    pub targets: BTreeMap<String, serde_json::Value>,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub provenance: Provenance,
    pub quality_flags: Vec<String>,
}

impl SpectralRecord {
    pub fn validate(&self) -> Result<()> {
        if self.signals.is_empty() {
            return Err(Error::InvalidRecord(
                "record must contain at least one signal".to_string(),
            ));
        }
        if self.provenance.sources.is_empty() {
            return Err(Error::InvalidRecord(
                "record provenance must contain at least one source".to_string(),
            ));
        }
        Ok(())
    }
}

fn detect_order(values: &[f64]) -> AxisOrder {
    let mut asc = true;
    let mut desc = true;
    for pair in values.windows(2) {
        asc &= pair[0] <= pair[1];
        desc &= pair[0] >= pair[1];
    }
    match (asc, desc) {
        (true, false) => AxisOrder::Ascending,
        (false, true) => AxisOrder::Descending,
        (true, true) => AxisOrder::Ascending,
        (false, false) => AxisOrder::NonMonotonic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_axis_order() {
        let axis = SpectralAxis::new(vec![900.0, 1000.0, 1100.0], "nm", AxisKind::Wavelength)
            .expect("axis");
        assert_eq!(axis.order, AxisOrder::Ascending);

        let axis = SpectralAxis::new(vec![9000.0, 8500.0, 8000.0], "cm-1", AxisKind::Wavenumber)
            .expect("axis");
        assert_eq!(axis.order, AxisOrder::Descending);
    }

    #[test]
    fn rejects_signal_without_x_dim() {
        let axis = SpectralAxis::new(vec![1.0, 2.0], "nm", AxisKind::Wavelength).unwrap();
        let err = SpectralArray::new(
            axis,
            vec![0.1, 0.2],
            vec!["sample".to_string()],
            SignalType::Absorbance,
            None,
            "absorbance",
            "file",
        )
        .unwrap_err();
        assert!(err.to_string().contains("'x'"));
    }
}
