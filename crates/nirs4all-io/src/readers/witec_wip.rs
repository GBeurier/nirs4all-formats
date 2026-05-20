use std::collections::BTreeMap;
use std::ops::Range;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralArray,
    SpectralAxis, SpectralRecord,
};
use serde_json::json;

use crate::readers::util::provenance;
use crate::Reader;

const FORMAT: &str = "witec-wip";
const MODERN_MAGIC: &[u8] = b"WIT_PR06";
const LEGACY_MAGIC: &[u8] = b"WIT^";
const SUPPORTED_SIZE_X: u32 = 90;
const SUPPORTED_SIZE_Y: u32 = 55;
const SUPPORTED_SIZE_GRAPH: u32 = 1024;
const SUPPORTED_DATA_TYPE: u32 = 6;

pub struct WitecWipReader;

impl Reader for WitecWipReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::witec_wip"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "wip" | "wid") {
            return None;
        }
        if head.starts_with(MODERN_MAGIC) {
            return Some(FormatProbe::new(
                FORMAT,
                self.name(),
                Confidence::Definite,
                "WiTec WIP project container with WIT_PR06 signature",
            ));
        }
        if head.starts_with(LEGACY_MAGIC) {
            return Some(FormatProbe::new(
                FORMAT,
                self.name(),
                Confidence::Definite,
                "WiTec WIP/WID binary project container; legacy WIT^ layout",
            ));
        }
        None
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if bytes.starts_with(LEGACY_MAGIC) && !bytes.starts_with(MODERN_MAGIC) {
            return Err(legacy_unsupported(path));
        }
        if !bytes.starts_with(MODERN_MAGIC) {
            return Err(Error::InvalidRecord(format!(
                "unsupported WiTec WIP layout for {}: expected WIT_PR06 signature",
                path.display()
            )));
        }

        parse_wit_pr06(path, &bytes, self.name())
    }
}

fn parse_wit_pr06(path: &Path, bytes: &[u8], reader: &str) -> Result<Vec<SpectralRecord>> {
    let graph = find_supported_graph(bytes)?;
    let line_valid = read_line_valid(bytes, &graph.entry)?;
    let valid_lines = line_valid.iter().filter(|valid| **valid).count();
    if valid_lines == 0 {
        return Err(Error::InvalidRecord(
            "unsupported WiTec WIP layout: TDGraph LineValid has no valid lines".to_string(),
        ));
    }

    let coeffs = read_free_polynom(bytes, graph.size_graph)?;
    let axis_values: Vec<f64> = (0..graph.size_graph)
        .map(|bin| eval_polynom(&coeffs, f64::from(bin)))
        .collect();

    let source = SourceFile::from_bytes(path, bytes, "primary");
    let warnings = vec![
        "witec_wip_experimental_parser".to_string(),
        "witec_wip_layout_limited_to_wit_pr06_tdgraph_u16_sa4".to_string(),
    ];

    let size_x = graph.size_x as usize;
    let size_graph = graph.size_graph as usize;
    let bytes_per_spectrum = size_graph * 2;
    let mut records = Vec::with_capacity(valid_lines * size_x);

    for (y_index, valid) in line_valid.iter().enumerate() {
        if !valid {
            continue;
        }
        for x_index in 0..size_x {
            let spectrum_index = y_index * size_x + x_index;
            let offset = graph.data_start + spectrum_index * bytes_per_spectrum;
            let end = offset + bytes_per_spectrum;
            let values = read_u16_le_values(bytes, offset..end)?;

            let axis = SpectralAxis::new(axis_values.clone(), "nm", AxisKind::Wavelength)?;
            let signal = SpectralArray::new(
                axis,
                values,
                vec!["x".to_string()],
                SignalType::RawCounts,
                Some("counts".to_string()),
                "raw_counts",
                "file",
            )?;
            let mut signals = BTreeMap::new();
            signals.insert("raw_counts".to_string(), signal);

            let mut metadata = BTreeMap::new();
            metadata.insert("x_index".to_string(), json!(x_index));
            metadata.insert("y_index".to_string(), json!(y_index));
            metadata.insert("size_x".to_string(), json!(graph.size_x));
            metadata.insert("size_y".to_string(), json!(graph.size_y));
            metadata.insert("size_graph".to_string(), json!(graph.size_graph));
            metadata.insert("data_type".to_string(), json!(graph.data_type));

            let record = SpectralRecord {
                signals,
                signal_type: SignalType::RawCounts,
                targets: BTreeMap::new(),
                metadata,
                provenance: provenance(FORMAT, reader, source.clone(), warnings.clone()),
                quality_flags: Vec::new(),
            };
            record.validate()?;
            records.push(record);
        }
    }

    Ok(records)
}

