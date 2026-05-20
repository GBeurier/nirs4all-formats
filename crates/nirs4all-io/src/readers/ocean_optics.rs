use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralArray,
    SpectralAxis,
};
use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;
use sha2::{Digest, Sha512};
use zip::ZipArchive;

use crate::readers::util::{
    metadata_from_pairs, parse_number, read_text_lossy, record_from_signals, safe_signal_name,
};
use crate::Reader;

pub struct OceanOpticsReader;

impl Reader for OceanOpticsReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::ocean_optics"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext == "procspec" && head.starts_with(b"PK") {
            return Some(FormatProbe::new(
                "ocean-optics-procspec",
                self.name(),
                Confidence::Definite,
                "Ocean Optics/OceanView ProcSpec ZIP archive detected",
            ));
        }
        let text = String::from_utf8_lossy(head);
        let normalized = text.replace('\r', "\n");
        if normalized.contains("SpectraSuite Data File")
            || normalized.contains("OOIBase32 Version")
            || normalized.contains("Jaz Data File")
            || normalized.contains("Jaz Absolute Irradiance File")
            || (normalized.contains("Data from ") && normalized.contains("Begin Spectral Data"))
        {
            return Some(FormatProbe::new(
                "ocean-optics-text",
                self.name(),
                Confidence::Definite,
                "Ocean Optics/OceanView ASCII export detected",
            ));
        }
        if normalized.contains("SciMode:") && normalized.lines().any(is_numeric_pair_line) {
            return Some(FormatProbe::new(
                "ocean-optics-craic-text",
                self.name(),
                Confidence::Likely,
                "CRAIC/Ocean-style two-column text export detected",
            ));
        }
        if ext == "csv"
            && normalized
                .lines()
                .find(|line| !line.trim().is_empty())
                .is_some_and(is_numeric_pair_line)
            && normalized
                .lines()
                .take(10)
                .filter(|line| is_numeric_pair_line(line))
                .count()
                >= 5
        {
            return Some(FormatProbe::new(
                "ocean-optics-two-column-csv",
                self.name(),
                Confidence::Likely,
                "two-column spectral CSV export detected",
            ));
        }
        None
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        if path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("procspec"))
        {
            let (source, parsed) = parse_procspec_archive(path)?;
            let dominant = parsed
                .signals
                .get("processed")
                .map(|signal| signal.signal_type.clone())
                .unwrap_or_else(|| dominant_signal_type(&parsed.signals));
            let record = record_from_signals(
                "ocean-optics-procspec",
                self.name(),
                source,
                parsed.signals,
                dominant,
                metadata_from_pairs(parsed.metadata_pairs),
                parsed.warnings,
            )?;
            return Ok(vec![record]);
        }

        let (text, source) = read_text_lossy(path)?;
        let parsed = parse_ocean_text(&text, path)?;
        let signals = signals_from_columns(&parsed)?;
        let dominant = dominant_signal_type(&signals);
        let record = record_from_signals(
            parsed.format,
            self.name(),
            source,
            signals,
            dominant,
            metadata_from_pairs(parsed.metadata_pairs),
            parsed.warnings,
        )?;
        Ok(vec![record])
    }
}

struct ParsedOceanText {
    format: &'static str,
    metadata_pairs: Vec<(String, String)>,
    column_labels: Vec<String>,
    rows: Vec<Vec<f64>>,
    warnings: Vec<String>,
}

struct ParsedProcSpec {
    signals: BTreeMap<String, SpectralArray>,
    metadata_pairs: Vec<(String, String)>,
    warnings: Vec<String>,
}

#[derive(Default)]
struct ProcSpecXml {
    wavelengths: Vec<f64>,
    sample: Vec<f64>,
    dark_reference: Vec<f64>,
    white_reference: Vec<f64>,
    processed: Vec<f64>,
    metadata: BTreeMap<String, String>,
}

