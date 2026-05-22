use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralArray,
    SpectralAxis, SpectralRecord,
};
use serde_json::json;

use crate::readers::util::{provenance, safe_signal_name, single_signal_record, SingleSignalSpec};
use crate::Reader;

const OMNIC_DATA_MAGIC: &[u8] = b"Spectral Data File";
const OMNIC_SERIES_MAGIC: &[u8] = b"Spectral Exte File";
const KEY_TABLE_COUNT_OFFSET: usize = 294;
const KEY_TABLE_OFFSET: usize = 304;
const KEY_TABLE_ENTRY_LEN: usize = 16;
const SRS_TG_SIGNATURE: &[u8] = b"\x02\x00\x00\x00\x18\x00\x00\x00\x00\x00";

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
                "Thermo Nicolet OMNIC series file header; TGA/GC series layout is decoded on read",
            ));
        }
        None
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(&self, path: &Path, bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
        let source = SourceFile::from_bytes(path, bytes, "primary");
        if bytes.starts_with(OMNIC_SERIES_MAGIC) {
            return read_srs(bytes, source, self.name());
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
        let entries = parse_key_table(bytes)?;
        if ext == "spg" || count_key(&entries, OmnicKey::Header) > 1 {
            read_spg(bytes, source, self.name())
        } else {
            read_spa(bytes, source, self.name())
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

#[derive(Clone, Debug)]
struct SrsLayout {
    data_header_offset: usize,
    background_header_offset: usize,
    data_offset: usize,
}

#[derive(Clone, Debug)]
enum SrsVariant {
    TgGc(SrsLayout),
    Unsupported { signature_count: usize },
}

#[derive(Clone, Debug)]
struct SrsHeader {
    base: OmnicHeader,
    name: String,
    collection_length_seconds: f32,
    first_y: f32,
    last_y: f32,
    ny: usize,
}

struct SrsSpectra {
    labels: Vec<String>,
    values: Vec<f64>,
}

fn read_srs(bytes: &[u8], source: SourceFile, reader: &str) -> Result<Vec<SpectralRecord>> {
    match detect_srs_variant(bytes)? {
        SrsVariant::TgGc(layout) => read_srs_tg_gc(bytes, source, reader, layout),
        SrsVariant::Unsupported { signature_count } => Err(Error::InvalidRecord(format!(
            "Nicolet OMNIC .srs/.srsx series variant is not supported yet: found {signature_count} TGA/GC anchors, expected 3. Supported series layout is TGA/GC; rapid-scan, high-speed and .srsx variants require a real fixture plus a reference export before decoding."
        ))),
    }
}

fn read_srs_tg_gc(
    bytes: &[u8],
    source: SourceFile,
    reader: &str,
    layout: SrsLayout,
) -> Result<Vec<SpectralRecord>> {
    let header = read_srs_header(bytes, layout.data_header_offset)?;
    let spectra = read_srs_spectra(bytes, layout.data_offset, header.ny, header.base.nx)?;
    let axis = SpectralAxis::new(
        linspace(header.base.first_x, header.base.last_x, header.base.nx),
        header.base.axis_unit.clone(),
        header.base.axis_kind.clone(),
    )?;
    let signal = SpectralArray::new(
        axis,
        spectra.values,
        vec!["y".to_string(), "x".to_string()],
        header.base.signal_type.clone(),
        header.base.signal_unit.clone(),
        safe_signal_name(&header.base.signal_title, "signal"),
        "file",
    )?;
    let signal_name = safe_signal_name(&header.base.signal_title, "signal");
    let mut signals = BTreeMap::new();
    signals.insert(signal_name, signal);

    let series_variant = srs_series_variant(&header);
    let mut metadata = BTreeMap::new();
    metadata.insert("series_variant".to_string(), json!(series_variant));
    metadata.insert("series_name".to_string(), json!(header.name));
    metadata.insert("series_y_len".to_string(), json!(header.ny));
    metadata.insert("series_y_first_min".to_string(), json!(header.first_y));
    metadata.insert("series_y_last_min".to_string(), json!(header.last_y));
    if header.ny > 1 {
        metadata.insert(
            "series_y_step_min".to_string(),
            json!((header.last_y - header.first_y) / (header.ny - 1) as f32),
        );
    }
    metadata.insert(
        "collection_length_seconds".to_string(),
        json!(header.collection_length_seconds),
    );
    metadata.insert(
        "omnic_srs_data_header_offset".to_string(),
        json!(layout.data_header_offset),
    );
    metadata.insert(
        "omnic_srs_background_header_offset".to_string(),
        json!(layout.background_header_offset),
    );
    metadata.insert(
        "omnic_srs_data_offset".to_string(),
        json!(layout.data_offset),
    );
    metadata.insert("axis_title".to_string(), json!(header.base.axis_title));
    metadata.insert("signal_title".to_string(), json!(header.base.signal_title));
    metadata.insert("scan_points".to_string(), json!(header.base.scan_points));
    metadata.insert(
        "zero_path_difference".to_string(),
        json!(header.base.zero_path_difference),
    );
    metadata.insert("scan_count".to_string(), json!(header.base.scan_count));
    metadata.insert(
        "background_scan_count".to_string(),
        json!(header.base.background_scan_count),
    );
    metadata.insert(
        "reference_frequency_cm-1".to_string(),
        json!(header.base.reference_frequency),
    );
    metadata.insert(
        "optical_velocity".to_string(),
        json!(header.base.optical_velocity),
    );
    if let Some(first) = spectra.labels.first().filter(|value| !value.is_empty()) {
        metadata.insert("first_spectrum_label".to_string(), json!(first));
    }
    if let Some(last) = spectra.labels.last().filter(|value| !value.is_empty()) {
        metadata.insert("last_spectrum_label".to_string(), json!(last));
    }

    let record = SpectralRecord {
        signals,
        signal_type: header.base.signal_type,
        targets: BTreeMap::new(),
        metadata,
        provenance: provenance(
            "nicolet-omnic-srs",
            reader,
            source,
            vec![srs_warning(series_variant).to_string()],
        ),
        quality_flags: Vec::new(),
    };
    record.validate()?;
    Ok(vec![record])
}

fn srs_series_variant(header: &SrsHeader) -> &'static str {
    if header.base.signal_type == SignalType::Interferogram
        || header.base.axis_kind == AxisKind::Index
    {
        "rapid_scan_raw"
    } else if header.base.zero_path_difference <= 128
        && header.base.background_scan_count == 0
        && header.base.scan_count >= 16
    {
        "rapid_scan_reprocessed"
    } else {
        "tg_gc"
    }
}

fn srs_warning(series_variant: &str) -> &'static str {
    match series_variant {
        "rapid_scan_raw" | "rapid_scan_reprocessed" => {
            "nicolet_omnic_srs_rapid_scan_reverse_engineered"
        }
        _ => "nicolet_omnic_srs_tg_gc_reverse_engineered",
    }
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

fn detect_srs_variant(bytes: &[u8]) -> Result<SrsVariant> {
    let occurrences = find_all(bytes, SRS_TG_SIGNATURE);
    if occurrences.len() != 3 {
        return Ok(SrsVariant::Unsupported {
            signature_count: occurrences.len(),
        });
    }
    let data_header_offset = occurrences[0].checked_sub(152).ok_or_else(|| {
        Error::InvalidRecord("Nicolet OMNIC .srs data header offset underflow".to_string())
    })?;
    let background_header_offset = occurrences[1].checked_sub(152).ok_or_else(|| {
        Error::InvalidRecord("Nicolet OMNIC .srs background header offset underflow".to_string())
    })?;
    let data_offset = occurrences[2].checked_add(60).ok_or_else(|| {
        Error::InvalidRecord("Nicolet OMNIC .srs data offset overflow".to_string())
    })?;
    Ok(SrsVariant::TgGc(SrsLayout {
        data_header_offset,
        background_header_offset,
        data_offset,
    }))
}

fn read_srs_header(bytes: &[u8], offset: usize) -> Result<SrsHeader> {
    let base = read_header(bytes, offset)?;
    let ny = read_u32(bytes, offset + 1026)? as usize;
    if ny == 0 {
        return Err(Error::InvalidRecord(
            "Nicolet OMNIC .srs series has zero y points".to_string(),
        ));
    }
    Ok(SrsHeader {
        base,
        name: read_fixed_text(bytes, offset + 938, 256),
        collection_length_seconds: read_f32(bytes, offset + 1002)? * 60.0,
        first_y: read_f32(bytes, offset + 1010)?,
        last_y: read_f32(bytes, offset + 1006)?,
        ny,
    })
}

fn read_srs_spectra(bytes: &[u8], data_offset: usize, ny: usize, nx: usize) -> Result<SrsSpectra> {
    let value_count = nx.checked_mul(ny).ok_or_else(|| {
        Error::InvalidRecord("Nicolet OMNIC .srs matrix dimensions overflow".to_string())
    })?;
    let mut labels = Vec::with_capacity(ny);
    let mut values = Vec::with_capacity(value_count);
    let mut cursor = data_offset;
    for index in 0..ny {
        if index > 0 {
            cursor = cursor.checked_add(16).ok_or_else(|| {
                Error::InvalidRecord("Nicolet OMNIC .srs cursor offset overflow".to_string())
            })?;
        }
        let label_end = cursor.checked_add(84).ok_or_else(|| {
            Error::InvalidRecord("Nicolet OMNIC .srs label offset overflow".to_string())
        })?;
        if label_end > bytes.len() {
            return Err(Error::InvalidRecord(
                "Nicolet OMNIC .srs spectrum label extends past end of file".to_string(),
            ));
        }
        labels.push(read_fixed_text(bytes, cursor, 84));
        let values_offset = label_end;
        let byte_len = nx.checked_mul(4).ok_or_else(|| {
            Error::InvalidRecord("Nicolet OMNIC .srs row byte length overflow".to_string())
        })?;
        let values_end = values_offset.checked_add(byte_len).ok_or_else(|| {
            Error::InvalidRecord("Nicolet OMNIC .srs row offset overflow".to_string())
        })?;
        if values_end > bytes.len() {
            return Err(Error::InvalidRecord(
                "Nicolet OMNIC .srs spectrum data extends past end of file".to_string(),
            ));
        }
        for point in 0..nx {
            values.push(read_f32(bytes, values_offset + point * 4)? as f64);
        }
        cursor = values_end;
    }
    Ok(SrsSpectra { labels, values })
}

fn find_all(bytes: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() || bytes.len() < needle.len() {
        return Vec::new();
    }
    let mut positions = Vec::new();
    let mut start = 0;
    while start + needle.len() <= bytes.len() {
        let Some(relative) = bytes[start..]
            .windows(needle.len())
            .position(|window| window == needle)
        else {
            break;
        };
        let absolute = start + relative;
        positions.push(absolute);
        start = absolute + 1;
    }
    positions
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn detects_gc_demo_srs_tg_gc_layout() {
        let bytes = fixture_bytes("samples/nicolet_omnic/GC_Demo.srs");
        let layout = tg_gc_layout(&bytes);

        assert_eq!(layout.data_header_offset, 5_584);
        assert_eq!(layout.background_header_offset, 7_044);
        assert_eq!(layout.data_offset, 20_616);
    }

    #[test]
    fn detects_tgair_srs_tg_gc_layout() {
        let bytes = fixture_bytes("samples/nicolet_omnic/TGAIR.srs");
        let layout = tg_gc_layout(&bytes);

        assert_eq!(layout.data_header_offset, 14_032);
        assert_eq!(layout.background_header_offset, 20_836);
        assert_eq!(layout.data_offset, 30_888);
    }

    #[test]
    fn classifies_series_without_tg_gc_anchors_as_unsupported() {
        let variant = detect_srs_variant(OMNIC_SERIES_MAGIC).expect("variant");
        match variant {
            SrsVariant::Unsupported { signature_count } => assert_eq!(signature_count, 0),
            SrsVariant::TgGc(_) => panic!("unexpected TGA/GC variant"),
        }
    }

    fn tg_gc_layout(bytes: &[u8]) -> SrsLayout {
        match detect_srs_variant(bytes).expect("variant") {
            SrsVariant::TgGc(layout) => layout,
            SrsVariant::Unsupported { signature_count } => {
                panic!("expected TGA/GC layout, found {signature_count} anchors")
            }
        }
    }

    fn fixture_bytes(relative: &str) -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative);
        std::fs::read(path).expect("fixture bytes")
    }
}
