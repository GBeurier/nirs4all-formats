//! CF bounds variables.
//!
//! Many coordinate variables have associated bounds variables that define
//! the extent of each cell. The `bounds` attribute on a coordinate variable
//! names the bounds variable, which has an extra trailing dimension of size 2
//! (or `nv` for climatological bounds).

use crate::types::{NcGroup, NcVariable};

/// A pair of lower and upper cell bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellBounds {
    pub lower: f64,
    pub upper: f64,
}

/// Find the bounds variable for a coordinate variable.
///
/// Looks for a `bounds` attribute on the variable and resolves it
/// within the given group.
pub fn find_bounds_variable<'a>(var: &NcVariable, group: &'a NcGroup) -> Option<&'a NcVariable> {
    let bounds_name = var.attribute("bounds")?.value.as_string()?;
    group.variables.iter().find(|v| v.name == bounds_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NcAttrValue, NcAttribute, NcDimension, NcType};

    #[test]
    fn finds_bounds_variable_from_bounds_attribute() {
        let bounds_var = NcVariable {
            name: "lat_bnds".into(),
            dimensions: vec![
                NcDimension {
                    name: "lat".into(),
                    size: 3,
                    is_unlimited: false,
                },
                NcDimension {
                    name: "nv".into(),
                    size: 2,
                    is_unlimited: false,
                },
            ],
            dtype: NcType::Double,
            attributes: vec![],
            data_offset: 0,
            _data_size: 0,
            is_record_var: false,
            record_size: 0,
        };

        let coord_var = NcVariable {
            name: "lat".into(),
            dimensions: vec![NcDimension {
                name: "lat".into(),
                size: 3,
                is_unlimited: false,
            }],
            dtype: NcType::Double,
            attributes: vec![NcAttribute {
                name: "bounds".into(),
                value: NcAttrValue::Chars("lat_bnds".into()),
            }],
            data_offset: 0,
            _data_size: 0,
            is_record_var: false,
            record_size: 0,
        };

        let group = NcGroup {
            name: "/".into(),
            dimensions: vec![],
            variables: vec![coord_var.clone(), bounds_var],
            attributes: vec![],
            groups: vec![],
        };

        let found = find_bounds_variable(&coord_var, &group).unwrap();
        assert_eq!(found.name, "lat_bnds");
    }

    #[test]
    fn returns_none_without_bounds_attribute() {
        let var = NcVariable {
            name: "lat".into(),
            dimensions: vec![],
            dtype: NcType::Double,
            attributes: vec![],
            data_offset: 0,
            _data_size: 0,
            is_record_var: false,
            record_size: 0,
        };

        let group = NcGroup {
            name: "/".into(),
            dimensions: vec![],
            variables: vec![var.clone()],
            attributes: vec![],
            groups: vec![],
        };

        assert!(find_bounds_variable(&var, &group).is_none());
    }
}
