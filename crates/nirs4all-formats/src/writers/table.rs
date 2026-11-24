//! Matrix targets: one row per spectrum sharing an axis.

use std::collections::BTreeSet;

use nirs4all_formats_core::SpectralRecord;
use serde_json::Value;

use super::select::{groups, report_record_losses, spectra, target_text, Spectrum, TargetSupport};
use super::xlsx::{self, Cell};
use super::{number, refused, Context, WriteError, WrittenFile};

const XLSX_MAX_COLUMNS: usize = 16_384;
const XLSX_MAX_ROWS: usize = 1_048_576;

pub(crate) fn write_csv(
    records: &[SpectralRecord],
    context: &mut Context<'_>,
) -> Result<Vec<WrittenFile>, WriteError> {
    let delimiter = context.options.delimiter.unwrap_or(',');
    let spectra = spectra(records, context)?;
    report_record_losses(records, context, TargetSupport::Columns);
    let groups = groups(&spectra);
    let several = groups.len() > 1;
    let mut files = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        let table = Table::new(group, context);
        context.losses.add(
            "axis_and_signal_not_described",
            format!(
                "the table does not store that the axis is in '{}' and the signal is {}",
                group[0].axis.unit,
                group[0].type_label()
            ),
        );
        let mut out = String::new();
        let header: Vec<String> = table
            .header
            .iter()
            .map(|text| plain(text, delimiter, context))
            .collect();
        push_row(&mut out, &header, delimiter);
        for row in &table.rows {
            let cells: Vec<String> = row
                .iter()
                .map(|cell| match cell {
                    Cell::Empty => String::new(),
                    Cell::Number(value) => number(*value),
                    Cell::Text(text) => plain(text, delimiter, context),
                })
                .collect();
            push_row(&mut out, &cells, delimiter);
        }
        files.push(context.file(
            &[&group[0].signal_label, &grid_label(several, index)],
            out.into_bytes(),
        ));
    }
    Ok(files)
}

pub(crate) fn write_xlsx(
    records: &[SpectralRecord],
    context: &mut Context<'_>,
) -> Result<Vec<WrittenFile>, WriteError> {
    let spectra = spectra(records, context)?;
    report_record_losses(records, context, TargetSupport::Columns);
    let groups = groups(&spectra);
    let several = groups.len() > 1;
    let mut files = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        let table = Table::new(group, context);
        if table.header.len() > XLSX_MAX_COLUMNS || table.rows.len() + 1 > XLSX_MAX_ROWS {
            return Err(refused(format!(
                "{} rows by {} columns exceed the Excel limits of {XLSX_MAX_ROWS} by {XLSX_MAX_COLUMNS}",
                table.rows.len() + 1,
                table.header.len()
            )));
        }
        let mut sheet: Vec<Vec<Cell>> = Vec::with_capacity(table.rows.len() + 1);
        sheet.push(
            table
                .header
                .iter()
                .enumerate()
                .map(|(column, text)| match text.parse::<f64>() {
                    Ok(value) if column >= table.spectral_start => Cell::Number(value),
                    _ => Cell::Text(text.clone()),
                })
                .collect(),
        );
        sheet.extend(table.rows);
        let bytes = xlsx::workbook(&[("spectra", sheet), ("info", info_sheet(group))])?;
        files.push(context.file(
            &[&group[0].signal_label, &grid_label(several, index)],
            bytes,
        ));
    }
    Ok(files)
}

fn grid_label(several: bool, index: usize) -> String {
    if several {
        format!("grid{}", index + 1)
    } else {
        String::new()
    }
}

struct Table {
    header: Vec<String>,
    rows: Vec<Vec<Cell>>,
    spectral_start: usize,
}

impl Table {
    fn new(group: &[&Spectrum<'_>], context: &mut Context<'_>) -> Self {
        let keys: Vec<&String> = group
            .iter()
            .flat_map(|spectrum| spectrum.record.targets.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut header = vec!["sample_id".to_string()];
        header.extend(keys.iter().map(|key| column_name(key)));
        let spectral_start = header.len();
        header.extend(group[0].axis.values.iter().map(|value| number(*value)));

        let rows = group
            .iter()
            .map(|spectrum| {
                let mut row = vec![Cell::Text(spectrum.id.clone())];
                for key in &keys {
                    row.push(target_cell(spectrum.record.targets.get(*key), context));
                }
                let mut missing = false;
                row.extend(spectrum.values.iter().map(|value| {
                    if value.is_finite() {
                        Cell::Number(*value)
                    } else {
                        missing = true;
                        Cell::Empty
                    }
                }));
                if missing {
                    context.losses.add(
                        "nonfinite_as_empty",
                        "missing and non-finite values are written as empty cells",
                    );
                }
                row
            })
            .collect();
        Self {
            header,
            rows,
            spectral_start,
        }
    }
}

/// Names that read as numbers would be mistaken for axis values.
fn column_name(key: &str) -> String {
    let numeric = key.trim().replace(',', ".").parse::<f64>().is_ok();
    if numeric || key.eq_ignore_ascii_case("sample_id") {
        format!("target_{key}")
    } else {
        key.to_string()
    }
}

fn target_cell(value: Option<&Value>, context: &mut Context<'_>) -> Cell {
    match value {
        None | Some(Value::Null) => Cell::Empty,
        Some(Value::Number(value)) => match value.as_f64() {
            Some(float) => Cell::Number(float),
            None => Cell::Text(value.to_string()),
        },
        Some(other) => match target_text(other) {
            Some(text) => Cell::Text(text),
            None => {
                context.losses.add(
                    "structured_targets_omitted",
                    "target values that are lists or objects are not written",
                );
                Cell::Empty
            }
        },
    }
}

fn push_row(out: &mut String, cells: &[String], delimiter: char) {
    out.push_str(&cells.join(&delimiter.to_string()));
    out.push_str("\r\n");
}

/// Many importers, including ours, do not honour quoted separators.
fn plain(text: &str, delimiter: char, context: &mut Context<'_>) -> String {
    if !text.contains([delimiter, '"', '\n', '\r']) {
        return text.to_string();
    }
    context.losses.add(
        "text_sanitized",
        "separators, quotes and line breaks inside text cells are replaced by '_'",
    );
    text.replace([delimiter, '"', '\n', '\r'], "_")
}

fn info_sheet(group: &[&Spectrum<'_>]) -> Vec<Vec<Cell>> {
    let head = group[0];
    let provenance = &head.record.provenance;
    let mut rows = vec![
        ("axis_unit", head.axis.unit.clone()),
        (
            "axis_kind",
            format!("{:?}", head.axis.kind).to_ascii_lowercase(),
        ),
        ("signal", head.signal_name.to_string()),
        ("signal_type", head.type_label().to_string()),
        ("signal_unit", head.signal.unit.clone().unwrap_or_default()),
        ("source_format", provenance.format.clone()),
        (
            "writer",
            format!("nirs4all-formats {}", env!("CARGO_PKG_VERSION")),
        ),
    ];
    let sources: BTreeSet<&str> = group
        .iter()
        .flat_map(|spectrum| &spectrum.record.provenance.sources)
        .map(|source| source.path.as_str())
        .collect();
    rows.extend(
        sources
            .into_iter()
            .map(|path| ("source_file", path.to_string())),
    );
    rows.into_iter()
        .map(|(key, value)| vec![Cell::Text(key.to_string()), Cell::Text(value)])
        .collect()
}