fn parse_procspec_archive(path: &Path) -> Result<(SourceFile, ParsedProcSpec)> {
    let bytes = std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let source = SourceFile::from_bytes(path, &bytes, "primary");
    let mut archive = ZipArchive::new(Cursor::new(bytes.as_slice())).map_err(|error| {
        Error::InvalidRecord(format!("Ocean Optics ProcSpec ZIP error: {error}"))
    })?;

    let mut names = Vec::new();
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(|error| {
            Error::InvalidRecord(format!("Ocean Optics ProcSpec ZIP member error: {error}"))
        })?;
        names.push(file.name().to_string());
    }
    let xml_name = names
        .iter()
        .find(|name| {
            name.rsplit('/')
                .next()
                .is_some_and(|file| file.starts_with("ps_") && file.ends_with(".xml"))
        })
        .cloned()
        .ok_or_else(|| {
            Error::InvalidRecord("Ocean Optics ProcSpec missing ps_*.xml".to_string())
        })?;

    let mut xml_bytes = Vec::new();
    archive
        .by_name(&xml_name)
        .map_err(|error| {
            Error::InvalidRecord(format!("Ocean Optics ProcSpec XML member error: {error}"))
        })?
        .read_to_end(&mut xml_bytes)
        .map_err(|error| Error::Io {
            path: path.to_path_buf(),
            source: error,
        })?;
    let xml = String::from_utf8_lossy(&xml_bytes).into_owned();

    let mut warnings = Vec::new();
    let signature_status = if names.iter().any(|name| name.ends_with("OOISignatures.xml")) {
        let mut signature = String::new();
        archive
            .by_name("OOISignatures.xml")
            .map_err(|error| {
                Error::InvalidRecord(format!(
                    "Ocean Optics ProcSpec signature member error: {error}"
                ))
            })?
            .read_to_string(&mut signature)
            .map_err(|error| Error::Io {
                path: path.to_path_buf(),
                source: error,
            })?;
        verify_procspec_signature(&signature, &xml_name, &xml_bytes, &mut warnings)
    } else {
        warnings.push("ocean_optics_procspec_missing_signature".to_string());
        "missing".to_string()
    };

    let parsed_xml = parse_procspec_xml(&xml)?;
    let (processed_name, processed_type, processed_unit) = procspec_processed_mapping(&xml);
    let axis = parsed_xml.wavelengths;
    if axis.is_empty() {
        return Err(Error::InvalidRecord(
            "Ocean Optics ProcSpec missing channelWavelengths".to_string(),
        ));
    }

    let mut signals = BTreeMap::new();
    push_procspec_signal(
        &mut signals,
        &axis,
        "sample",
        parsed_xml.sample,
        SignalType::RawCounts,
        None,
    )?;
    push_procspec_signal(
        &mut signals,
        &axis,
        "dark_reference",
        parsed_xml.dark_reference,
        SignalType::RawCounts,
        None,
    )?;
    push_procspec_signal(
        &mut signals,
        &axis,
        "white_reference",
        parsed_xml.white_reference,
        SignalType::RawCounts,
        None,
    )?;
    push_procspec_signal(
        &mut signals,
        &axis,
        processed_name,
        parsed_xml.processed,
        processed_type,
        processed_unit,
    )?;
    if signals.is_empty() {
        return Err(Error::InvalidRecord(
            "Ocean Optics ProcSpec contains no spectral arrays".to_string(),
        ));
    }

    let mut metadata_pairs = parsed_xml
        .metadata
        .into_iter()
        .collect::<Vec<(String, String)>>();
    metadata_pairs.push(("archive_member".to_string(), xml_name));
    metadata_pairs.push(("signature_status".to_string(), signature_status));
    metadata_pairs.push((
        "processed_signal_type".to_string(),
        processed_name.to_string(),
    ));
    metadata_pairs.push(("zip_members".to_string(), names.join(";")));
    if let Some(file_name) = path.file_name().and_then(|value| value.to_str()) {
        metadata_pairs.push(("file_name".to_string(), file_name.to_string()));
    }

    Ok((
        source,
        ParsedProcSpec {
            signals,
            metadata_pairs,
            warnings,
        },
    ))
}

