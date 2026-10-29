//! CF coordinate reference system (CRS) extraction.
//!
//! Reads `grid_mapping` attributes to determine the projection/CRS of
//! spatial data. Supports common projections:
//! - latitude_longitude (EPSG:4326)
//! - transverse_mercator
//! - lambert_conformal_conic
//! - polar_stereographic
//! - rotated_latitude_longitude
//!
//! Reference: CF Conventions §5.6 "Grid Mappings and Projections"

use crate::types::{NcGroup, NcVariable};

/// Extracted CRS information from CF grid_mapping attributes.
#[derive(Debug, Clone)]
pub struct CfCrs {
    /// The grid_mapping_name (e.g., "latitude_longitude", "transverse_mercator").
    pub grid_mapping_name: String,
    /// Semi-major axis of the ellipsoid in meters.
    pub semi_major_axis: Option<f64>,
    /// Inverse flattening of the ellipsoid.
    pub inverse_flattening: Option<f64>,
    /// Latitude of projection origin.
    pub latitude_of_projection_origin: Option<f64>,
    /// Longitude of central meridian.
    pub longitude_of_central_meridian: Option<f64>,
    /// Scale factor at the central meridian or natural origin.
    pub scale_factor_at_projection_origin: Option<f64>,
    /// False easting in projection units.
    pub false_easting: Option<f64>,
    /// False northing in projection units.
    pub false_northing: Option<f64>,
    /// Standard parallel(s) for conic projections.
    pub standard_parallel: Vec<f64>,
    /// Longitude of projection center (for some projections).
    pub longitude_of_projection_origin: Option<f64>,
    /// Straight vertical longitude from pole (polar stereographic).
    pub straight_vertical_longitude_from_pole: Option<f64>,
    /// North pole grid longitude (rotated pole).
    pub grid_north_pole_longitude: Option<f64>,
    /// North pole grid latitude (rotated pole).
    pub grid_north_pole_latitude: Option<f64>,
    /// EPSG code if identifiable.
    pub epsg: Option<u32>,
}

impl CfCrs {
    /// Returns true if this is a standard geographic CRS (lat/lon on WGS84).
    pub fn is_geographic(&self) -> bool {
        self.grid_mapping_name == "latitude_longitude"
    }
}

/// Extract CRS from a variable's grid_mapping attribute.
///
/// Looks up the `grid_mapping` attribute on the variable, finds the
/// referenced grid mapping variable in the group, and extracts its
/// CRS attributes.
pub fn extract_crs(var: &NcVariable, group: &NcGroup) -> Option<CfCrs> {
    let mapping_name = var.attribute("grid_mapping")?.value.as_string()?;
    let mapping_var = group.variables.iter().find(|v| v.name == mapping_name)?;
    Some(parse_grid_mapping(mapping_var))
}

/// Parse CRS attributes from a grid mapping variable.
pub fn parse_grid_mapping(mapping_var: &NcVariable) -> CfCrs {
    let grid_mapping_name = mapping_var
        .attribute("grid_mapping_name")
        .and_then(|a| a.value.as_string())
        .unwrap_or_default();

    let semi_major_axis = mapping_var
        .attribute("semi_major_axis")
        .and_then(|a| a.value.as_f64());
    let inverse_flattening = mapping_var
        .attribute("inverse_flattening")
        .and_then(|a| a.value.as_f64());
    let latitude_of_projection_origin = mapping_var
        .attribute("latitude_of_projection_origin")
        .and_then(|a| a.value.as_f64());
    let longitude_of_central_meridian = mapping_var
        .attribute("longitude_of_central_meridian")
        .and_then(|a| a.value.as_f64());
    let scale_factor_at_projection_origin = mapping_var
        .attribute("scale_factor_at_projection_origin")
        .or_else(|| mapping_var.attribute("scale_factor_at_central_meridian"))
        .and_then(|a| a.value.as_f64());
    let false_easting = mapping_var
        .attribute("false_easting")
        .and_then(|a| a.value.as_f64());
    let false_northing = mapping_var
        .attribute("false_northing")
        .and_then(|a| a.value.as_f64());
    let standard_parallel = mapping_var
        .attribute("standard_parallel")
        .and_then(|a| a.value.as_f64_vec())
        .unwrap_or_default();
    let longitude_of_projection_origin = mapping_var
        .attribute("longitude_of_projection_origin")
        .and_then(|a| a.value.as_f64());
    let straight_vertical_longitude_from_pole = mapping_var
        .attribute("straight_vertical_longitude_from_pole")
        .and_then(|a| a.value.as_f64());
    let grid_north_pole_longitude = mapping_var
        .attribute("grid_north_pole_longitude")
        .and_then(|a| a.value.as_f64());
    let grid_north_pole_latitude = mapping_var
        .attribute("grid_north_pole_latitude")
        .and_then(|a| a.value.as_f64());

    // Try to identify EPSG code for common cases
    let epsg = identify_epsg(&grid_mapping_name, semi_major_axis, inverse_flattening);

    CfCrs {
        grid_mapping_name,
        semi_major_axis,
        inverse_flattening,
        latitude_of_projection_origin,
        longitude_of_central_meridian,
        scale_factor_at_projection_origin,
        false_easting,
        false_northing,
        standard_parallel,
        longitude_of_projection_origin,
        straight_vertical_longitude_from_pole,
        grid_north_pole_longitude,
        grid_north_pole_latitude,
        epsg,
    }
}

