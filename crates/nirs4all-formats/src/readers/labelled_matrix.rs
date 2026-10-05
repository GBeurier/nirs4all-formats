//! Shared mapping for Unscrambler matrices and their labelled MATLAB exports.
use std::collections::BTreeMap;

use nirs4all_formats_core::{
    Error, Result, SignalType, SourceFile, SpectralArray, SpectralAxis, SpectralRecord,
};
use serde_json::{json, Value};

use super::util::provenance;

pub(super) struct MatrixGroup {
    pub name: String,
    pub columns: Vec<usize>,
}

pub(super) struct LabelledMatrix {
    pub name: String,
    pub rows: usize,
    pub labels: Vec<String>,
    pub objects: Vec<String>,
    pub values: Vec<f64>, // column-major
    pub groups: Vec<MatrixGroup>,
    pub metadata: BTreeMap<String, Value>,
}

pub(super) fn records(
    matrix: LabelledMatrix,
    format: &str,
    reader: &str,
    source: SourceFile,
) -> Result<Vec<SpectralRecord>> {
    let cols = matrix.labels.len();
    if matrix.rows == 0
        || cols == 0
        || matrix.objects.len() != matrix.rows
        || matrix.rows.checked_mul(cols) != Some(matrix.values.len())
    {
        return Err(Error::InvalidRecord(
            "labelled matrix dimensions do not match labels/data".into(),
        ));
    }
    let placeholder = |col: usize| matrix.labels[col].is_empty() || matrix.labels[col] == "*";
    let mut groups: Vec<&MatrixGroup> = matrix
        .groups
        .iter()
        .filter(|group| {
            group.columns.len() > 1
                && group
                    .columns
                    .iter()
                    .all(|&col| col < cols && placeholder(col))
        })
        .collect();
    // Keep the leaf blocks; aggregate selections such as "spectroscopic" would
    // otherwise duplicate their child channels. Equal selections are aliases.
    groups.retain(|group| {
        !matrix.groups.iter().any(|other| {
            other.columns.len() > 1
                && other.columns.len() < group.columns.len()
                && other.columns.iter().all(|col| group.columns.contains(col))
                && other
                    .columns
                    .iter()
                    .all(|&col| col < cols && placeholder(col))
        })
    });
    let mut channels: Vec<(String, Vec<usize>)> = Vec::new();
    let mut used = vec![false; cols];
    for group in groups {
        if channels
            .iter()
            .any(|(_, columns)| columns == &group.columns)
        {
            continue;
        }
        if group.columns.iter().any(|&col| used[col]) {
            return Err(Error::InvalidRecord(
                "Unscrambler leaf signal groups overlap".into(),
            ));
        }
        for &col in &group.columns {
            used[col] = true;
        }
        channels.push((group.name.clone(), group.columns.clone()));
    }
    // A MAT export omits group definitions: retain the unlabelled block as one
    // unknown signal, without reconstructing NIR/NMR boundaries from a filename.
    let mut col = 0;
    while col < cols {
        if !used[col] && placeholder(col) {
            let start = col;
            while col < cols && !used[col] && placeholder(col) {
                used[col] = true;
                col += 1;
            }
            channels.push((format!("signal_{}", start + 1), (start..col).collect()));
        } else {
            col += 1;
        }
    }
    if channels.is_empty() {
        used.fill(true);
        channels.push(("signal".into(), (0..cols).collect()));
    }
    let mut signal_names = BTreeMap::new();
    for (name, columns) in channels {
        let mut key = if name.is_empty() {
            "signal".into()
        } else {
            name
        };
        if signal_names.contains_key(&key) {
            key = format!("{key}_{}", columns[0] + 1);
        }
        if signal_names.insert(key, columns).is_some() {
            return Err(Error::InvalidRecord(
                "duplicate Unscrambler signal names".into(),
            ));
        }
    }
    let warnings = vec![
        "Signal calibration and physical type are unavailable; generated index axes and unknown signal types are used".into(),
        "Labelled columns outside signal blocks are exposed as targets; their modelling roles are not specified".into(),
    ];
    let target_columns: Vec<_> = (0..cols).filter(|&col| !used[col]).collect();
    let mut records = Vec::with_capacity(matrix.rows);
    for row in 0..matrix.rows {
        let mut signals = BTreeMap::new();
        let mut missing = 0;
        for (name, columns) in &signal_names {
            let values: Vec<_> = columns
                .iter()
                .map(|&col| matrix.values[row + col * matrix.rows])
                .collect();
            missing += values.iter().filter(|v| !v.is_finite()).count();
            signals.insert(
                name.clone(),
                SpectralArray::new(
                    SpectralAxis::index(columns.len()),
                    values,
                    vec!["x".into()],
                    SignalType::Unknown,
                    None,
                    "signal",
                    "file",
                )?,
            );
        }
        let mut targets = BTreeMap::new();
        for &col in &target_columns {
            let label = &matrix.labels[col];
            let mut key = label.clone();
            if targets.contains_key(&key) {
                key = format!("{label}_{}", col + 1);
            }
            let value = matrix.values[row + col * matrix.rows];
            if !value.is_finite() {
                missing += 1;
            }
            if targets
                .insert(
                    key,
                    if value.is_finite() {
                        json!(value)
                    } else {
                        Value::Null
                    },
                )
                .is_some()
            {
                return Err(Error::InvalidRecord(
                    "duplicate Unscrambler target names".into(),
                ));
            }
        }
        let mut metadata = matrix.metadata.clone();
        metadata.insert("matrix".into(), json!(matrix.name));
        metadata.insert("matrix_orientation".into(), json!("samples_by_variables"));
        metadata.insert("sample_index".into(), json!(row));
        metadata.insert("sample_id".into(), json!(matrix.objects[row]));
        metadata.insert("variable_labels".into(), json!(matrix.labels));
        metadata.insert("signal_columns".into(), json!(signal_names));
        metadata.insert("missing_value_count".into(), json!(missing));
        let record = SpectralRecord {
            signals,
            signal_type: SignalType::Unknown,
            targets,
            metadata,
            provenance: provenance(format, reader, source.clone(), warnings.clone()),
            quality_flags: if missing > 0 {
                vec!["missing_values".into()]
            } else {
                vec![]
            },
        };
        record.validate()?;
        records.push(record);
    }
    Ok(records)
}
