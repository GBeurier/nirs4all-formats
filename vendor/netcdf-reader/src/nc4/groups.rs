//! Map HDF5 groups to NetCDF-4 groups.
//!
//! Each HDF5 group becomes an `NcGroup`. The root group is special: it may
//! contain `_NCProperties` and other internal attributes that should be filtered.
//! Sub-groups are traversed recursively.

use std::collections::HashMap;

use hdf5_reader::Hdf5File;

use crate::error::{Error, Result};
use crate::types::{NcDimension, NcGroup};

use super::attributes;
use super::dimensions;
use super::variables;

#[allow(dead_code)]
pub(crate) struct GroupContext {
    pub(crate) group: hdf5_reader::group::Group,
    pub(crate) visible_dimensions: Vec<NcDimension>,
    pub(crate) visible_dim_addr_map: HashMap<u64, NcDimension>,
}

fn leaf_name(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

fn visible_dimensions(
    local_dimensions: &[NcDimension],
    inherited_dimensions: &[NcDimension],
) -> Vec<NcDimension> {
    let mut visible_dimensions = local_dimensions.to_vec();
    visible_dimensions.extend(
        inherited_dimensions
            .iter()
            .filter(|dim| {
                !local_dimensions
                    .iter()
                    .any(|local_dim| local_dim.name == dim.name)
            })
            .cloned(),
    );
    visible_dimensions
}

fn visible_dim_addr_map(
    local_dim_addr_map: HashMap<u64, NcDimension>,
    inherited_dim_addr_map: &HashMap<u64, NcDimension>,
) -> HashMap<u64, NcDimension> {
    let mut visible_dim_addr_map = inherited_dim_addr_map.clone();
    visible_dim_addr_map.extend(local_dim_addr_map);
    visible_dim_addr_map
}

/// Build the root NcGroup from an HDF5 file.
pub fn build_root_group(hdf5: &Hdf5File, metadata_mode: crate::NcMetadataMode) -> Result<NcGroup> {
    build_group_at_path(hdf5, "/", metadata_mode, true)
}

/// Build only the root group's local metadata.
pub fn build_root_group_metadata(
    hdf5: &Hdf5File,
    metadata_mode: crate::NcMetadataMode,
) -> Result<NcGroup> {
    build_group_at_path(hdf5, "/", metadata_mode, false)
}

/// Build metadata for a group path.
pub fn build_group_at_path(
    hdf5: &Hdf5File,
    path: &str,
    metadata_mode: crate::NcMetadataMode,
    recursive: bool,
) -> Result<NcGroup> {
    let normalized = normalize_group_path(path);
    let root = hdf5.root_group()?;
    let mut group = root;
    let mut inherited_dimensions = Vec::new();
    let mut inherited_dim_addr_map = HashMap::new();

    for component in normalized.split('/').filter(|part| !part.is_empty()) {
        let datasets = group.datasets()?;
        let (local_dimensions, local_dim_addr_map) =
            dimensions::extract_dimensions_from_datasets(&datasets, metadata_mode)?;
        inherited_dimensions = visible_dimensions(&local_dimensions, &inherited_dimensions);
        inherited_dim_addr_map = visible_dim_addr_map(local_dim_addr_map, &inherited_dim_addr_map);
        group = group
            .group(component)
            .map_err(|_| Error::GroupNotFound(path.to_string()))?;
    }

    let group_name = if normalized.is_empty() {
        "/".to_string()
    } else {
        leaf_name(group.name()).to_string()
    };

    if recursive {
        build_group_recursive(
            &group,
            &group_name,
            &inherited_dimensions,
            &inherited_dim_addr_map,
            metadata_mode,
        )
    } else {
        build_group_metadata(
            &group,
            &group_name,
            &inherited_dimensions,
            &inherited_dim_addr_map,
            metadata_mode,
        )
    }
}

#[allow(dead_code)]
pub(crate) fn group_context_at_path(
    hdf5: &Hdf5File,
    path: &str,
    metadata_mode: crate::NcMetadataMode,
) -> Result<GroupContext> {
    let normalized = normalize_group_path(path);
    let root = hdf5.root_group()?;
    let mut group = root;
    let mut inherited_dimensions = Vec::new();
    let mut inherited_dim_addr_map = HashMap::new();

    for component in normalized.split('/').filter(|part| !part.is_empty()) {
        let datasets = group.datasets()?;
        let (local_dimensions, local_dim_addr_map) =
            dimensions::extract_dimensions_from_datasets(&datasets, metadata_mode)?;
        inherited_dimensions = visible_dimensions(&local_dimensions, &inherited_dimensions);
        inherited_dim_addr_map = visible_dim_addr_map(local_dim_addr_map, &inherited_dim_addr_map);
        group = group
            .group(component)
            .map_err(|_| Error::GroupNotFound(path.to_string()))?;
    }

    let datasets = group.datasets()?;
    let (local_dimensions, local_dim_addr_map) =
        dimensions::extract_dimensions_from_datasets(&datasets, metadata_mode)?;
    let visible_dimensions = visible_dimensions(&local_dimensions, &inherited_dimensions);
    let visible_dim_addr_map = visible_dim_addr_map(local_dim_addr_map, &inherited_dim_addr_map);

    Ok(GroupContext {
        group,
        visible_dimensions,
        visible_dim_addr_map,
    })
}

/// Recursively build an NcGroup from an HDF5 Group.
fn build_group_recursive(
    hdf5_group: &hdf5_reader::group::Group,
    name: &str,
    inherited_dimensions: &[NcDimension],
    inherited_dim_addr_map: &HashMap<u64, NcDimension>,
    metadata_mode: crate::NcMetadataMode,
) -> Result<NcGroup> {
    let (hdf5_children, datasets) = hdf5_group.members()?;

    // Extract dimensions declared locally in this group, then combine them
    // with dimensions inherited from ancestor groups for lookups and variable
    // reconstruction.
    let (local_dimensions, local_dim_addr_map) =
        dimensions::extract_dimensions_from_datasets(&datasets, metadata_mode)?;
    let visible_dimensions = visible_dimensions(&local_dimensions, inherited_dimensions);
    let visible_dim_addr_map = visible_dim_addr_map(local_dim_addr_map, inherited_dim_addr_map);

    // Extract variables (non-dimension-scale datasets).
    let variables = variables::extract_variables_from_datasets(
        &datasets,
        hdf5_group,
        &visible_dimensions,
        &visible_dim_addr_map,
        metadata_mode,
    )?;

    // Extract group-level attributes, filtering internal NetCDF-4 attributes.
    let nc_attributes = attributes::extract_group_attributes(hdf5_group, metadata_mode)?;

    // Recurse into child groups.
    let mut child_groups = Vec::new();
    for child in &hdf5_children {
        let child_name = leaf_name(child.name()).to_string();
        let nc_child = build_group_recursive(
            child,
            &child_name,
            &visible_dimensions,
            &visible_dim_addr_map,
            metadata_mode,
        )?;
        child_groups.push(nc_child);
    }

    Ok(NcGroup {
        name: name.to_string(),
        dimensions: visible_dimensions,
        variables,
        attributes: nc_attributes,
        groups: child_groups,
    })
}

fn build_group_metadata(
    hdf5_group: &hdf5_reader::group::Group,
    name: &str,
    inherited_dimensions: &[NcDimension],
    inherited_dim_addr_map: &HashMap<u64, NcDimension>,
    metadata_mode: crate::NcMetadataMode,
) -> Result<NcGroup> {
    let datasets = hdf5_group.datasets()?;
    let (local_dimensions, local_dim_addr_map) =
        dimensions::extract_dimensions_from_datasets(&datasets, metadata_mode)?;
    let visible_dimensions = visible_dimensions(&local_dimensions, inherited_dimensions);
    let visible_dim_addr_map = visible_dim_addr_map(local_dim_addr_map, inherited_dim_addr_map);
    let variables = variables::extract_variables_from_datasets(
        &datasets,
        hdf5_group,
        &visible_dimensions,
        &visible_dim_addr_map,
        metadata_mode,
    )?;
    let attributes = attributes::extract_group_attributes(hdf5_group, metadata_mode)?;

    Ok(NcGroup {
        name: name.to_string(),
        dimensions: visible_dimensions,
        variables,
        attributes,
        groups: Vec::new(),
    })
}

fn normalize_group_path(path: &str) -> &str {
    path.trim_matches('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_dimensions_include_inherited_without_duplicates() {
        let local = vec![NcDimension {
            name: "y".to_string(),
            size: 4,
            is_unlimited: false,
        }];
        let inherited = vec![
            NcDimension {
                name: "x".to_string(),
                size: 3,
                is_unlimited: false,
            },
            NcDimension {
                name: "y".to_string(),
                size: 99,
                is_unlimited: true,
            },
        ];

        let merged = visible_dimensions(&local, &inherited);
        let names: Vec<&str> = merged.iter().map(|dim| dim.name.as_str()).collect();
        assert_eq!(names, vec!["y", "x"]);
        assert_eq!(merged[0].size, 4);
        assert!(!merged[0].is_unlimited);
    }

    #[test]
    fn visible_dim_addr_map_prefers_local_dimensions() {
        let mut inherited = HashMap::new();
        inherited.insert(
            10,
            NcDimension {
                name: "x".to_string(),
                size: 3,
                is_unlimited: false,
            },
        );
        inherited.insert(
            20,
            NcDimension {
                name: "shared".to_string(),
                size: 1,
                is_unlimited: false,
            },
        );

        let mut local = HashMap::new();
        local.insert(
            20,
            NcDimension {
                name: "shared".to_string(),
                size: 2,
                is_unlimited: true,
            },
        );

        let merged = visible_dim_addr_map(local, &inherited);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged.get(&10).unwrap().name, "x");
        assert_eq!(merged.get(&20).unwrap().size, 2);
        assert!(merged.get(&20).unwrap().is_unlimited);
    }
}
