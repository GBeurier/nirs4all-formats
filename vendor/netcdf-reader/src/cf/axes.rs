//! CF axis identification.
//!
//! Determines the role of each coordinate variable (T, X, Y, Z) based on:
//! - The `axis` attribute (most explicit)
//! - The `standard_name` attribute (e.g., "latitude", "longitude", "time")
//! - The `units` attribute (e.g., "degrees_north", "degrees_east")
//! - The `positive` attribute for vertical axes
//!
//! Priority follows CF Conventions Table 1: axis > standard_name > units > positive.

use crate::types::{NcDimension, NcGroup, NcVariable};

/// The axis role of a coordinate variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CfAxisType {
    /// Time axis.
    T,
    /// Longitude / easting axis.
    X,
    /// Latitude / northing axis.
    Y,
    /// Vertical axis.
    Z,
    /// Cannot be determined.
    Unknown,
}

/// A discovered CF axis backed by a NetCDF coordinate variable.
#[derive(Debug, Clone, Copy)]
pub struct CfCoordinateAxis<'a> {
    /// The coordinate variable carrying the CF axis metadata.
    pub variable: &'a NcVariable,
    /// The dimension represented by the coordinate variable.
    pub dimension: &'a NcDimension,
    /// The inferred CF axis type.
    pub axis_type: CfAxisType,
}

/// Identify the CF axis type of a variable.
///
/// Checks attributes in CF priority order:
/// 1. `axis` attribute ("X", "Y", "Z", "T")
/// 2. `standard_name` attribute
/// 3. `units` attribute
/// 4. `positive` attribute (vertical indicator)
pub fn identify_axis(var: &NcVariable) -> CfAxisType {
    // 1. Explicit axis attribute (highest priority)
    if let Some(attr) = var.attribute("axis") {
        if let Some(val) = attr.value.as_string() {
            match val.trim().to_uppercase().as_str() {
                "X" => return CfAxisType::X,
                "Y" => return CfAxisType::Y,
                "Z" => return CfAxisType::Z,
                "T" => return CfAxisType::T,
                _ => {}
            }
        }
    }

    // 2. standard_name attribute
    if let Some(attr) = var.attribute("standard_name") {
        if let Some(val) = attr.value.as_string() {
            match val.trim() {
                "latitude" => return CfAxisType::Y,
                "longitude" => return CfAxisType::X,
                "time" => return CfAxisType::T,
                "altitude"
                | "height"
                | "depth"
                | "air_pressure"
                | "atmosphere_hybrid_sigma_pressure_coordinate"
                | "atmosphere_ln_pressure_coordinate"
                | "atmosphere_sigma_coordinate"
                | "ocean_sigma_coordinate"
                | "ocean_s_coordinate"
                | "ocean_double_sigma_coordinate" => return CfAxisType::Z,
                "projection_x_coordinate" | "grid_longitude" => return CfAxisType::X,
                "projection_y_coordinate" | "grid_latitude" => return CfAxisType::Y,
                _ => {}
            }
        }
    }

    // 3. units attribute
    if let Some(attr) = var.attribute("units") {
        if let Some(val) = attr.value.as_string() {
            let lower = val.trim().to_lowercase();
            // Latitude units
            if matches!(
                lower.as_str(),
                "degrees_north"
                    | "degree_north"
                    | "degree_n"
                    | "degrees_n"
                    | "degreen"
                    | "degreesn"
            ) {
                return CfAxisType::Y;
            }
            // Longitude units
            if matches!(
                lower.as_str(),
                "degrees_east" | "degree_east" | "degree_e" | "degrees_e" | "degreee" | "degreese"
            ) {
                return CfAxisType::X;
            }
            // Time units (contains "since")
            if lower.contains(" since ") {
                return CfAxisType::T;
            }
            // Pressure units (common vertical)
            if matches!(
                lower.as_str(),
                "pa" | "hpa" | "mbar" | "millibar" | "bar" | "atm"
            ) {
                return CfAxisType::Z;
            }
        }
    }

    // 4. positive attribute (vertical axis indicator)
    if let Some(attr) = var.attribute("positive") {
        if let Some(val) = attr.value.as_string() {
            let lower = val.trim().to_lowercase();
            if lower == "up" || lower == "down" {
                return CfAxisType::Z;
            }
        }
    }

    CfAxisType::Unknown
}