fn procspec_processed_mapping(xml: &str) -> (&'static str, SignalType, Option<String>) {
    let lower = xml.to_ascii_lowercase();
    if lower.contains("transmissioncoreprocessor")
        || lower.contains("transmissionintensitydescriptor")
    {
        (
            "transmittance",
            SignalType::Transmittance,
            Some("%".to_string()),
        )
    } else if lower.contains("reflectioncoreprocessor")
        || lower.contains("reflectionintensitydescriptor")
    {
        (
            "reflectance",
            SignalType::Reflectance,
            Some("%".to_string()),
        )
    } else if lower.contains("absorbancecoreprocessor")
        || lower.contains("absorbanceintensitydescriptor")
    {
        ("absorbance", SignalType::Absorbance, None)
    } else {
        ("processed", SignalType::Unknown, None)
    }
}

fn parse_procspec_xml(xml: &str) -> Result<ProcSpecXml> {
    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut stack = Vec::<String>::new();
    let mut parsed = ProcSpecXml::default();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                stack.push(String::from_utf8_lossy(event.name().as_ref()).into_owned());
            }
            Ok(Event::Text(event)) => {
                let text = event.decode().map_err(|error| {
                    Error::InvalidRecord(format!("ProcSpec XML text error: {error}"))
                })?;
                if stack.last().is_some_and(|tag| tag == "double") {
                    if let Some(value) = parse_number(&text) {
                        match procspec_array_from_stack(&stack) {
                            Some("wavelengths") => parsed.wavelengths.push(value),
                            Some("sample") => parsed.sample.push(value),
                            Some("dark_reference") => parsed.dark_reference.push(value),
                            Some("white_reference") => parsed.white_reference.push(value),
                            Some("processed") => parsed.processed.push(value),
                            _ => {}
                        }
                    }
                } else if let Some(tag) = stack.last() {
                    if procspec_metadata_tag(tag) {
                        parsed
                            .metadata
                            .entry(tag.to_string())
                            .or_insert_with(|| text.trim().to_string());
                    }
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(Error::InvalidRecord(format!(
                    "Ocean Optics ProcSpec XML error: {error}"
                )));
            }
            _ => {}
        }
    }

    Ok(parsed)
}

fn procspec_array_from_stack(stack: &[String]) -> Option<&'static str> {
    let parent = stack.iter().rev().nth(1)?.as_str();
    match parent {
        "channelWavelengths" => Some("wavelengths"),
        "processedPixels" => Some("processed"),
        "pixelValues" if stack_contains(stack, "darkSpectrum") => Some("dark_reference"),
        "pixelValues" if stack_contains(stack, "referenceSpectrum") => Some("white_reference"),
        "pixelValues" if stack_contains(stack, "sourceSpectra") => Some("sample"),
        _ => None,
    }
}

fn stack_contains(stack: &[String], target: &str) -> bool {
    stack.iter().any(|value| value == target)
}

fn procspec_metadata_tag(tag: &str) -> bool {
    matches!(
        tag,
        "integrationTime"
            | "boxcarWidth"
            | "scansToAverage"
            | "correctForElectricalDark"
            | "correctForNonLinearity"
            | "serialNumber"
            | "spectrometerSerialNumber"
            | "spectrometerClass"
            | "spectrometerNumberOfPixels"
            | "timestamp"
            | "userName"
    )
}

fn push_procspec_signal(
    signals: &mut BTreeMap<String, SpectralArray>,
    axis: &[f64],
    name: &str,
    values: Vec<f64>,
    signal_type: SignalType,
    unit: Option<String>,
) -> Result<()> {
    if values.is_empty() {
        return Ok(());
    }
    if values.len() != axis.len() {
        return Err(Error::InvalidRecord(format!(
            "Ocean Optics ProcSpec {name} length {} does not match wavelength length {}",
            values.len(),
            axis.len()
        )));
    }
    let signal = SpectralArray::new(
        SpectralAxis::new(axis.to_vec(), "nm", AxisKind::Wavelength)?,
        values,
        vec!["x".to_string()],
        signal_type,
        unit,
        name,
        "file",
    )?;
    signals.insert(name.to_string(), signal);
    Ok(())
}

