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
    Energy,
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
        if values.iter().any(|value| !value.is_finite()) {
            return Err(Error::InvalidRecord(
                "spectral axis coordinates must be finite".to_string(),
            ));
        }

        let order = detect_order(&values);
        Ok(Self {
            values,
            unit: unit.into(),
            kind,
            order,
        })
    }

    /// Build a 0-based integer index axis of length `n` (`Index` kind,
    /// ascending). Used as the coordinate for non-spectral dimensions
    /// that an instrument leaves uncalibrated (e.g. a spatial pixel row).
    pub fn index(n: usize) -> Self {
        Self {
            values: (0..n).map(|i| i as f64).collect(),
            unit: "index".to_string(),
            kind: AxisKind::Index,
            order: AxisOrder::Ascending,
        }
    }
}

/// One named signal channel.
///
/// The canonical layout is N-dimensional and lossless: `values` is a flat,
/// C-order (row-major) buffer of `product(shape)` elements, `dims` names each
/// axis, and `axes` carries one coordinate per dimension. Exactly one
/// dimension is the spectral axis (named `"x"`); its coordinate is exposed
/// directly as [`SpectralArray::axis`] so the common 1-D spectrum stays
/// ergonomic, while non-spectral dimensions (e.g. spatial rows, a time
/// series) keep their coordinate in [`SpectralArray::coords`].
///
/// A 1-D spectrum is just the trivial case: `shape == [n]`, `dims == ["x"]`,
/// `coords` empty.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpectralArray {
    /// Coordinate of the spectral (`"x"`) dimension.
    pub axis: SpectralAxis,
    /// Flat, C-order signal buffer; `values.len() == product(shape)`.
    pub values: Vec<f64>,
    /// Per-dimension extent; `shape.len() == dims.len()`.
    pub shape: Vec<usize>,
    /// Dimension names; unique, non-empty, exactly one is `"x"`.
    pub dims: Vec<String>,
    /// Coordinates for the non-spectral dimensions, keyed by dim name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub coords: BTreeMap<String, SpectralAxis>,
    pub signal_type: SignalType,
    pub unit: Option<String>,
    pub role: String,
    pub source: String,
}

