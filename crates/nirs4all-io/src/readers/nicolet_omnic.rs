use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralRecord,
};
use serde_json::json;

use crate::readers::util::{safe_signal_name, single_signal_record, SingleSignalSpec};
use crate::Reader;

const OMNIC_DATA_MAGIC: &[u8] = b"Spectral Data File";
const OMNIC_SERIES_MAGIC: &[u8] = b"Spectral Exte File";
const KEY_TABLE_COUNT_OFFSET: usize = 294;
const KEY_TABLE_OFFSET: usize = 304;
const KEY_TABLE_ENTRY_LEN: usize = 16;

pub struct NicoletOmnicReader;

impl Reader for NicoletOmnicReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::nicolet_omnic"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if head.starts_with(OMNIC_DATA_MAGIC) && matches!(ext.as_str(), "spa" | "spg") {
            return Some(FormatProbe::new(
                "nicolet-omnic",
                self.name(),
                Confidence::Definite,
                "Thermo Nicolet OMNIC spectral data file header",
            ));
        }
        if head.starts_with(OMNIC_DATA_MAGIC) {
            return Some(FormatProbe::new(
                "nicolet-omnic",
                self.name(),
                Confidence::Likely,
                "Thermo Nicolet OMNIC spectral data file header with non-standard extension",
            ));
        }
        if head.starts_with(OMNIC_SERIES_MAGIC) {
            return Some(FormatProbe::new(
                "nicolet-omnic-srs",
                self.name(),
                Confidence::Possible,
                "Thermo Nicolet OMNIC series file header; series decoding is pending",
            ));
        }
        None
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let source = SourceFile::from_bytes(path, &bytes, "primary");
        if bytes.starts_with(OMNIC_SERIES_MAGIC) {
            return Err(Error::InvalidRecord(
                "Nicolet OMNIC .srs series are recognized but not implemented yet".to_string(),
            ));
        }
        if !bytes.starts_with(OMNIC_DATA_MAGIC) {
            return Err(Error::InvalidRecord(
                "missing Nicolet OMNIC spectral data header".to_string(),
            ));
        }
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let entries = parse_key_table(&bytes)?;
        if ext == "spg" || count_key(&entries, OmnicKey::Header) > 1 {
            read_spg(&bytes, source, self.name())
        } else {
            read_spa(&bytes, source, self.name())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OmnicKey {
    Header,
    Intensities,
    Title,
    History,
    Other(u8),
}

impl OmnicKey {
    fn from_u8(value: u8) -> Self {
        match value {
            2 => Self::Header,
            3 => Self::Intensities,
            27 => Self::History,
            107 => Self::Title,
            other => Self::Other(other),
        }
    }
}

#[derive(Clone, Debug)]
struct KeyEntry {
    offset: usize,
    key: OmnicKey,
    payload_offset: usize,
    payload_len: usize,
}

#[derive(Clone, Debug)]
struct OmnicHeader {
    nx: usize,
    axis_kind: AxisKind,
    axis_unit: String,
    axis_title: String,
    signal_type: SignalType,
    signal_unit: Option<String>,
    signal_title: String,
    first_x: f64,
    last_x: f64,
    scan_points: u32,
    zero_path_difference: u32,
    scan_count: u32,
    background_scan_count: u32,
    collection_length_ticks: u32,
    reference_frequency: f32,
    optical_velocity: f32,
}

#[derive(Clone, Debug)]
struct SpectrumDescriptor {
    header_entry: KeyEntry,
    data_entry: KeyEntry,
    title_entry: Option<KeyEntry>,
    record_index: usize,
    spectrum_title: Option<String>,
    timestamp: Option<u32>,
}

fn read_spa(bytes: &[u8], source: SourceFile, reader: &str) -> Result<Vec<SpectralRecord>> {
    let entries = parse_key_table(bytes)?;
    let header_entry = first_entry(&entries, OmnicKey::Header)?;
    let data_entry = first_entry(&entries, OmnicKey::Intensities)?;
    let record = build_record(
        bytes,
        source,
        reader,
        "nicolet-omnic-spa",
        SpectrumDescriptor {
            header_entry,
            data_entry,
            title_entry: None,
            record_index: 0,
            spectrum_title: Some(read_fixed_text(bytes, 30, 256)),
            timestamp: read_u32(bytes, 296).ok(),
        },
    )?;
    Ok(vec![record])
}

fn read_spg(bytes: &[u8], source: SourceFile, reader: &str) -> Result<Vec<SpectralRecord>> {
    let entries = parse_key_table(bytes)?;
    let headers = entries_for_key(&entries, OmnicKey::Header);
    let data = entries_for_key(&entries, OmnicKey::Intensities);
    let titles = entries_for_key(&entries, OmnicKey::Title);
    if headers.is_empty() || data.is_empty() {
        return Err(Error::InvalidRecord(
            "Nicolet OMNIC group contains no spectral header/data entries".to_string(),
        ));
    }
    if headers.len() != data.len() {
        return Err(Error::InvalidRecord(format!(
            "Nicolet OMNIC group has {} headers but {} data blocks",
            headers.len(),
            data.len()
        )));
    }

    let mut records = Vec::with_capacity(headers.len());
    for (index, (header_entry, data_entry)) in headers.into_iter().zip(data).enumerate() {
        let title_entry = titles.get(index).cloned();
        let (title, timestamp) = title_entry
            .as_ref()
            .map(|entry| {
                (
                    read_fixed_text(bytes, entry.payload_offset, 256),
                    read_u32(bytes, entry.payload_offset + 256).ok(),
                )
            })
            .unwrap_or_else(|| (read_fixed_text(bytes, 30, 256), None));
        records.push(build_record(
            bytes,
            source.clone(),
            reader,
            "nicolet-omnic-spg",
            SpectrumDescriptor {
                header_entry,
                data_entry,
                title_entry,
                record_index: index,
                spectrum_title: Some(title),
                timestamp,
            },
        )?);
    }
    Ok(records)
}

fn build_record(
    bytes: &[u8],
    source: SourceFile,
    reader: &str,
    format: &str,
    descriptor: SpectrumDescriptor,
) -> Result<SpectralRecord> {
    let header = read_header(bytes, descriptor.header_entry.payload_offset)?;
    let values = read_float_block(bytes, &descriptor.data_entry, header.nx)?;
    let axis = linspace(header.first_x, header.last_x, header.nx);
    let signal_name = safe_signal_name(&header.signal_title, "signal");

    let mut metadata = BTreeMap::new();
    metadata.insert("record_index".to_string(), json!(descriptor.record_index));
    metadata.insert(
        "omnic_header_offset".to_string(),
        json!(descriptor.header_entry.payload_offset),
    );
    metadata.insert(
        "omnic_header_key_offset".to_string(),
        json!(descriptor.header_entry.offset),
    );
    metadata.insert(
        "omnic_data_offset".to_string(),
        json!(descriptor.data_entry.payload_offset),
    );
    metadata.insert(
        "omnic_data_key_offset".to_string(),
        json!(descriptor.data_entry.offset),
    );
    metadata.insert("axis_title".to_string(), json!(header.axis_title));
    metadata.insert("signal_title".to_string(), json!(header.signal_title));
    metadata.insert("scan_points".to_string(), json!(header.scan_points));
    metadata.insert(
        "zero_path_difference".to_string(),
        json!(header.zero_path_difference),
    );
    metadata.insert("scan_count".to_string(), json!(header.scan_count));
    metadata.insert(
        "background_scan_count".to_string(),
        json!(header.background_scan_count),
    );
    metadata.insert(
        "collection_length_ticks_10ms".to_string(),
        json!(header.collection_length_ticks),
    );
    metadata.insert(
        "reference_frequency_cm-1".to_string(),
        json!(header.reference_frequency),
    );
    metadata.insert(
        "optical_velocity".to_string(),
        json!(header.optical_velocity),
    );
    if let Some(title) = descriptor
        .spectrum_title
        .filter(|value| !value.trim().is_empty())
    {
        metadata.insert("spectrum_title".to_string(), json!(title));
    }
    if let Some(timestamp) = descriptor.timestamp {
        metadata.insert("omnic_timestamp_seconds".to_string(), json!(timestamp));
    }
    if let Some(title_entry) = descriptor.title_entry {
        metadata.insert(
            "omnic_title_offset".to_string(),
            json!(title_entry.payload_offset),
        );
    }

    single_signal_record(
        format,
        reader,
        source,
        SingleSignalSpec {
            axis_values: axis,
            axis_unit: header.axis_unit,
            axis_kind: header.axis_kind,
            values,
            signal_name,
            signal_type: header.signal_type,
            signal_unit: header.signal_unit,
            role: safe_signal_name(&header.signal_title, "signal"),
        },
        BTreeMap::new(),
        metadata,
        vec!["nicolet_omnic_reverse_engineered_key_table".to_string()],
    )
}

fn parse_key_table(bytes: &[u8]) -> Result<Vec<KeyEntry>> {
    let nlines = read_u16(bytes, KEY_TABLE_COUNT_OFFSET)? as usize;
    if nlines == 0 {
        return Err(Error::InvalidRecord(
            "Nicolet OMNIC key table is empty".to_string(),
        ));
    }
    let mut entries = Vec::with_capacity(nlines);
    for index in 0..nlines {
        let offset = KEY_TABLE_OFFSET + index * KEY_TABLE_ENTRY_LEN;
        if offset + 10 > bytes.len() {
            return Err(Error::InvalidRecord(
                "Nicolet OMNIC key table extends past end of file".to_string(),
            ));
        }
        let raw_key = bytes[offset];
        entries.push(KeyEntry {
            offset,
            key: OmnicKey::from_u8(raw_key),
            payload_offset: read_u32(bytes, offset + 2)? as usize,
            payload_len: read_u32(bytes, offset + 6)? as usize,
        });
    }
    Ok(entries)
}

fn read_header(bytes: &[u8], offset: usize) -> Result<OmnicHeader> {
    let nx = read_u32(bytes, offset + 4)? as usize;
    if nx == 0 {
        return Err(Error::InvalidRecord(
            "Nicolet OMNIC spectrum has zero points".to_string(),
        ));
    }
    let (axis_kind, axis_unit, axis_title) =
        axis_from_key(*bytes.get(offset + 8).ok_or_else(|| {
            Error::InvalidRecord("Nicolet OMNIC header is truncated".to_string())
        })?);
    let (signal_type, signal_unit, signal_title) =
        signal_from_key(*bytes.get(offset + 12).ok_or_else(|| {
            Error::InvalidRecord("Nicolet OMNIC header is truncated".to_string())
        })?);
    Ok(OmnicHeader {
        nx,
        axis_kind,
        axis_unit,
        axis_title,
        signal_type,
        signal_unit,
        signal_title,
        first_x: read_f32(bytes, offset + 16)? as f64,
        last_x: read_f32(bytes, offset + 20)? as f64,
        scan_points: read_u32(bytes, offset + 28)?,
        zero_path_difference: read_u32(bytes, offset + 32)?,
        scan_count: read_u32(bytes, offset + 36)?,
        background_scan_count: read_u32(bytes, offset + 52)?,
        collection_length_ticks: read_u32(bytes, offset + 68)?,
        reference_frequency: read_f32(bytes, offset + 80)?,
        optical_velocity: read_f32(bytes, offset + 188)?,
    })
}

fn read_float_block(bytes: &[u8], entry: &KeyEntry, expected_len: usize) -> Result<Vec<f64>> {
    let available_len = entry.payload_len / 4;
    if available_len < expected_len {
        return Err(Error::InvalidRecord(format!(
            "Nicolet OMNIC data block at {} has {} float32 values, expected {}",
            entry.payload_offset, available_len, expected_len
        )));
    }
    let end = entry.payload_offset + expected_len * 4;
    if end > bytes.len() {
        return Err(Error::InvalidRecord(
            "Nicolet OMNIC data block extends past end of file".to_string(),
        ));
    }
    let values = (0..expected_len)
        .map(|index| {
            let start = entry.payload_offset + index * 4;
            f32::from_le_bytes(bytes[start..start + 4].try_into().expect("slice len")) as f64
        })
        .collect::<Vec<_>>();
    Ok(values)
}

fn first_entry(entries: &[KeyEntry], key: OmnicKey) -> Result<KeyEntry> {
    entries
        .iter()
        .find(|entry| entry.key == key)
        .cloned()
        .ok_or_else(|| Error::InvalidRecord(format!("Nicolet OMNIC missing {key:?} key entry")))
}

fn entries_for_key(entries: &[KeyEntry], key: OmnicKey) -> Vec<KeyEntry> {
    entries
        .iter()
        .filter(|entry| entry.key == key)
        .cloned()
        .collect()
}

fn count_key(entries: &[KeyEntry], key: OmnicKey) -> usize {
    entries.iter().filter(|entry| entry.key == key).count()
}

fn axis_from_key(key: u8) -> (AxisKind, String, String) {
    match key {
        1 => (
            AxisKind::Wavenumber,
            "cm-1".to_string(),
            "wavenumbers".to_string(),
        ),
        2 => (
            AxisKind::Index,
            "index".to_string(),
            "data points".to_string(),
        ),
        3 => (
            AxisKind::Wavelength,
            "nm".to_string(),
            "wavelengths".to_string(),
        ),
        4 => (
            AxisKind::Wavelength,
            "um".to_string(),
            "wavelengths".to_string(),
        ),
        32 => (
            AxisKind::Wavenumber,
            "cm-1".to_string(),
            "raman shift".to_string(),
        ),
        _ => (AxisKind::Index, "index".to_string(), "xaxis".to_string()),
    }
}

fn signal_from_key(key: u8) -> (SignalType, Option<String>, String) {
    match key {
        17 => (SignalType::Absorbance, None, "absorbance".to_string()),
        16 => (
            SignalType::Transmittance,
            Some("%".to_string()),
            "transmittance".to_string(),
        ),
        11 => (
            SignalType::Reflectance,
            Some("%".to_string()),
            "reflectance".to_string(),
        ),
        12 => (SignalType::Preprocessed, None, "log(1/R)".to_string()),
        20 => (
            SignalType::KubelkaMunk,
            Some("Kubelka_Munk".to_string()),
            "Kubelka-Munk".to_string(),
        ),
        21 => (SignalType::Reflectance, None, "reflectance".to_string()),
        22 => (
            SignalType::Interferogram,
            Some("V".to_string()),
            "detector signal".to_string(),
        ),
        26 => (SignalType::Unknown, None, "photoacoustic".to_string()),
        31 => (SignalType::Unknown, None, "Raman intensity".to_string()),
        _ => (SignalType::Unknown, None, "intensity".to_string()),
    }
}

fn linspace(first: f64, last: f64, len: usize) -> Vec<f64> {
    if len <= 1 {
        return vec![first];
    }
    let step = (last - first) / (len - 1) as f64;
    (0..len).map(|index| first + step * index as f64).collect()
}

fn read_fixed_text(bytes: &[u8], offset: usize, len: usize) -> String {
    if offset >= bytes.len() {
        return String::new();
    }
    let end = (offset + len).min(bytes.len());
    let raw = &bytes[offset..end];
    let text_bytes = raw.split(|byte| *byte == 0).next().unwrap_or_default();
    decode_text(text_bytes).trim().to_string()
}

fn decode_text(bytes: &[u8]) -> String {
    std::str::from_utf8(bytes)
        .map(|value| value.to_string())
        .unwrap_or_else(|_| bytes.iter().map(|byte| *byte as char).collect())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| Error::InvalidRecord("truncated Nicolet OMNIC u16 field".to_string()))?;
    Ok(u16::from_le_bytes(value.try_into().expect("slice len")))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| Error::InvalidRecord("truncated Nicolet OMNIC u32 field".to_string()))?;
    Ok(u32::from_le_bytes(value.try_into().expect("slice len")))
}

fn read_f32(bytes: &[u8], offset: usize) -> Result<f32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| Error::InvalidRecord("truncated Nicolet OMNIC f32 field".to_string()))?;
    Ok(f32::from_le_bytes(value.try_into().expect("slice len")))
}
