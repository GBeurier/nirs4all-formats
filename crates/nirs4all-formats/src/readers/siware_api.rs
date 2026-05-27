use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_formats_core::{AxisKind, Confidence, Error, FormatProbe, Result, SignalType};
use serde_json::{json, Value};

use crate::readers::util::{
    read_bytes, single_signal_record, text_lossy_from_bytes, SingleSignalSpec,
};
use crate::Reader;

pub struct SiwareApiReader;

impl Reader for SiwareApiReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::siware_api"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        if path
            .extension()
            .and_then(|value| value.to_str())
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("json"))
        {
            return None;
        }
        let text = String::from_utf8_lossy(head);
        (text.contains("\"measurement\"")
            && text.contains("\"wavelengths\"")
            && text.contains("\"absorbance\""))
        .then(|| {
            FormatProbe::new(
                "siware-api-json",
                self.name(),
                Confidence::Definite,
                "SiWare API JSON spectral measurement detected",
            )
        })
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
        let bytes = read_bytes(path)?;
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(
        &self,
        path: &Path,
        bytes: &[u8],
    ) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
        let (text, source) = text_lossy_from_bytes(path, bytes);
        let root = serde_json::from_str::<Value>(&text)
            .map_err(|error| Error::InvalidRecord(format!("SiWare API JSON error: {error}")))?;
        let measurement = root
            .get("measurement")
            .ok_or_else(|| Error::InvalidRecord("SiWare JSON missing measurement".to_string()))?;
        let axis = number_array(measurement, "wavelengths").ok_or_else(|| {
            Error::InvalidRecord("SiWare JSON missing numeric wavelengths".to_string())
        })?;
        let values = number_array(measurement, "absorbance").ok_or_else(|| {
            Error::InvalidRecord("SiWare JSON missing numeric absorbance".to_string())
        })?;
        if axis.len() != values.len() {
            return Err(Error::InvalidRecord(
                "SiWare wavelength and absorbance lengths differ".to_string(),
            ));
        }

        Ok(vec![single_signal_record(
            "siware-api-json",
            self.name(),
            source,
            SingleSignalSpec {
                axis_values: axis,
                axis_unit: measurement
                    .get("wavelength_units")
                    .and_then(Value::as_str)
                    .unwrap_or("nm")
                    .to_string(),
                axis_kind: AxisKind::Wavelength,
                values,
                signal_name: "absorbance".to_string(),
                signal_type: SignalType::Absorbance,
                signal_unit: None,
                role: "absorbance".to_string(),
            },
            targets(&root),
            metadata(&root),
            Vec::new(),
        )?])
    }
}

fn number_array(value: &Value, key: &str) -> Option<Vec<f64>> {
    value
        .get(key)?
        .as_array()?
        .iter()
        .map(Value::as_f64)
        .collect::<Option<Vec<_>>>()
}

fn targets(root: &Value) -> BTreeMap<String, Value> {
    root.get("predictions")
        .and_then(Value::as_object)
        .map(|predictions| {
            predictions
                .iter()
                .filter_map(|(key, value)| {
                    value.as_f64().map(|number| (key.clone(), json!(number)))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn metadata(root: &Value) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    if let Some(instrument) = root.get("instrument") {
        copy_string(&mut metadata, instrument, "vendor", "instrument_vendor");
        copy_string(&mut metadata, instrument, "model", "instrument_model");
        copy_string(&mut metadata, instrument, "serial", "instrument_serial");
    }
    if let Some(measurement) = root.get("measurement") {
        copy_string(&mut metadata, measurement, "id", "measurement_id");
        copy_string(&mut metadata, measurement, "timestamp", "timestamp");
        copy_string(&mut metadata, measurement, "operator", "operator");
        if let Some(extra) = measurement.get("metadata") {
            if let Some(gps) = extra.get("gps").and_then(Value::as_array) {
                if let (Some(latitude), Some(longitude)) = (
                    gps.first().and_then(Value::as_f64),
                    gps.get(1).and_then(Value::as_f64),
                ) {
                    metadata.insert("gps_latitude".to_string(), json!(latitude));
                    metadata.insert("gps_longitude".to_string(), json!(longitude));
                }
            }
            copy_number(&mut metadata, extra, "temperature_C", "temperature_c");
            copy_number(&mut metadata, extra, "humidity_pct", "humidity_pct");
        }
    }
    metadata
}

fn copy_string(
    metadata: &mut BTreeMap<String, Value>,
    value: &Value,
    source_key: &str,
    metadata_key: &str,
) {
    if let Some(text) = value.get(source_key).and_then(Value::as_str) {
        metadata.insert(metadata_key.to_string(), json!(text));
    }
}

fn copy_number(
    metadata: &mut BTreeMap<String, Value>,
    value: &Value,
    source_key: &str,
    metadata_key: &str,
) {
    if let Some(number) = value.get(source_key).and_then(Value::as_f64) {
        metadata.insert(metadata_key.to_string(), json!(number));
    }
}