impl SpectralArray {
    /// Construct a 1-D spectral channel. `dims` must be exactly `["x"]` and
    /// `values.len()` must equal `axis.values.len()`. For multi-dimensional
    /// signals use [`SpectralArray::new_nd`].
    pub fn new(
        axis: SpectralAxis,
        values: Vec<f64>,
        dims: Vec<String>,
        signal_type: SignalType,
        unit: Option<String>,
        role: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<Self> {
        if dims.len() != 1 || dims[0] != "x" {
            return Err(Error::InvalidRecord(
                "1-D spectral array dims must be exactly [\"x\"]; use new_nd for multi-dimensional signals".to_string(),
            ));
        }
        if values.is_empty() {
            return Err(Error::InvalidRecord(
                "spectral array values are empty".to_string(),
            ));
        }
        if values.len() != axis.values.len() {
            return Err(Error::InvalidRecord(
                "1-D spectral array values length must equal its axis length".to_string(),
            ));
        }

        let shape = vec![axis.values.len()];
        Ok(Self {
            axis,
            values,
            shape,
            dims,
            coords: BTreeMap::new(),
            signal_type,
            unit,
            role: role.into(),
            source: source.into(),
        })
    }

    /// Construct an N-dimensional spectral channel.
    ///
    /// `dims` names each dimension (unique, non-empty, exactly one `"x"`),
    /// `shape` gives each extent (all `> 0`), `axis` is the coordinate of the
    /// `"x"` dimension, and `coords` carries one coordinate per non-`"x"`
    /// dimension. `values` is the flat C-order buffer with
    /// `values.len() == product(shape)`.
    #[allow(clippy::too_many_arguments)]
    pub fn new_nd(
        shape: Vec<usize>,
        dims: Vec<String>,
        axis: SpectralAxis,
        coords: BTreeMap<String, SpectralAxis>,
        values: Vec<f64>,
        signal_type: SignalType,
        unit: Option<String>,
        role: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<Self> {
        if shape.is_empty() || shape.len() != dims.len() {
            return Err(Error::InvalidRecord(
                "spectral array shape and dims must be non-empty and equal length".to_string(),
            ));
        }
        if shape.contains(&0) {
            return Err(Error::InvalidRecord(
                "spectral array shape extents must all be greater than zero".to_string(),
            ));
        }
        let unique: std::collections::BTreeSet<&String> = dims.iter().collect();
        if unique.len() != dims.len() || dims.iter().any(|dim| dim.is_empty()) {
            return Err(Error::InvalidRecord(
                "spectral array dims must be unique and non-empty".to_string(),
            ));
        }
        if dims.iter().filter(|dim| dim.as_str() == "x").count() != 1 {
            return Err(Error::InvalidRecord(
                "spectral array dims must contain exactly one 'x' dimension".to_string(),
            ));
        }
        let expected = shape
            .iter()
            .try_fold(1usize, |acc, extent| acc.checked_mul(*extent))
            .ok_or_else(|| {
                Error::InvalidRecord("spectral array shape product overflows usize".to_string())
            })?;
        if values.len() != expected {
            return Err(Error::InvalidRecord(format!(
                "spectral array values length {} does not match shape product {expected}",
                values.len()
            )));
        }
        let x_index = dims
            .iter()
            .position(|dim| dim == "x")
            .expect("validated above");
        if axis.values.len() != shape[x_index] {
            return Err(Error::InvalidRecord(
                "spectral 'x' axis length must match its shape extent".to_string(),
            ));
        }
        for (dim_index, dim) in dims.iter().enumerate() {
            if dim == "x" {
                continue;
            }
            match coords.get(dim) {
                Some(coord) if coord.values.len() == shape[dim_index] => {}
                Some(_) => {
                    return Err(Error::InvalidRecord(format!(
                        "coordinate for dim '{dim}' length must match its shape extent"
                    )));
                }
                None => {
                    return Err(Error::InvalidRecord(format!(
                        "missing coordinate for non-spectral dim '{dim}'"
                    )));
                }
            }
        }

        Ok(Self {
            axis,
            values,
            shape,
            dims,
            coords,
            signal_type,
            unit,
            role: role.into(),
            source: source.into(),
        })
    }

    /// Number of dimensions.
    pub fn ndim(&self) -> usize {
        self.shape.len()
    }

    /// Whether this is a plain 1-D spectrum (`dims == ["x"]`).
    pub fn is_1d(&self) -> bool {
        self.shape.len() == 1
    }

    /// Position of the spectral (`"x"`) dimension within `dims`/`shape`.
    pub fn x_dim_index(&self) -> usize {
        self.dims
            .iter()
            .position(|dim| dim == "x")
            .expect("validated: exactly one 'x' dimension")
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
    fn new_1d_rejects_non_x_dims() {
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
        assert!(err.to_string().contains("new_nd"));
    }

    #[test]
    fn new_1d_requires_matching_axis_length() {
        let axis = SpectralAxis::new(vec![1.0, 2.0, 3.0], "nm", AxisKind::Wavelength).unwrap();
        let err = SpectralArray::new(
            axis,
            vec![0.1, 0.2],
            vec!["x".to_string()],
            SignalType::Absorbance,
            None,
            "absorbance",
            "file",
        )
        .unwrap_err();
        assert!(err.to_string().contains("equal its axis length"));
    }

    #[test]
    fn new_1d_sets_trivial_shape() {
        let axis = SpectralAxis::new(vec![1.0, 2.0, 3.0], "nm", AxisKind::Wavelength).unwrap();
        let array = SpectralArray::new(
            axis,
            vec![0.1, 0.2, 0.3],
            vec!["x".to_string()],
            SignalType::Absorbance,
            None,
            "absorbance",
            "file",
        )
        .unwrap();
        assert!(array.is_1d());
        assert_eq!(array.shape, vec![3]);
        assert_eq!(array.x_dim_index(), 0);
        assert!(array.coords.is_empty());
    }

    fn nd_2x3() -> (SpectralAxis, BTreeMap<String, SpectralAxis>) {
        let x = SpectralAxis::new(vec![900.0, 1000.0, 1100.0], "nm", AxisKind::Wavelength).unwrap();
        let mut coords = BTreeMap::new();
        coords.insert("y".to_string(), SpectralAxis::index(2));
        (x, coords)
    }

    #[test]
    fn new_nd_accepts_valid_cube_slice() {
        let (x, coords) = nd_2x3();
        let array = SpectralArray::new_nd(
            vec![2, 3],
            vec!["y".to_string(), "x".to_string()],
            x,
            coords,
            vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            SignalType::RawCounts,
            None,
            "intensity",
            "file",
        )
        .unwrap();
        assert_eq!(array.ndim(), 2);
        assert_eq!(array.x_dim_index(), 1);
        assert_eq!(array.coords["y"].values.len(), 2);
    }

    #[test]
    fn new_nd_rejects_shape_product_mismatch() {
        let (x, coords) = nd_2x3();
        let err = SpectralArray::new_nd(
            vec![2, 3],
            vec!["y".to_string(), "x".to_string()],
            x,
            coords,
            vec![0.0, 1.0, 2.0, 3.0, 4.0], // 5 != 2*3
            SignalType::RawCounts,
            None,
            "intensity",
            "file",
        )
        .unwrap_err();
        assert!(err.to_string().contains("shape product"));
    }

    #[test]
    fn new_nd_rejects_missing_coord() {
        let x = SpectralAxis::new(vec![900.0, 1000.0, 1100.0], "nm", AxisKind::Wavelength).unwrap();
        let err = SpectralArray::new_nd(
            vec![2, 3],
            vec!["y".to_string(), "x".to_string()],
            x,
            BTreeMap::new(), // no coord for "y"
            vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            SignalType::RawCounts,
            None,
            "intensity",
            "file",
        )
        .unwrap_err();
        assert!(err.to_string().contains("missing coordinate"));
    }

    #[test]
    fn new_nd_rejects_duplicate_dims() {
        let x = SpectralAxis::new(vec![900.0, 1000.0], "nm", AxisKind::Wavelength).unwrap();
        let mut coords = BTreeMap::new();
        coords.insert("x".to_string(), SpectralAxis::index(2));
        let err = SpectralArray::new_nd(
            vec![2, 2],
            vec!["x".to_string(), "x".to_string()],
            x,
            coords,
            vec![0.0, 1.0, 2.0, 3.0],
            SignalType::RawCounts,
            None,
            "intensity",
            "file",
        )
        .unwrap_err();
        assert!(err.to_string().contains("unique"));
    }

    #[test]
    fn axis_rejects_non_finite() {
        let err = SpectralAxis::new(vec![1.0, f64::NAN], "nm", AxisKind::Wavelength).unwrap_err();
        assert!(err.to_string().contains("finite"));
    }
}
