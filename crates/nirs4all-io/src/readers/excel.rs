use std::collections::BTreeMap;
use std::path::Path;

use calamine::{open_workbook_auto, Data, DataType, Reader};
use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralRecord,
};
use serde_json::{json, Value};

use crate::readers::util::{
    normalize_key, parse_number, safe_signal_name, signal_type_from_label, single_signal_record,
    SingleSignalSpec,
};
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
        let mut records = read_sheet_records(
            &range.rows().collect::<Vec<_>>(),
            &sheet_name,
            source,
            self.name(),
        )?;
        merge_optional_aux_sheet(
            &mut workbook,
            &mut records,
            &["metadata", "meta", "samples"],
            AuxRole::Metadata,
        )?;
        merge_optional_aux_sheet(
            &mut workbook,
            &mut records,
            &["references", "reference", "targets"],
            AuxRole::Targets,
        )?;
        Ok(records)
    }
}

#[derive(Clone, Copy)]
enum AuxRole {
    Metadata,
    Targets,
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

fn find_sheet<RS>(workbook: &calamine::Sheets<RS>, candidates: &[&str]) -> Option<String>
where
    RS: std::io::Read + std::io::Seek,
{
    workbook
        .sheet_names()
        .iter()
        .find(|name| {
            candidates
                .iter()
                .any(|candidate| name.eq_ignore_ascii_case(candidate))
        })
        .cloned()
}

fn merge_optional_aux_sheet<RS>(
    workbook: &mut calamine::Sheets<RS>,
    records: &mut [SpectralRecord],
    candidate_names: &[&str],
    role: AuxRole,
) -> Result<()>
where
    RS: std::io::Read + std::io::Seek,
{
    let Some(sheet_name) = find_sheet(workbook, candidate_names) else {
        return Ok(());
    };
    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|error| Error::InvalidRecord(format!("Excel sheet read error: {error}")))?;
    let rows = range.rows().map(|row| row.to_vec()).collect::<Vec<_>>();
    merge_aux_sheet(records, &sheet_name, &rows, role)
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
    let descriptor = axis_descriptor(&headers, &spectral_columns);
    let (axis_kind, axis_unit) = axis_kind_unit(descriptor);
    let signal_label = descriptor
        .and_then(data_label)
        .unwrap_or_else(|| "absorbance".to_string());
    let signal_type = signal_type_from_label(&signal_label);
    let signal_name = canonical_signal_name(&signal_label, &signal_type);
    let signal_unit = descriptor.and_then(data_unit);

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
        if let Some(descriptor) = descriptor {
            metadata.insert("axis_descriptor".to_string(), json!(descriptor));
        }
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
            if is_sample_id_column(header, column) {
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
                axis_unit: axis_unit.clone(),
                axis_kind: axis_kind.clone(),
                values,
                signal_name: signal_name.clone(),
                signal_type: signal_type.clone(),
                signal_unit: signal_unit.clone(),
                role: signal_name.clone(),
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

fn merge_aux_sheet(
    records: &mut [SpectralRecord],
    sheet_name: &str,
    rows: &[Vec<Data>],
    role: AuxRole,
) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let headers = rows[0].iter().map(cell_string).collect::<Vec<_>>();
    let sample_column = headers
        .iter()
        .position(|header| is_sample_id_header(header))
        .ok_or_else(|| {
            Error::InvalidRecord(format!(
                "Excel auxiliary sheet {sheet_name} contains no sample_id column"
            ))
        })?;
    let sample_index = sample_index(records)?;

    for (row_index, row) in rows.iter().enumerate().skip(1) {
        if row.iter().all(|cell| matches!(cell, Data::Empty)) {
            continue;
        }
        let sample_id = row.get(sample_column).map(cell_string).unwrap_or_default();
        if sample_id.trim().is_empty() {
            return Err(Error::InvalidRecord(format!(
                "Excel auxiliary sheet {sheet_name} row {row_index} has an empty sample_id"
            )));
        }
        let Some(record_index) = sample_index.get(&sample_id) else {
            return Err(Error::InvalidRecord(format!(
                "Excel auxiliary sheet {sheet_name} references unknown sample_id {sample_id}"
            )));
        };
        let record = &mut records[*record_index];
        match role {
            AuxRole::Metadata => {
                record
                    .metadata
                    .insert("metadata_sheet".to_string(), json!(sheet_name));
            }
            AuxRole::Targets => {
                record
                    .metadata
                    .insert("reference_sheet".to_string(), json!(sheet_name));
            }
        }

        for (column, header) in headers.iter().enumerate() {
            if column == sample_column || header.trim().is_empty() {
                continue;
            }
            let Some(value) = row.get(column) else {
                continue;
            };
            if matches!(value, Data::Empty) {
                continue;
            }
            let key = normalize_key(header);
            match role {
                AuxRole::Metadata => {
                    record.metadata.insert(key, cell_json(value));
                }
                AuxRole::Targets => {
                    let number = cell_number(value).ok_or_else(|| {
                        Error::InvalidRecord(format!(
                            "Excel references sheet {sheet_name} row {row_index} column {header} is not numeric"
                        ))
                    })?;
                    record.targets.insert(key, json!(number));
                }
            }
        }
    }

    Ok(())
}

fn sample_index(records: &[SpectralRecord]) -> Result<BTreeMap<String, usize>> {
    let mut index = BTreeMap::new();
    for (record_index, record) in records.iter().enumerate() {
        let Some(sample_id) = record
            .metadata
            .get("sample_id")
            .and_then(|value| value.as_str())
        else {
            return Err(Error::InvalidRecord(
                "Excel auxiliary sheets require sample_id values in the spectra sheet".to_string(),
            ));
        };
        if index.insert(sample_id.to_string(), record_index).is_some() {
            return Err(Error::InvalidRecord(format!(
                "Excel spectra sheet contains duplicate sample_id {sample_id}"
            )));
        }
    }
    Ok(index)
}

fn is_sample_id_header(header: &str) -> bool {
    matches!(
        normalize_key(header).as_str(),
        "sample" | "sample_id" | "sampleid" | "id"
    )
}

fn is_sample_id_column(header: &str, column: usize) -> bool {
    is_sample_id_header(header) || (column == 0 && is_axis_descriptor(header))
}

fn axis_descriptor<'a>(headers: &'a [String], spectral_columns: &[usize]) -> Option<&'a str> {
    let first_spectral_column = *spectral_columns.iter().min()?;
    headers
        .iter()
        .take(first_spectral_column)
        .find(|header| is_axis_descriptor(header))
        .map(String::as_str)
}

