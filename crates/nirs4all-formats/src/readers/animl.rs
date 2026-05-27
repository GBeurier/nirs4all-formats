use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_formats_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SpectralArray, SpectralAxis,
    SpectralRecord,
};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader as XmlReader;
use serde_json::{json, Value};

use crate::readers::util::{
    normalize_key, parse_number, provenance, read_bytes, safe_signal_name, signal_type_from_label,
    text_lossy_from_bytes,
};
use crate::Reader;

pub struct AnimlReader;

impl Reader for AnimlReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::animl"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let text = String::from_utf8_lossy(head);
        (ext == "animl" && (text.contains("<AnIML") || text.contains(":AnIML"))).then(|| {
            FormatProbe::new(
                "animl",
                self.name(),
                Confidence::Definite,
                "AnIML XML document detected",
            )
        })
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let bytes = read_bytes(path)?;
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(
        &self,
        path: &Path,
        bytes: &[u8],
    ) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
        let (text, source) = text_lossy_from_bytes(path, bytes);
        let parsed = parse_animl_text(&text)?;
        let record = build_animl_record(self.name(), source, parsed)?;
        Ok(vec![record])
    }
}

#[derive(Default)]
struct ParsedAniml {
    sample_id: Option<String>,
    sample_name: Option<String>,
    targets: BTreeMap<String, Value>,
    series: Vec<AnimlSeries>,
}

struct AnimlSeries {
    id: String,
    name: String,
    unit: Option<String>,
    values: Vec<f64>,
}

struct CurrentSeries {
    id: String,
    name: String,
    unit: Option<String>,
    values: Vec<f64>,
}

#[derive(Default)]
struct CurrentAutoValues {
    start_index: usize,
    end_index: Option<usize>,
    start_value: Option<f64>,
    increment: Option<f64>,
}

fn parse_animl_text(text: &str) -> Result<ParsedAniml> {
    let mut reader = XmlReader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut parsed = ParsedAniml::default();
    let mut stack = Vec::<String>::new();
    let mut current_series: Option<CurrentSeries> = None;
    let mut current_series_set_length: Option<usize> = None;
    let mut current_auto_values: Option<CurrentAutoValues> = None;
    let mut current_parameter: Option<String> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = tag_name(&event);
                match tag.as_str() {
                    "Sample" => {
                        parsed.sample_id = attr_value(&event, "sampleID").or(parsed.sample_id);
                        parsed.sample_name = attr_value(&event, "name").or(parsed.sample_name);
                    }
                    "Parameter" => {
                        current_parameter = attr_value(&event, "name");
                    }
                    "Series" => {
                        current_series = Some(CurrentSeries {
                            id: attr_value(&event, "seriesID").unwrap_or_default(),
                            name: attr_value(&event, "name")
                                .unwrap_or_else(|| "signal".to_string()),
                            unit: None,
                            values: Vec::new(),
                        });
                    }
                    "SeriesSet" => {
                        current_series_set_length = attr_usize(&event, "length");
                    }
                    "AutoIncrementedValueSet" => {
                        current_auto_values = Some(CurrentAutoValues {
                            start_index: attr_usize(&event, "startIndex")
                                .or_else(|| attr_usize(&event, "start_index"))
                                .unwrap_or(0),
                            end_index: attr_usize(&event, "endIndex")
                                .or_else(|| attr_usize(&event, "end_index")),
                            start_value: None,
                            increment: None,
                        });
                    }
                    "Unit" => {
                        if let Some(series) = &mut current_series {
                            series.unit =
                                attr_value(&event, "label").or_else(|| series.unit.clone());
                        }
                    }
                    _ => {}
                }
                stack.push(tag);
            }
            Ok(Event::Empty(event)) => {
                let tag = tag_name(&event);
                if tag == "Sample" {
                    parsed.sample_id = attr_value(&event, "sampleID").or(parsed.sample_id);
                    parsed.sample_name = attr_value(&event, "name").or(parsed.sample_name);
                } else if tag == "Unit" {
                    if let Some(series) = &mut current_series {
                        series.unit = attr_value(&event, "label").or_else(|| series.unit.clone());
                    }
                }
            }
            Ok(Event::Text(event)) => {
                let text = event
                    .decode()
                    .map_err(|error| Error::InvalidRecord(format!("AnIML text error: {error}")))?;
                if stack.last().is_some_and(|tag| tag == "F" || tag == "D") {
                    if let Some(value) = parse_number(&text) {
                        if let Some(auto_values) = &mut current_auto_values {
                            if stack.iter().any(|tag| tag == "StartValue") {
                                auto_values.start_value = Some(value);
                            } else if stack.iter().any(|tag| tag == "Increment") {
                                auto_values.increment = Some(value);
                            }
                        } else if let Some(series) = &mut current_series {
                            series.values.push(value);
                        } else if stack.iter().any(|tag| tag == "SampleSet") {
                            if let Some(parameter) = &current_parameter {
                                parsed
                                    .targets
                                    .insert(normalize_key(parameter), json!(value));
                            }
                        }
                    }
                }
            }
            Ok(Event::End(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "Series" {
                    if let Some(series) = current_series.take() {
                        parsed.series.push(AnimlSeries {
                            id: series.id,
                            name: series.name,
                            unit: series.unit,
                            values: series.values,
                        });
                    }
                } else if tag == "AutoIncrementedValueSet" {
                    if let Some(auto_values) = current_auto_values.take() {
                        let values = expand_auto_values(auto_values, current_series_set_length)?;
                        if let Some(series) = &mut current_series {
                            series.values.extend(values);
                        }
                    }
                } else if tag == "SeriesSet" {
                    current_series_set_length = None;
                } else if tag == "Parameter" {
                    current_parameter = None;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(Error::InvalidRecord(format!("AnIML XML error: {error}")));
            }
            _ => {}
        }
    }

    Ok(parsed)
}