/// Try to identify the EPSG code from common parameter combinations.
fn identify_epsg(
    grid_mapping_name: &str,
    semi_major_axis: Option<f64>,
    inverse_flattening: Option<f64>,
) -> Option<u32> {
    if grid_mapping_name == "latitude_longitude" {
        // Check for WGS84 ellipsoid
        if let (Some(a), Some(f)) = (semi_major_axis, inverse_flattening) {
            if (a - 6378137.0).abs() < 1.0 && (f - 298.257223563).abs() < 0.001 {
                return Some(4326);
            }
        }
        // Default lat/lon without explicit ellipsoid
        if semi_major_axis.is_none() {
            return Some(4326);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NcAttrValue, NcAttribute, NcType};

    fn mapping_var(attrs: Vec<NcAttribute>) -> NcVariable {
        NcVariable {
            name: "crs".into(),
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
    fn geographic_wgs84() {
        let var = mapping_var(vec![
            NcAttribute {
                name: "grid_mapping_name".into(),
                value: NcAttrValue::Chars("latitude_longitude".into()),
            },
            NcAttribute {
                name: "semi_major_axis".into(),
                value: NcAttrValue::Doubles(vec![6378137.0]),
            },
            NcAttribute {
                name: "inverse_flattening".into(),
                value: NcAttrValue::Doubles(vec![298.257223563]),
            },
        ]);
        let crs = parse_grid_mapping(&var);
        assert!(crs.is_geographic());
        assert_eq!(crs.epsg, Some(4326));
    }

    #[test]
    fn transverse_mercator() {
        let var = mapping_var(vec![
            NcAttribute {
                name: "grid_mapping_name".into(),
                value: NcAttrValue::Chars("transverse_mercator".into()),
            },
            NcAttribute {
                name: "latitude_of_projection_origin".into(),
                value: NcAttrValue::Doubles(vec![0.0]),
            },
            NcAttribute {
                name: "longitude_of_central_meridian".into(),
                value: NcAttrValue::Doubles(vec![9.0]),
            },
            NcAttribute {
                name: "scale_factor_at_central_meridian".into(),
                value: NcAttrValue::Doubles(vec![0.9996]),
            },
            NcAttribute {
                name: "false_easting".into(),
                value: NcAttrValue::Doubles(vec![500000.0]),
            },
            NcAttribute {
                name: "false_northing".into(),
                value: NcAttrValue::Doubles(vec![0.0]),
            },
        ]);
        let crs = parse_grid_mapping(&var);
        assert_eq!(crs.grid_mapping_name, "transverse_mercator");
        assert_eq!(crs.latitude_of_projection_origin, Some(0.0));
        assert_eq!(crs.longitude_of_central_meridian, Some(9.0));
        assert_eq!(crs.scale_factor_at_projection_origin, Some(0.9996));
        assert!(!crs.is_geographic());
    }

    #[test]
    fn extract_crs_finds_grid_mapping_variable() {
        let mapping = NcVariable {
            name: "crs".into(),
            dimensions: vec![],
            dtype: NcType::Int,
            attributes: vec![NcAttribute {
                name: "grid_mapping_name".into(),
                value: NcAttrValue::Chars("latitude_longitude".into()),
            }],
            data_offset: 0,
            _data_size: 0,
            is_record_var: false,
            record_size: 0,
        };
        let data_var = NcVariable {
            name: "temperature".into(),
            dimensions: vec![],
            dtype: NcType::Float,
            attributes: vec![NcAttribute {
                name: "grid_mapping".into(),
                value: NcAttrValue::Chars("crs".into()),
            }],
            data_offset: 0,
            _data_size: 0,
            is_record_var: false,
            record_size: 0,
        };
        let group = NcGroup {
            name: "/".into(),
            dimensions: vec![],
            variables: vec![data_var.clone(), mapping],
            attributes: vec![],
            groups: vec![],
        };

        let crs = extract_crs(&data_var, &group).unwrap();
        assert!(crs.is_geographic());
        assert_eq!(crs.epsg, Some(4326));
    }
}
