//! Conversions applied to a record before export.
//!
//! Hosts that convert spectra (through nirs4all-methods) list each step under
//! the record metadata key [`CONVERSIONS_KEY`]. Writers report them and write
//! them where the format already holds free text.

use nirs4all_formats_core::SpectralRecord;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{refused, WriteError};

/// Record metadata key listing the conversions applied to its signals.
pub const CONVERSIONS_KEY: &str = "conversions";

/// One conversion step applied to a signal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliedConversion {
    /// Signal produced by the step; every signal when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    /// Plan step name, for example `signal_conversion` or `resample`.
    pub step: String,
    /// Methods catalog id of the computation; none for pure selections.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method_id: Option<String>,
    /// Version of nirs4all-methods that computed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub methods_version: Option<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub params: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

impl AppliedConversion {
    /// One-line summary: step, method and version.
    pub(crate) fn summary(&self) -> String {
        let mut out = self.step.clone();
        if let Some(method) = &self.method_id {
            out.push(' ');
            out.push_str(method);
        }
        if let Some(version) = &self.methods_version {
            out.push_str(&format!(" (nirs4all-methods {version})"));
        }
        out
    }

    /// Parameters as `name = value` text.
    pub(crate) fn param_lines(&self) -> Vec<(String, String)> {
        self.params
            .iter()
            .map(|(name, value)| {
                let text = match value {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                (name.clone(), text)
            })
            .collect()
    }
}

/// Every conversion listed by a record; refused when the key is malformed.
pub(crate) fn listed(record: &SpectralRecord) -> Result<Vec<AppliedConversion>, WriteError> {
    match record.metadata.get(CONVERSIONS_KEY) {
        None => Ok(Vec::new()),
        Some(value) => serde_json::from_value(value.clone()).map_err(|error| {
            refused(format!(
                "record metadata '{CONVERSIONS_KEY}' must be a list of conversion steps ({error})"
            ))
        }),
    }
}

/// Conversions that produced `signal` in a record already checked by
/// [`listed`].
pub(crate) fn applied(record: &SpectralRecord, signal: &str) -> Vec<AppliedConversion> {
    listed(record)
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| entry.signal.as_deref().is_none_or(|name| name == signal))
        .collect()
}

/// Distinct conversions of the exported signals, in record order.
pub(crate) fn exported(
    records: &[SpectralRecord],
    signal: Option<&str>,
) -> Result<Vec<AppliedConversion>, WriteError> {
    let mut out: Vec<AppliedConversion> = Vec::new();
    for record in records {
        for entry in listed(record)? {
            let written = match (&entry.signal, signal) {
                (Some(name), Some(wanted)) => name == wanted,
                (Some(name), None) => record.signals.contains_key(name),
                (None, _) => true,
            };
            if written && !out.contains(&entry) {
                out.push(entry);
            }
        }
    }
    Ok(out)
}