fn expand_auto_values(
    values: CurrentAutoValues,
    series_set_length: Option<usize>,
) -> Result<Vec<f64>> {
    if values.start_index != 0 {
        return Err(Error::InvalidRecord(
            "AnIML AutoIncrementedValueSet with non-zero startIndex is not supported yet"
                .to_string(),
        ));
    }
    let start_value = values.start_value.ok_or_else(|| {
        Error::InvalidRecord("AnIML AutoIncrementedValueSet missing StartValue".to_string())
    })?;
    let increment = values.increment.ok_or_else(|| {
        Error::InvalidRecord("AnIML AutoIncrementedValueSet missing Increment".to_string())
    })?;
    let count = if let Some(end_index) = values.end_index {
        end_index.checked_add(1).ok_or_else(|| {
            Error::InvalidRecord("AnIML AutoIncrementedValueSet endIndex overflow".to_string())
        })?
    } else {
        series_set_length.ok_or_else(|| {
            Error::InvalidRecord(
                "AnIML AutoIncrementedValueSet missing endIndex and SeriesSet length".to_string(),
            )
        })?
    };
    Ok((0..count)
        .map(|index| start_value + increment * index as f64)
        .collect())
}

fn build_animl_record(
    reader: &str,
    source: nirs4all_formats_core::SourceFile,
    parsed: ParsedAniml,
) -> Result<SpectralRecord> {
    let axis_series = parsed
        .series
        .iter()
        .find(|series| is_axis_series(series))
        .ok_or_else(|| {
            Error::InvalidRecord("AnIML contains no supported axis series".to_string())
        })?;
    let (axis_kind, axis_unit) = axis_kind_unit(axis_series);
    let mut signals = BTreeMap::new();
    let mut dominant = SignalType::Unknown;

    for series in parsed
        .series
        .iter()
        .filter(|series| !is_axis_series(series))
    {
        if series.values.len() != axis_series.values.len() {
            continue;
        }
        let signal_type = signal_type_from_label(&series.name);
        dominant = choose_dominant(&dominant, &signal_type);
        let name = safe_signal_name(&series.name, "signal");
        let signal = SpectralArray::new(
            SpectralAxis::new(
                axis_series.values.clone(),
                axis_unit.clone(),
                axis_kind.clone(),
            )?,
            series.values.clone(),
            vec!["x".to_string()],
            signal_type,
            series.unit.clone(),
            name.clone(),
            "file",
        )?;
        signals.insert(name, signal);
    }

    if signals.is_empty() {
        return Err(Error::InvalidRecord(
            "AnIML contains no supported spectral signal series".to_string(),
        ));
    }

    let mut metadata = BTreeMap::new();
    if let Some(sample_id) = parsed.sample_id {
        metadata.insert("sample_id".to_string(), json!(sample_id));
    }
    if let Some(sample_name) = parsed.sample_name {
        metadata.insert("sample_name".to_string(), json!(sample_name));
    }

    let record = SpectralRecord {
        signals,
        signal_type: dominant,
        targets: parsed.targets,
        metadata,
        provenance: provenance("animl", reader, source, Vec::new()),
        quality_flags: Vec::new(),
    };
    record.validate()?;
    Ok(record)
}

fn tag_name(event: &BytesStart<'_>) -> String {
    local_name(event.name().as_ref())
}

fn local_name(name: &[u8]) -> String {
    let local = name
        .iter()
        .rposition(|byte| *byte == b':')
        .map_or(name, |index| &name[index + 1..]);
    String::from_utf8_lossy(local).into_owned()
}

fn attr_value(event: &BytesStart<'_>, key: &str) -> Option<String> {
    event
        .attributes()
        .flatten()
        .find(|attr| attr.key.as_ref() == key.as_bytes())
        .map(|attr| String::from_utf8_lossy(attr.value.as_ref()).into_owned())
}

fn attr_usize(event: &BytesStart<'_>, key: &str) -> Option<usize> {
    attr_value(event, key)?.parse().ok()
}

fn is_axis_series(series: &AnimlSeries) -> bool {
    let lower = format!("{} {}", series.id, series.name).to_ascii_lowercase();
    lower.contains("wavelength") || lower.contains("wavenumber")
}

fn axis_kind_unit(series: &AnimlSeries) -> (AxisKind, String) {
    let lower = format!(
        "{} {} {}",
        series.id,
        series.name,
        series.unit.clone().unwrap_or_default()
    )
    .to_ascii_lowercase();
    if lower.contains("wavenumber") || lower.contains("cm-1") {
        (AxisKind::Wavenumber, "cm-1".to_string())
    } else {
        (
            AxisKind::Wavelength,
            series.unit.clone().unwrap_or_else(|| "nm".to_string()),
        )
    }
}

fn choose_dominant(current: &SignalType, candidate: &SignalType) -> SignalType {
    if signal_priority(candidate) > signal_priority(current) {
        candidate.clone()
    } else {
        current.clone()
    }
}

fn signal_priority(signal_type: &SignalType) -> u8 {
    match signal_type {
        SignalType::Absorbance
        | SignalType::Reflectance
        | SignalType::Transmittance
        | SignalType::Irradiance
        | SignalType::Radiance
        | SignalType::AerosolOpticalThickness => 4,
        SignalType::KubelkaMunk | SignalType::Derivative | SignalType::Preprocessed => 3,
        SignalType::RawCounts | SignalType::SingleBeam | SignalType::Interferogram => 2,
        SignalType::Uncertainty => 1,
        SignalType::Unknown => 0,
    }
}