fn find_supported_graph(bytes: &[u8]) -> Result<GraphLayout> {
    for entry in find_entries(bytes, 0..bytes.len(), "TDGraph") {
        let Ok(version) = read_u32_field(bytes, &entry, "Version") else {
            continue;
        };
        let Ok(size_x) = read_u32_field(bytes, &entry, "SizeX") else {
            continue;
        };
        let Ok(size_y) = read_u32_field(bytes, &entry, "SizeY") else {
            continue;
        };
        let Ok(size_graph) = read_u32_field(bytes, &entry, "SizeGraph") else {
            continue;
        };
        let graph_data = required_entry(bytes, entry.range(), "GraphData")?;
        let dimension = read_u32_field(bytes, &graph_data, "Dimension")?;
        let data_type = read_u32_field(bytes, &graph_data, "DataType")?;
        let data = required_entry(bytes, graph_data.range(), "Data")?;
        if version != 1
            || size_x != SUPPORTED_SIZE_X
            || size_y != SUPPORTED_SIZE_Y
            || size_graph != SUPPORTED_SIZE_GRAPH
            || dimension != 2
            || data_type != SUPPORTED_DATA_TYPE
        {
            return Err(Error::InvalidRecord(format!(
                "unsupported WiTec WIP TDGraph layout: Version={version}, DataType={data_type}, Dimension={dimension}, SizeX={size_x}, SizeY={size_y}, SizeGraph={size_graph}"
            )));
        }

        require_type(&data, 7, "TDGraph GraphData Data")?;
        let expected_data_len = size_x as usize * size_y as usize * size_graph as usize * 2;
        if data.len() != expected_data_len {
            return Err(Error::InvalidRecord(format!(
                "unsupported WiTec WIP TDGraph layout: data byte length {} does not match expected {expected_data_len}",
                data.len()
            )));
        }

        return Ok(GraphLayout {
            entry,
            size_x,
            size_y,
            size_graph,
            data_type,
            data_start: data.data_start,
        });
    }

    Err(Error::InvalidRecord(
        "unsupported WiTec WIP layout: no TDGraph with SizeX=90, SizeY=55, SizeGraph=1024, DataType=6 found".to_string(),
    ))
}

fn read_line_valid(bytes: &[u8], graph: &WipEntry) -> Result<Vec<bool>> {
    let line_valid = required_entry(bytes, graph.range(), "LineValid")?;
    require_type(&line_valid, 8, "TDGraph LineValid")?;
    if line_valid.len() != SUPPORTED_SIZE_Y as usize {
        return Err(Error::InvalidRecord(format!(
            "unsupported WiTec WIP TDGraph layout: LineValid length {} does not match SizeY={SUPPORTED_SIZE_Y}",
            line_valid.len()
        )));
    }
    Ok(bytes[line_valid.range()]
        .iter()
        .map(|value| *value != 0)
        .collect())
}