fn is_axis_descriptor(header: &str) -> bool {
    let lower = header.trim().to_ascii_lowercase();
    lower.starts_with("axis:") || lower.starts_with("axis ")
}

fn axis_kind_unit(descriptor: Option<&str>) -> (AxisKind, String) {
    let lower = descriptor.unwrap_or_default().to_ascii_lowercase();
    if lower.contains("wavenumber") || lower.contains("cm-1") || lower.contains("cm^-1") {
        (AxisKind::Wavenumber, "cm-1".to_string())
    } else {
        (AxisKind::Wavelength, "nm".to_string())
    }
}

fn data_label(descriptor: &str) -> Option<String> {
    descriptor
        .split('/')
        .find_map(|part| strip_label_prefix(part, "data:"))
        .map(|label| label.split('(').next().unwrap_or(label).trim().to_string())
        .filter(|label| !label.is_empty())
}

fn data_unit(descriptor: &str) -> Option<String> {
    let data_part = descriptor
        .split('/')
        .find_map(|part| strip_label_prefix(part, "data:"))?;
    let start = data_part.find('(')?;
    let end = data_part[start + 1..].find(')')? + start + 1;
    let unit = data_part[start + 1..end].trim();
    if unit.is_empty() || unit == "-" {
        None
    } else {
        Some(unit.to_string())
    }
}

fn strip_label_prefix<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let trimmed = value.trim();
    if trimmed.len() < prefix.len() || !trimmed[..prefix.len()].eq_ignore_ascii_case(prefix) {
        return None;
    }
    Some(trimmed[prefix.len()..].trim())
}

fn canonical_signal_name(label: &str, signal_type: &SignalType) -> String {
    match signal_type {
        SignalType::Absorbance => "absorbance".to_string(),
        SignalType::Reflectance => "reflectance".to_string(),
        SignalType::Transmittance => "transmittance".to_string(),
        SignalType::Radiance => "radiance".to_string(),
        SignalType::Irradiance => "irradiance".to_string(),
        SignalType::RawCounts => "raw".to_string(),
        SignalType::SingleBeam => "single_beam".to_string(),
        SignalType::Interferogram => "interferogram".to_string(),
        SignalType::KubelkaMunk => "kubelka_munk".to_string(),
        SignalType::Derivative => "derivative".to_string(),
        SignalType::Preprocessed => "preprocessed".to_string(),
        SignalType::AerosolOpticalThickness => "aot".to_string(),
        SignalType::Uncertainty => "uncertainty".to_string(),
        SignalType::Unknown => safe_signal_name(label, "signal"),
    }
}

fn header_number(value: &str) -> Option<f64> {
    parse_number(value)
}

fn cell_number(cell: &Data) -> Option<f64> {
    cell.as_f64()
}

fn cell_json(cell: &Data) -> Value {
    match cell {
        Data::Bool(value) => json!(value),
        Data::Int(value) => json!(value),
        Data::Float(value) => json!(value),
        _ => json!(cell_string(cell)),
    }
}

fn cell_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.trim().to_string(),
        other => other.to_string(),
    }
}
