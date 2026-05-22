use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use hdf5_reader::Hdf5File;
use nirs4all_io_core::{Confidence, Error, FormatProbe, Result, SourceFile, SpectralRecord};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader as XmlReader;
use serde_json::{json, Value};

use crate::readers::hdf5::read_hdf5_records;
use crate::readers::util::{normalize_key, read_bytes, text_lossy_from_bytes};
use crate::Reader;

pub struct FgiXmlReader;

impl Reader for FgiXmlReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::fgi_xml"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext != "xml" {
            return None;
        }
        let text = String::from_utf8_lossy(head);
        (text.contains("<FGIMeasurement") && text.contains("<DataReference")).then(|| {
            FormatProbe::new(
                "fgi-hdf5-xml",
                self.name(),
                Confidence::Definite,
                "FGI XML sidecar referencing an HDF5 spectral payload",
            )
        })
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let bytes = read_bytes(path)?;
        let (text, mut xml_source) = text_lossy_from_bytes(path, &bytes);
        xml_source.role = "metadata_sidecar".to_string();
        let parsed = parse_fgi_xml(&text)?;
        let hdf5_path = resolve_data_reference(path, &parsed.data_reference);
        let file = Hdf5File::open(&hdf5_path)
            .map_err(|error| Error::InvalidRecord(format!("FGI HDF5 open error: {error}")))?;
        let hdf5_source = SourceFile::from_path(&hdf5_path, "primary")?;
        let mut records = read_hdf5_records(&file, hdf5_source, self.name())?;
        for record in &mut records {
            record.provenance.format = "fgi-hdf5-xml".to_string();
            record.provenance.reader = self.name().to_string();
            record.provenance.sources.push(xml_source.clone());
            record
                .metadata
                .insert("fgi_xml".to_string(), json!(parsed.metadata));
            record.metadata.insert(
                "fgi_data_reference".to_string(),
                json!(parsed.data_reference),
            );
            record.validate()?;
        }
        Ok(records)
    }
}

#[derive(Default)]
struct ParsedFgiXml {
    data_reference: String,
    metadata: BTreeMap<String, Value>,
}

fn parse_fgi_xml(text: &str) -> Result<ParsedFgiXml> {
    let mut reader = XmlReader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut parsed = ParsedFgiXml::default();
    let mut stack = Vec::<String>::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "DataReference" {
                    if let Some(path) = attr_value(&event, "path") {
                        parsed.data_reference = path;
                    }
                }
                stack.push(tag);
            }
            Ok(Event::Empty(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "DataReference" {
                    if let Some(path) = attr_value(&event, "path") {
                        parsed.data_reference = path;
                    }
                }
            }
            Ok(Event::Text(event)) => {
                let Some(field) = stack.last() else {
                    continue;
                };
                if stack.iter().any(|tag| tag == "Metadata") && field != "Metadata" {
                    let value = event.decode().map_err(|error| {
                        Error::InvalidRecord(format!("FGI XML text error: {error}"))
                    })?;
                    parsed
                        .metadata
                        .insert(normalize_key(field), json!(value.trim()));
                }
            }
            Ok(Event::End(_event)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(Error::InvalidRecord(format!("FGI XML error: {error}"))),
            _ => {}
        }
    }

    if parsed.data_reference.trim().is_empty() {
        return Err(Error::InvalidRecord(
            "FGI XML sidecar has no DataReference path".to_string(),
        ));
    }
    Ok(parsed)
}

fn resolve_data_reference(xml_path: &Path, reference: &str) -> PathBuf {
    let referenced = PathBuf::from(reference);
    if referenced.is_absolute() {
        return referenced;
    }
    xml_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(referenced)
}

fn attr_value(event: &BytesStart<'_>, name: &str) -> Option<String> {
    event.attributes().flatten().find_map(|attr| {
        (local_name(attr.key.as_ref()) == name)
            .then(|| String::from_utf8_lossy(attr.value.as_ref()).to_string())
    })
}

fn local_name(name: &[u8]) -> String {
    let text = String::from_utf8_lossy(name);
    text.rsplit(':').next().unwrap_or(&text).to_string()
}