fn read_free_polynom(bytes: &[u8], size_graph: u32) -> Result<Vec<f64>> {
    let file = 0..bytes.len();
    let order = read_u32_named(bytes, file.clone(), "FreePolynomOrder")?;
    let start_bin = read_f64_named(bytes, file.clone(), "FreePolynomStartBin")?;
    let stop_bin = read_f64_named(bytes, file.clone(), "FreePolynomStopBin")?;
    if order != 6 || start_bin != 0.0 || stop_bin != f64::from(size_graph) {
        return Err(Error::InvalidRecord(format!(
            "unsupported WiTec WIP spectral axis layout: FreePolynomOrder={order}, FreePolynomStartBin={start_bin}, FreePolynomStopBin={stop_bin}"
        )));
    }

    let free_polynom = required_entry(bytes, file, "FreePolynom")?;
    require_type(&free_polynom, 2, "FreePolynom")?;
    let expected_len = (order as usize + 1) * 8;
    if free_polynom.len() != expected_len {
        return Err(Error::InvalidRecord(format!(
            "unsupported WiTec WIP spectral axis layout: FreePolynom length {} does not match order {order}",
            free_polynom.len()
        )));
    }
    read_f64_values(bytes, free_polynom.range())
}

fn read_u32_named(bytes: &[u8], range: Range<usize>, name: &str) -> Result<u32> {
    let parent = WipEntry {
        type_code: 0,
        data_start: range.start,
        data_end: range.end,
    };
    read_u32_field(bytes, &parent, name)
}

fn read_f64_named(bytes: &[u8], range: Range<usize>, name: &str) -> Result<f64> {
    let entry = required_entry(bytes, range, name)?;
    require_type(&entry, 2, name)?;
    if entry.len() != 8 {
        return Err(Error::InvalidRecord(format!(
            "unsupported WiTec WIP layout: {name} has {} bytes, expected 8",
            entry.len()
        )));
    }
    let values = read_f64_values(bytes, entry.range())?;
    Ok(values[0])
}

fn read_u32_field(bytes: &[u8], parent: &WipEntry, name: &str) -> Result<u32> {
    let entry = required_entry(bytes, parent.range(), name)?;
    require_type(&entry, 5, name)?;
    if entry.len() != 4 {
        return Err(Error::InvalidRecord(format!(
            "unsupported WiTec WIP layout: {name} has {} bytes, expected 4",
            entry.len()
        )));
    }
    Ok(u32::from_le_bytes(
        bytes[entry.data_start..entry.data_end]
            .try_into()
            .expect("u32 field length checked"),
    ))
}

fn read_u16_le_values(bytes: &[u8], range: Range<usize>) -> Result<Vec<f64>> {
    if range.end > bytes.len() || !range.len().is_multiple_of(2) {
        return Err(Error::InvalidRecord(
            "unsupported WiTec WIP layout: invalid u16 data range".to_string(),
        ));
    }
    Ok(bytes[range]
        .chunks_exact(2)
        .map(|chunk| f64::from(u16::from_le_bytes([chunk[0], chunk[1]])))
        .collect())
}

fn read_f64_values(bytes: &[u8], range: Range<usize>) -> Result<Vec<f64>> {
    if range.end > bytes.len() || !range.len().is_multiple_of(8) {
        return Err(Error::InvalidRecord(
            "unsupported WiTec WIP layout: invalid f64 data range".to_string(),
        ));
    }
    Ok(bytes[range]
        .chunks_exact(8)
        .map(|chunk| {
            f64::from_le_bytes(
                chunk
                    .try_into()
                    .expect("chunks_exact(8) always yields 8 bytes"),
            )
        })
        .collect())
}

fn eval_polynom(coeffs: &[f64], x: f64) -> f64 {
    coeffs.iter().rev().fold(0.0, |acc, coeff| acc * x + coeff)
}

fn required_entry(bytes: &[u8], range: Range<usize>, name: &str) -> Result<WipEntry> {
    let entries = find_entries(bytes, range, name);
    match entries.as_slice() {
        [entry] => Ok(*entry),
        [] => Err(Error::InvalidRecord(format!(
            "unsupported WiTec WIP layout: missing {name}"
        ))),
        _ => Err(Error::InvalidRecord(format!(
            "unsupported WiTec WIP layout: multiple {name} entries found"
        ))),
    }
}

