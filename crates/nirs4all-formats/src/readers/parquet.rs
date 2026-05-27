use std::collections::BTreeMap;
use std::path::Path;

use arrow_array::{
    Array, Float32Array, Float64Array, Int32Array, Int64Array, LargeStringArray, StringArray,
};
use arrow_schema::DataType;
use bytes::Bytes;
use nirs4all_formats_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile,
};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde_json::{json, Value};

use crate::readers::util::{single_signal_record, SingleSignalSpec};
use crate::Reader;

pub struct ParquetReader;

impl Reader for ParquetReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::parquet"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        if ext == "parquet" && head.starts_with(b"PAR1") {
            Some(FormatProbe::new(
                "parquet-container",
                self.name(),
                Confidence::Likely,
                "Apache Parquet container detected; schema validated on read",
            ))
        } else {
            None
        }
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        decode_parquet(path, &bytes, self.name())
    }

    fn read_bytes(
        &self,
        name: &Path,
        bytes: &[u8],
    ) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
        decode_parquet(name, bytes, self.name())
    }
}

fn decode_parquet(
    name: &Path,
    bytes: &[u8],
    reader_name: &str,
) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
    let source = SourceFile::from_bytes(name, bytes, "primary");
    // `bytes::Bytes` implements `parquet::file::reader::ChunkReader` and
    // serves the file from RAM, so the same decode path covers both the
    // filesystem-mode read_path and the in-memory read_bytes.
    let buffer = Bytes::copy_from_slice(bytes);
    let builder = ParquetRecordBatchReaderBuilder::try_new(buffer)
        .map_err(|error| Error::InvalidRecord(format!("Parquet reader error: {error}")))?;
    let schema = builder.schema().clone();
    let layout = spectral_layout(
        schema
            .fields()
            .iter()
            .enumerate()
            .map(|(index, field)| (index, field.name().to_string(), field.data_type().clone())),
    )?;

    let batch_reader = builder
        .with_batch_size(1024)
        .build()
        .map_err(|error| Error::InvalidRecord(format!("Parquet batch reader error: {error}")))?;
    let mut records = Vec::new();
    for batch in batch_reader {
        let batch =
            batch.map_err(|error| Error::InvalidRecord(format!("Parquet batch error: {error}")))?;
        for row in 0..batch.num_rows() {
            let mut values = Vec::with_capacity(layout.spectral_columns.len());
            for column in &layout.spectral_columns {
                values.push(numeric_value(
                    batch.column(column.index).as_ref(),
                    row,
                    &column.name,
                )?);
            }

            let mut metadata = BTreeMap::<String, Value>::new();
            metadata.insert("row_index".to_string(), json!(records.len()));
            metadata.insert(
                "parquet".to_string(),
                json!({
                    "spectral_column_count": layout.spectral_columns.len(),
                    "target_column_count": layout.target_columns.len(),
                }),
            );
            if let Some(sample_id_column) = layout.sample_id_column {
                if let Some(sample_id) = string_value(batch.column(sample_id_column).as_ref(), row)?
                {
                    metadata.insert("sample_id".to_string(), json!(sample_id));
                }
            }

            let mut targets = BTreeMap::<String, Value>::new();
            for column in &layout.target_columns {
                let value = target_value(batch.column(column.index).as_ref(), row, &column.name)?;
                targets.insert(column.name.clone(), value);
            }

            records.push(single_signal_record(
                "parquet-nirs-table",
                reader_name,
                source.clone(),
                SingleSignalSpec {
                    axis_values: layout.axis.clone(),
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
    }

    if records.is_empty() {
        return Err(Error::InvalidRecord(
            "Parquet spectral table contains no rows".to_string(),
        ));
    }
    Ok(records)
}

struct ParquetLayout {
    axis: Vec<f64>,
    spectral_columns: Vec<ColumnRef>,
    target_columns: Vec<ColumnRef>,
    sample_id_column: Option<usize>,
}

struct ColumnRef {
    index: usize,
    name: String,
}

fn spectral_layout(
    fields: impl IntoIterator<Item = (usize, String, DataType)>,
) -> Result<ParquetLayout> {
    let mut spectral_columns = Vec::new();
    let mut target_columns = Vec::new();
    let mut sample_id_column = None;
    let mut axis = Vec::new();

    for (index, name, data_type) in fields {
        if let Ok(axis_value) = name.parse::<f64>() {
            if is_float(&data_type) {
                axis.push(axis_value);
                spectral_columns.push(ColumnRef { index, name });
            }
            continue;
        }
        if is_sample_id_column(&name, &data_type) {
            sample_id_column = Some(index);
        } else if is_numeric_target(&data_type) {
            target_columns.push(ColumnRef { index, name });
        }
    }

    if spectral_columns.len() < 8 {
        return Err(Error::InvalidRecord(
            "Parquet table is not a NIRS spectral table: fewer than 8 numeric wavelength columns"
                .to_string(),
        ));
    }
    if !axis.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(Error::InvalidRecord(
            "Parquet spectral axis is not strictly ascending".to_string(),
        ));
    }

    Ok(ParquetLayout {
        axis,
        spectral_columns,
        target_columns,
        sample_id_column,
    })
}

fn is_float(data_type: &DataType) -> bool {
    matches!(data_type, DataType::Float32 | DataType::Float64)
}

fn is_numeric_target(data_type: &DataType) -> bool {
    matches!(
        data_type,
        DataType::Float32 | DataType::Float64 | DataType::Int32 | DataType::Int64
    )
}

fn is_sample_id_column(name: &str, data_type: &DataType) -> bool {
    matches!(name, "sample_id" | "sample" | "id")
        && matches!(data_type, DataType::Utf8 | DataType::LargeUtf8)
}

fn numeric_value(array: &dyn Array, row: usize, name: &str) -> Result<f64> {
    if array.is_null(row) {
        return Err(Error::InvalidRecord(format!(
            "Parquet spectral value is null in column {name}"
        )));
    }
    if let Some(array) = array.as_any().downcast_ref::<Float64Array>() {
        Ok(array.value(row))
    } else if let Some(array) = array.as_any().downcast_ref::<Float32Array>() {
        Ok(array.value(row) as f64)
    } else {
        Err(Error::InvalidRecord(format!(
            "Parquet spectral column {name} is not float32/float64"
        )))
    }
}

fn target_value(array: &dyn Array, row: usize, name: &str) -> Result<Value> {
    if array.is_null(row) {
        return Ok(Value::Null);
    }
    if let Some(array) = array.as_any().downcast_ref::<Float64Array>() {
        Ok(json!(array.value(row)))
    } else if let Some(array) = array.as_any().downcast_ref::<Float32Array>() {
        Ok(json!(array.value(row) as f64))
    } else if let Some(array) = array.as_any().downcast_ref::<Int32Array>() {
        Ok(json!(array.value(row)))
    } else if let Some(array) = array.as_any().downcast_ref::<Int64Array>() {
        Ok(json!(array.value(row)))
    } else {
        Err(Error::InvalidRecord(format!(
            "Parquet target column {name} has unsupported type"
        )))
    }
}

fn string_value(array: &dyn Array, row: usize) -> Result<Option<String>> {
    if array.is_null(row) {
        return Ok(None);
    }
    if let Some(array) = array.as_any().downcast_ref::<StringArray>() {
        Ok(Some(array.value(row).to_string()))
    } else if let Some(array) = array.as_any().downcast_ref::<LargeStringArray>() {
        Ok(Some(array.value(row).to_string()))
    } else {
        Err(Error::InvalidRecord(
            "Parquet sample_id column is not a string".to_string(),
        ))
    }
}
