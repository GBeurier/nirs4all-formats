//! scale_factor / add_offset unpacking for NetCDF variables.
//!
//! The CF convention defines:
//!   `actual_value = stored_value * scale_factor + add_offset`
//!
//! Defaults: `scale_factor = 1.0`, `add_offset = 0.0`.

use ndarray::ArrayD;

use crate::types::NcVariable;

/// Parameters for unpacking packed variable data.
#[derive(Debug, Clone, Copy)]
pub struct UnpackParams {
    pub scale_factor: f64,
    pub add_offset: f64,
}

impl UnpackParams {
    /// Extract unpacking parameters from a variable's attributes.
    ///
    /// Returns `None` if neither `scale_factor` nor `add_offset` is present
    /// (i.e., no unpacking needed).
    pub fn from_variable(var: &NcVariable) -> Option<Self> {
        let scale = var.attribute("scale_factor").and_then(|a| a.value.as_f64());
        let offset = var.attribute("add_offset").and_then(|a| a.value.as_f64());

        if scale.is_none() && offset.is_none() {
            return None;
        }

        Some(UnpackParams {
            scale_factor: scale.unwrap_or(1.0),
            add_offset: offset.unwrap_or(0.0),
        })
    }

    /// Apply unpacking: `actual = stored * scale_factor + add_offset`.
    pub fn apply(&self, data: &mut ArrayD<f64>) {
        if self.scale_factor == 1.0 && self.add_offset == 0.0 {
            return;
        }
        data.mapv_inplace(|v| v * self.scale_factor + self.add_offset);
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
            dtype: NcType::Int,
            attributes: attrs,
            data_offset: 0,
            _data_size: 0,
            is_record_var: false,
            record_size: 0,
        }
    }

    #[test]
    fn no_unpack_attrs() {
        let var = make_var(vec![]);
        assert!(UnpackParams::from_variable(&var).is_none());
    }

    #[test]
    fn scale_only() {
        let var = make_var(vec![NcAttribute {
            name: "scale_factor".into(),
            value: NcAttrValue::Doubles(vec![0.01]),
        }]);
        let params = UnpackParams::from_variable(&var).unwrap();
        assert_eq!(params.scale_factor, 0.01);
        assert_eq!(params.add_offset, 0.0);
    }

    #[test]
    fn offset_only() {
        let var = make_var(vec![NcAttribute {
            name: "add_offset".into(),
            value: NcAttrValue::Doubles(vec![273.15]),
        }]);
        let params = UnpackParams::from_variable(&var).unwrap();
        assert_eq!(params.scale_factor, 1.0);
        assert_eq!(params.add_offset, 273.15);
    }

    #[test]
    fn both() {
        let var = make_var(vec![
            NcAttribute {
                name: "scale_factor".into(),
                value: NcAttrValue::Doubles(vec![0.1]),
            },
            NcAttribute {
                name: "add_offset".into(),
                value: NcAttrValue::Doubles(vec![10.0]),
            },
        ]);
        let params = UnpackParams::from_variable(&var).unwrap();
        let mut data = arr1(&[100.0, 200.0, 300.0]).into_dyn();
        params.apply(&mut data);
        assert_eq!(data[[0]], 20.0); // 100 * 0.1 + 10
        assert_eq!(data[[1]], 30.0); // 200 * 0.1 + 10
        assert_eq!(data[[2]], 40.0); // 300 * 0.1 + 10
    }
}