fn verify_procspec_signature(
    signature_xml: &str,
    xml_name: &str,
    xml_bytes: &[u8],
    warnings: &mut Vec<String>,
) -> String {
    let file_name = xml_tag_text(signature_xml, "fileName");
    let hash_algorithm = xml_tag_text(signature_xml, "hashAlgorithm");
    let hash_value = xml_tag_text(signature_xml, "hashValue");
    if file_name.as_deref() != Some(xml_name) {
        warnings.push(format!(
            "ocean_optics_procspec_signature_file_mismatch: expected {xml_name}, found {:?}",
            file_name
        ));
        return "mismatch".to_string();
    }
    if hash_algorithm.as_deref() != Some("SHA-512") {
        warnings.push(format!(
            "ocean_optics_procspec_signature_algorithm_unsupported: {:?}",
            hash_algorithm
        ));
        return "unsupported".to_string();
    }
    let Some(expected) = hash_value.map(|value| value.split_whitespace().collect::<String>())
    else {
        warnings.push("ocean_optics_procspec_signature_missing_hash".to_string());
        return "missing_hash".to_string();
    };
    let actual = format!("{:x}", Sha512::digest(xml_bytes));
    if actual.eq_ignore_ascii_case(&expected) {
        "verified".to_string()
    } else {
        warnings.push("ocean_optics_procspec_signature_hash_mismatch".to_string());
        "mismatch".to_string()
    }
}

fn xml_tag_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim().to_string())
}

fn parse_ocean_text(text: &str, path: &Path) -> Result<ParsedOceanText> {
    let normalized = text.replace('\r', "\n");
    let mut metadata_pairs = Vec::new();
    if let Some(file_name) = path.file_name().and_then(|value| value.to_str()) {
        metadata_pairs.push(("file_name".to_string(), file_name.to_string()));
    }
    let mut rows = Vec::new();
    let mut column_labels = Vec::new();
    let mut in_data = false;
    let mut saw_begin_marker = false;

    for raw_line in normalized.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line == "++++++++++++++++++++++++++++++++++++" {
            continue;
        }
        if line.contains(">>>>>Begin") && line.contains("Data<<<<<") {
            in_data = true;
            saw_begin_marker = true;
            continue;
        }
        if in_data {
            if let Some(numbers) = parse_numeric_row(line) {
                rows.push(numbers);
            } else if rows.is_empty() && column_labels.is_empty() {
                column_labels = split_fields(line);
            }
            continue;
        }
        if let Some(numbers) = parse_numeric_row(line) {
            in_data = true;
            rows.push(numbers);
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            metadata_pairs.push((key.to_string(), value.trim().to_string()));
        } else {
            metadata_pairs.push(("header".to_string(), line.to_string()));
        }
    }

    if rows.is_empty() {
        return Err(Error::InvalidRecord(
            "Ocean Optics text export contains no numeric rows".to_string(),
        ));
    }
    let width = rows[0].len();
    rows.retain(|row| row.len() == width);
    if width < 2 {
        return Err(Error::InvalidRecord(
            "Ocean Optics text export needs at least x and y columns".to_string(),
        ));
    }
    if column_labels.len() != width {
        column_labels = default_column_labels(width);
    }
    let format = if metadata_pairs
        .iter()
        .any(|(_, value)| value.contains("CRAIC"))
        || metadata_pairs
            .iter()
            .any(|(_, value)| value.eq_ignore_ascii_case("Reflectance"))
    {
        "ocean-optics-craic-text"
    } else if saw_begin_marker {
        "ocean-optics-text"
    } else {
        "ocean-optics-two-column-csv"
    };

    Ok(ParsedOceanText {
        format,
        metadata_pairs,
        column_labels,
        rows,
        warnings: Vec::new(),
    })
}

