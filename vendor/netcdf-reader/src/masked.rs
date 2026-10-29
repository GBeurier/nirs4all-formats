//! _FillValue / missing_value masking for NetCDF variables.
//!
//! Replaces fill/missing values with NaN according to CF conventions.
//! Checks `_FillValue`, `missing_value`, `valid_min`, `valid_max`, and `valid_range`.

use ndarray::ArrayD;

use crate::types::NcVariable;

/// Parameters for masking invalid data.
#[derive(Debug, Clone)]
pub struct MaskParams {
    pub fill_value: Option<f64>,
    pub missing_value: Option<f64>,
    pub valid_min: Option<f64>,
    pub valid_max: Option<f64>,
}

impl MaskParams {
    /// Extract masking parameters from a variable's attributes.
    ///
    /// Returns `None` if no masking attributes are present.
    pub fn from_variable(var: &NcVariable) -> Option<Self> {
        let fill = var.attribute("_FillValue").and_then(|a| a.value.as_f64());
        let missing = var
            .attribute("missing_value")
            .and_then(|a| a.value.as_f64());

        // valid_range takes precedence over valid_min/valid_max individually
        let (vmin, vmax) = if let Some(range) = var
            .attribute("valid_range")
            .and_then(|a| a.value.as_f64_vec())
        {
            if range.len() >= 2 {
                (Some(range[0]), Some(range[1]))
            } else {
                (None, None)
            }
        } else {
            let vmin = var.attribute("valid_min").and_then(|a| a.value.as_f64());
            let vmax = var.attribute("valid_max").and_then(|a| a.value.as_f64());
            (vmin, vmax)
        };

        if fill.is_none() && missing.is_none() && vmin.is_none() && vmax.is_none() {
            return None;
        }

        Some(MaskParams {
            fill_value: fill,
            missing_value: missing,
            valid_min: vmin,
            valid_max: vmax,
        })
    }

    /// Replace fill/missing values with NaN and mask values outside valid range.
    ///
    /// Uses bit-exact comparison for fill/missing values to correctly handle
    /// NaN fill values (since `NaN != NaN` with normal `==`).
    pub fn apply(&self, data: &mut ArrayD<f64>) {
        data.mapv_inplace(|v| {
            if let Some(fill) = self.fill_value {
                if v.to_bits() == fill.to_bits() {
                    return f64::NAN;
                }
            }
            if let Some(miss) = self.missing_value {
                if v.to_bits() == miss.to_bits() {
                    return f64::NAN;
                }
            }
            if let Some(vmin) = self.valid_min {
                if v < vmin {
                    return f64::NAN;
                }
            }
            if let Some(vmax) = self.valid_max {
                if v > vmax {
                    return f64::NAN;
                }
            }
            v
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NcAttrValue, NcAttribute, NcType, NcVariable};
    use ndarray::arr1;

    fn make_var(attrs: Vec<NcAttribute>) -> NcVariable {
        NcVariable {
            name: "test".into(),
            dimensions: vec![],
            dtype: NcType::Float,
            attributes: attrs,
            data_offset: 0,
            _data_size: 0,
            is_record_var: false,
            record_size: 0,
        }
    }

    #[test]
    fn no_mask_attrs() {
        let var = make_var(vec![]);
        assert!(MaskParams::from_variable(&var).is_none());
    }

    #[test]
    fn fill_value() {
        let var = make_var(vec![NcAttribute {
            name: "_FillValue".into(),
            value: NcAttrValue::Floats(vec![-9999.0]),
        }]);
        let params = MaskParams::from_variable(&var).unwrap();
        let mut data = arr1(&[1.0, -9999.0, 3.0]).into_dyn();
        params.apply(&mut data);
        assert_eq!(data[[0]], 1.0);
        assert!(data[[1]].is_nan());
        assert_eq!(data[[2]], 3.0);
    }

    #[test]
    fn missing_value() {
        let var = make_var(vec![NcAttribute {
            name: "missing_value".into(),
            value: NcAttrValue::Doubles(vec![-999.0]),
        }]);
        let params = MaskParams::from_variable(&var).unwrap();
        let mut data = arr1(&[-999.0, 5.0]).into_dyn();
        params.apply(&mut data);
        assert!(data[[0]].is_nan());
        assert_eq!(data[[1]], 5.0);
    }

    #[test]
    fn valid_range() {
        let var = make_var(vec![NcAttribute {
            name: "valid_range".into(),
            value: NcAttrValue::Doubles(vec![0.0, 100.0]),
        }]);
        let params = MaskParams::from_variable(&var).unwrap();
        let mut data = arr1(&[-5.0, 50.0, 150.0]).into_dyn();
        params.apply(&mut data);
        assert!(data[[0]].is_nan());
        assert_eq!(data[[1]], 50.0);
        assert!(data[[2]].is_nan());
    }

    #[test]
    fn valid_min_max() {
        let var = make_var(vec![
            NcAttribute {
                name: "valid_min".into(),
                value: NcAttrValue::Doubles(vec![0.0]),
            },
            NcAttribute {
                name: "valid_max".into(),
                value: NcAttrValue::Doubles(vec![50.0]),
            },
        ]);
        let params = MaskParams::from_variable(&var).unwrap();
        let mut data = arr1(&[-1.0, 25.0, 51.0]).into_dyn();
        params.apply(&mut data);
        assert!(data[[0]].is_nan());
        assert_eq!(data[[1]], 25.0);
        assert!(data[[2]].is_nan());
    }
}
