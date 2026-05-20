use std::collections::BTreeMap;
use std::path::Path;

use calamine::{open_workbook_auto, Data, DataType, Reader};
use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralRecord,
};
use serde_json::json;

use crate::readers::util::{normalize_key, parse_number, single_signal_record, SingleSignalSpec};
use crate::Reader as NirsReader;

pub struct ExcelReader;

impl NirsReader for ExcelReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::excel"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "xlsx" | "xlsm") || !head.starts_with(b"PK\x03\x04") {
            return None;
        }
        Some(FormatProbe::new(
            "excel-workbook",
            self.name(),
            Confidence::Likely,
            "Excel workbook detected; spectral table schema will be validated on read",
        ))
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let source = SourceFile::from_path(path, "primary")?;
        let mut workbook = open_workbook_auto(path)
            .map_err(|error| Error::InvalidRecord(format!("Excel open error: {error}")))?;
        let sheet_name = choose_sheet(&workbook)?;
        let range = workbook
            .worksheet_range(&sheet_name)
            .map_err(|error| Error::InvalidRecord(format!("Excel sheet read error: {error}")))?;
        read_sheet_records(
            &range.rows().collect::<Vec<_>>(),
            &sheet_name,
            source,
            self.name(),
        )
    }
}

fn choose_sheet<RS>(workbook: &calamine::Sheets<RS>) -> Result<String>
where
    RS: std::io::Read + std::io::Seek,
{
    let names = workbook.sheet_names();
    if names.is_empty() {
        return Err(Error::InvalidRecord(
            "Excel workbook contains no worksheets".to_string(),
        ));
    }
    Ok(names
        .iter()
        .find(|name| name.eq_ignore_ascii_case("spectra"))
        .cloned()
        .unwrap_or_else(|| names[0].clone()))
}

fn read_sheet_records(
    rows: &[&[Data]],
    sheet_name: &str,
    source: SourceFile,
    reader: &str,
) -> Result<Vec<SpectralRecord>> {
    let Some(header_row) = rows.first() else {
        return Err(Error::InvalidRecord("Excel worksheet is empty".to_string()));
    };
    let headers = header_row.iter().map(cell_string).collect::<Vec<_>>();
    let spectral_columns = headers
        .iter()
        .enumerate()
        .filter_map(|(index, header)| header_number(header).map(|_| index))
        .collect::<Vec<_>>();
    if spectral_columns.is_empty() {
        return Err(Error::InvalidRecord(
            "Excel worksheet contains no numeric spectral headers".to_string(),
        ));
    }
    let axis = spectral_columns
        .iter()
        .map(|index| {
            header_number(&headers[*index]).ok_or_else(|| {
                Error::InvalidRecord("Excel spectral header is not numeric".to_string())
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let mut records = Vec::new();
    for (row_index, row) in rows.iter().enumerate().skip(1) {
        if row.iter().all(|cell| matches!(cell, Data::Empty)) {
            continue;
        }
        let values = spectral_columns
            .iter()
            .map(|column| {
                row.get(*column).and_then(cell_number).ok_or_else(|| {
                    Error::InvalidRecord(format!(
                        "Excel row {row_index} contains a non-numeric spectral value"
                    ))
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let mut metadata = BTreeMap::new();
        metadata.insert("sheet".to_string(), json!(sheet_name));
        metadata.insert("row_index".to_string(), json!(row_index));
        let mut targets = BTreeMap::new();
        for (column, header) in headers.iter().enumerate() {
            if spectral_columns.contains(&column) {
                continue;
            }
            let value = row.get(column).unwrap_or(&Data::Empty);
            if matches!(value, Data::Empty) || header.trim().is_empty() {
                continue;
            }
            let key = normalize_key(header);
            if matches!(key.as_str(), "sample" | "sample_id" | "sampleid" | "id") {
                metadata.insert("sample_id".to_string(), json!(cell_string(value)));
            } else if let Some(number) = cell_number(value) {
                targets.insert(key, json!(number));
            } else {
                metadata.insert(key, json!(cell_string(value)));
            }
        }

        records.push(single_signal_record(
            "excel-xlsx",
            reader,
            source.clone(),
            SingleSignalSpec {
                axis_values: axis.clone(),
                axis_unit: "nm".to_string(),
                axis_kind: AxisKind::Wavelength,
                values,
                signal_name: "absorbance".to_string(),
                signal_type: SignalType::Absorbance,
                signal_unit: None,
                role: "absorbance".to_string(),
            },
            targets,
            metadata,
            Vec::new(),
        )?);
    }

    if records.is_empty() {
        return Err(Error::InvalidRecord(
            "Excel worksheet contains no spectral data rows".to_string(),
        ));
    }
    Ok(records)
}

fn header_number(value: &str) -> Option<f64> {
    parse_number(value)
}

fn cell_number(cell: &Data) -> Option<f64> {
    cell.as_f64()
}

fn cell_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.trim().to_string(),
        other => other.to_string(),
    }
}