fn signals_from_columns(parsed: &ParsedOceanText) -> Result<BTreeMap<String, SpectralArray>> {
    let axis: Vec<f64> = parsed.rows.iter().map(|row| row[0]).collect();
    let mut signals = BTreeMap::new();
    for column_index in 1..parsed.column_labels.len() {
        let label = &parsed.column_labels[column_index];
        let values = parsed
            .rows
            .iter()
            .map(|row| row[column_index])
            .collect::<Vec<_>>();
        let (name, signal_type, unit) = signal_mapping(label, parsed);
        let axis_obj = SpectralAxis::new(axis.clone(), "nm", AxisKind::Wavelength)?;
        let signal = SpectralArray::new(
            axis_obj,
            values,
            vec!["x".to_string()],
            signal_type,
            unit,
            &name,
            "file",
        )?;
        signals.insert(name, signal);
    }
    Ok(signals)
}

fn signal_mapping(label: &str, parsed: &ParsedOceanText) -> (String, SignalType, Option<String>) {
    let lower = label.to_ascii_lowercase();
    if lower == "d" || lower.contains("dark") {
        return ("dark_reference".to_string(), SignalType::RawCounts, None);
    }
    if lower == "r" || lower.contains("reference") {
        return ("white_reference".to_string(), SignalType::RawCounts, None);
    }
    if lower == "s" || lower.contains("sample") {
        return ("sample".to_string(), SignalType::RawCounts, None);
    }

    let header_text = parsed
        .metadata_pairs
        .iter()
        .map(|(_, value)| value.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    if lower == "p" || lower.contains("processed") || lower == "y" {
        if header_text.contains("absolute irradiance") {
            return ("irradiance".to_string(), SignalType::Irradiance, None);
        }
        if header_text.contains("transmission") || header_text.contains("transmittance") {
            return (
                "transmittance".to_string(),
                SignalType::Transmittance,
                Some("%".to_string()),
            );
        }
        if header_text.contains("reflectance") {
            return (
                "reflectance".to_string(),
                SignalType::Reflectance,
                Some("%".to_string()),
            );
        }
        return ("processed".to_string(), SignalType::Unknown, None);
    }

    (safe_signal_name(label, "signal"), SignalType::Unknown, None)
}

fn dominant_signal_type(signals: &BTreeMap<String, SpectralArray>) -> SignalType {
    for preferred in [
        SignalType::Absorbance,
        SignalType::Reflectance,
        SignalType::Transmittance,
        SignalType::Irradiance,
    ] {
        if signals
            .values()
            .any(|signal| signal.signal_type == preferred)
        {
            return preferred;
        }
    }
    signals
        .values()
        .next()
        .map(|signal| signal.signal_type.clone())
        .unwrap_or(SignalType::Unknown)
}

fn default_column_labels(width: usize) -> Vec<String> {
    if width == 2 {
        vec!["W".to_string(), "P".to_string()]
    } else {
        (0..width)
            .map(|index| {
                if index == 0 {
                    "W".to_string()
                } else {
                    format!("signal_{index}")
                }
            })
            .collect()
    }
}

fn is_numeric_pair_line(line: &str) -> bool {
    parse_numeric_row(line)
        .map(|values| values.len() >= 2)
        .unwrap_or(false)
}

fn parse_numeric_row(line: &str) -> Option<Vec<f64>> {
    let values = split_fields(line)
        .iter()
        .map(|field| parse_number(field))
        .collect::<Option<Vec<_>>>()?;
    (values.len() >= 2).then_some(values)
}

fn split_fields(line: &str) -> Vec<String> {
    if line.contains('\t') {
        line.split('\t')
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect()
    } else if line.contains(',') {
        line.split(',')
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect()
    } else {
        line.split_whitespace().map(ToString::to_string).collect()
    }
}