fn find_entries(bytes: &[u8], range: Range<usize>, name: &str) -> Vec<WipEntry> {
    let needle = name.as_bytes();
    let mut out = Vec::new();
    if needle.is_empty() || range.start >= range.end || range.end > bytes.len() {
        return out;
    }

    let mut offset = range.start;
    while offset + needle.len() <= range.end {
        let Some(relative) = find_bytes(&bytes[offset..range.end], needle) else {
            break;
        };
        let name_start = offset + relative;
        offset = name_start + 1;
        if name_start < 4 {
            continue;
        }
        let entry_start = name_start - 4;
        if entry_start < range.start {
            continue;
        }
        let encoded_name_len = u32::from_le_bytes(
            bytes[entry_start..name_start]
                .try_into()
                .expect("name length prefix is 4 bytes"),
        ) as usize;
        if encoded_name_len != needle.len() {
            continue;
        }
        let Ok(entry) = parse_entry(bytes, entry_start) else {
            continue;
        };
        if entry.data_end <= range.end {
            out.push(entry);
        }
    }
    out
}

fn parse_entry(bytes: &[u8], entry_start: usize) -> Result<WipEntry> {
    let name_len_end = entry_start + 4;
    if name_len_end > bytes.len() {
        return Err(Error::InvalidRecord(
            "unsupported WiTec WIP layout: truncated entry name length".to_string(),
        ));
    }
    let name_len = u32::from_le_bytes(
        bytes[entry_start..name_len_end]
            .try_into()
            .expect("name length slice is 4 bytes"),
    ) as usize;
    if !(1..=128).contains(&name_len) {
        return Err(Error::InvalidRecord(
            "unsupported WiTec WIP layout: invalid entry name length".to_string(),
        ));
    }

    let type_pos = name_len_end + name_len;
    let data_start_pos = type_pos + 4;
    let data_end_pos = data_start_pos + 8;
    let header_end = data_end_pos + 8;
    if header_end > bytes.len() {
        return Err(Error::InvalidRecord(
            "unsupported WiTec WIP layout: truncated entry header".to_string(),
        ));
    }
    let name = &bytes[name_len_end..type_pos];
    if !name
        .iter()
        .all(|byte| byte.is_ascii_graphic() || *byte == b' ')
    {
        return Err(Error::InvalidRecord(
            "unsupported WiTec WIP layout: non-ASCII entry name".to_string(),
        ));
    }

    let type_code = u32::from_le_bytes(
        bytes[type_pos..data_start_pos]
            .try_into()
            .expect("type slice is 4 bytes"),
    );
    let data_start = u64::from_le_bytes(
        bytes[data_start_pos..data_end_pos]
            .try_into()
            .expect("data start slice is 8 bytes"),
    ) as usize;
    let data_end = u64::from_le_bytes(
        bytes[data_end_pos..header_end]
            .try_into()
            .expect("data end slice is 8 bytes"),
    ) as usize;
    if data_start != header_end || data_start > data_end || data_end > bytes.len() {
        return Err(Error::InvalidRecord(
            "unsupported WiTec WIP layout: inconsistent entry offsets".to_string(),
        ));
    }

    Ok(WipEntry {
        type_code,
        data_start,
        data_end,
    })
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn require_type(entry: &WipEntry, expected: u32, name: &str) -> Result<()> {
    if entry.type_code != expected {
        return Err(Error::InvalidRecord(format!(
            "unsupported WiTec WIP layout: {name} type {} does not match expected {expected}",
            entry.type_code
        )));
    }
    Ok(())
}

fn legacy_unsupported(path: &Path) -> Error {
    Error::InvalidRecord(format!(
        "legacy WiTec WIP/WID WIT^ project layout is not supported by nirs4all-io: {}. The current native subset is limited to the WIT_PR06 TDGraph layout validated by Sa4.wip; export other WiTec projects from WiTec Project/FIVE as ASCII text and load the .txt export.",
        path.display()
    ))
}

#[derive(Clone, Copy)]
struct WipEntry {
    type_code: u32,
    data_start: usize,
    data_end: usize,
}

impl WipEntry {
    fn len(&self) -> usize {
        self.data_end - self.data_start
    }

    fn range(&self) -> Range<usize> {
        self.data_start..self.data_end
    }
}

struct GraphLayout {
    entry: WipEntry,
    size_x: u32,
    size_y: u32,
    size_graph: u32,
    data_type: u32,
    data_start: usize,
}