/// Discover CF axes from coordinate variables in a group.
///
/// Only true coordinate variables are considered: one-dimensional variables
/// whose name matches their dimension name. Variables whose axis role cannot
/// be inferred are omitted.
pub fn discover_coordinate_axes(group: &NcGroup) -> Vec<CfCoordinateAxis<'_>> {
    group
        .coordinate_variables()
        .filter_map(discover_axis_for_coordinate_variable)
        .collect()
}

/// Discover CF axes used by a data variable from its coordinate variables.
///
/// The returned axes follow the variable's dimension order and are resolved
/// through the containing group.
pub fn discover_variable_axes<'a>(
    var: &NcVariable,
    group: &'a NcGroup,
) -> Vec<CfCoordinateAxis<'a>> {
    var.dimensions()
        .iter()
        .filter_map(|dim| group.coordinate_variable(&dim.name))
        .filter_map(discover_axis_for_coordinate_variable)
        .collect()
}

fn discover_axis_for_coordinate_variable(var: &NcVariable) -> Option<CfCoordinateAxis<'_>> {
    let dimension = var.coordinate_dimension()?;
    let axis_type = identify_axis(var);
    if axis_type == CfAxisType::Unknown {
        return None;
    }

    Some(CfCoordinateAxis {
        variable: var,
        dimension,
        axis_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NcAttrValue, NcAttribute, NcDimension, NcType, NcVariable};

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

    fn make_coordinate_var(
        name: &str,
        size: u64,
        dtype: NcType,
        attrs: Vec<NcAttribute>,
    ) -> NcVariable {
        NcVariable {
            name: name.into(),
            dimensions: vec![NcDimension {
                name: name.into(),
                size,
                is_unlimited: false,
            }],
            dtype,
            attributes: attrs,
            data_offset: 0,
            _data_size: 0,
            is_record_var: false,
            record_size: 0,
        }
    }

    fn attr(name: &str, value: &str) -> NcAttribute {
        NcAttribute {
            name: name.into(),
            value: NcAttrValue::Chars(value.into()),
        }
    }

    #[test]
    fn axis_attribute() {
        let var = make_var(vec![NcAttribute {
            name: "axis".into(),
            value: NcAttrValue::Chars("X".into()),
        }]);
        assert_eq!(identify_axis(&var), CfAxisType::X);
    }

    #[test]
    fn standard_name_latitude() {
        let var = make_var(vec![NcAttribute {
            name: "standard_name".into(),
            value: NcAttrValue::Chars("latitude".into()),
        }]);
        assert_eq!(identify_axis(&var), CfAxisType::Y);
    }

    #[test]
    fn standard_name_time() {
        let var = make_var(vec![NcAttribute {
            name: "standard_name".into(),
            value: NcAttrValue::Chars("time".into()),
        }]);
        assert_eq!(identify_axis(&var), CfAxisType::T);
    }

    #[test]
    fn units_degrees_north() {
        let var = make_var(vec![NcAttribute {
            name: "units".into(),
            value: NcAttrValue::Chars("degrees_north".into()),
        }]);
        assert_eq!(identify_axis(&var), CfAxisType::Y);
    }

    #[test]
    fn units_time_since() {
        let var = make_var(vec![NcAttribute {
            name: "units".into(),
            value: NcAttrValue::Chars("days since 1970-01-01".into()),
        }]);
        assert_eq!(identify_axis(&var), CfAxisType::T);
    }

    #[test]
    fn positive_up() {
        let var = make_var(vec![NcAttribute {
            name: "positive".into(),
            value: NcAttrValue::Chars("up".into()),
        }]);
        assert_eq!(identify_axis(&var), CfAxisType::Z);
    }

    #[test]
    fn unknown() {
        let var = make_var(vec![]);
        assert_eq!(identify_axis(&var), CfAxisType::Unknown);
    }

    #[test]
    fn axis_takes_precedence() {
        // axis="X" should win over standard_name="latitude"
        let var = make_var(vec![
            NcAttribute {
                name: "axis".into(),
                value: NcAttrValue::Chars("X".into()),
            },
            NcAttribute {
                name: "standard_name".into(),
                value: NcAttrValue::Chars("latitude".into()),
            },
        ]);
        assert_eq!(identify_axis(&var), CfAxisType::X);
    }

    #[test]
    fn discover_coordinate_axes_from_group() {
        let time = make_coordinate_var(
            "time",
            4,
            NcType::Double,
            vec![attr("units", "hours since 2000-01-01")],
        );
        let lat = make_coordinate_var(
            "lat",
            6,
            NcType::Double,
            vec![attr("units", "degrees_north")],
        );
        let lon = make_coordinate_var(
            "lon",
            12,
            NcType::Double,
            vec![attr("units", "degrees_east")],
        );
        let station = NcVariable {
            name: "station".into(),
            dimensions: vec![NcDimension {
                name: "obs".into(),
                size: 4,
                is_unlimited: false,
            }],
            dtype: NcType::Int,
            attributes: vec![attr("axis", "X")],
            data_offset: 0,
            _data_size: 0,
            is_record_var: false,
            record_size: 0,
        };
        let group = crate::types::NcGroup {
            name: "/".into(),
            dimensions: vec![
                time.dimensions()[0].clone(),
                lat.dimensions()[0].clone(),
                lon.dimensions()[0].clone(),
            ],
            variables: vec![time, lat, lon, station],
            attributes: vec![],
            groups: vec![],
        };

        let axes = discover_coordinate_axes(&group);
        let discovered: Vec<(&str, CfAxisType)> = axes
            .iter()
            .map(|axis| (axis.variable.name(), axis.axis_type))
            .collect();
        assert_eq!(
            discovered,
            vec![
                ("time", CfAxisType::T),
                ("lat", CfAxisType::Y),
                ("lon", CfAxisType::X)
            ]
        );
    }

    #[test]
    fn discover_variable_axes_follows_dimension_order() {
        let time = make_coordinate_var(
            "time",
            4,
            NcType::Double,
            vec![attr("units", "hours since 2000-01-01")],
        );
        let lat = make_coordinate_var(
            "lat",
            6,
            NcType::Double,
            vec![attr("units", "degrees_north")],
        );
        let lon = make_coordinate_var(
            "lon",
            12,
            NcType::Double,
            vec![attr("units", "degrees_east")],
        );
        let temperature = NcVariable {
            name: "temperature".into(),
            dimensions: vec![
                time.dimensions()[0].clone(),
                lat.dimensions()[0].clone(),
                lon.dimensions()[0].clone(),
            ],
            dtype: NcType::Float,
            attributes: vec![],
            data_offset: 0,
            _data_size: 0,
            is_record_var: false,
            record_size: 0,
        };
        let group = crate::types::NcGroup {
            name: "/".into(),
            dimensions: vec![
                time.dimensions()[0].clone(),
                lat.dimensions()[0].clone(),
                lon.dimensions()[0].clone(),
            ],
            variables: vec![lon, time, lat, temperature.clone()],
            attributes: vec![],
            groups: vec![],
        };

        let axes = discover_variable_axes(&temperature, &group);
        let discovered: Vec<(&str, CfAxisType)> = axes
            .iter()
            .map(|axis| (axis.dimension.name.as_str(), axis.axis_type))
            .collect();
        assert_eq!(
            discovered,
            vec![
                ("time", CfAxisType::T),
                ("lat", CfAxisType::Y),
                ("lon", CfAxisType::X)
            ]
        );
    }
}
