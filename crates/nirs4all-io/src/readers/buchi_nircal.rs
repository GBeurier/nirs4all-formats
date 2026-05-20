use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralRecord,
};
use serde_json::{json, Value};

use crate::readers::util::{single_signal_record, SingleSignalSpec};
use crate::Reader;

const NIRCAL_MAGIC: &[u8] = b"NIRCAL Project File";

pub struct BuchiNircalReader;

impl Reader for BuchiNircalReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::buchi_nircal"
    }

    fn sniff(&self, head: &[u8], _path: &Path) -> Option<FormatProbe> {
        if head.starts_with(NIRCAL_MAGIC) {
            Some(FormatProbe::new(
                "buchi-nircal",
                self.name(),
                Confidence::Definite,
                "BUCHI/Buhler NIRCal project header",
            ))
        } else {
            None
        }
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let source = SourceFile::from_bytes(path, &bytes, "primary");
        parse_nircal(&bytes, source, self.name())
    }
}

struct NircalSections {
    sample_ids: Vec<String>,
    axis: Vec<f64>,
    spectrum_starts: Vec<usize>,
    spectrum_len: usize,
    title: String,
    targets: TargetMaps,
    warnings: Vec<String>,
}

type TargetMaps = Vec<BTreeMap<String, Value>>;

fn parse_nircal(bytes: &[u8], source: SourceFile, reader: &str) -> Result<Vec<SpectralRecord>> {
    if !bytes.starts_with(NIRCAL_MAGIC) {
        return Err(Error::InvalidRecord(
            "missing BUCHI NIRCal project header".to_string(),
        ));
    }
    let sections = locate_sections(bytes)?;
    if sections.sample_ids.len() != sections.spectrum_starts.len() {
        return Err(Error::InvalidRecord(format!(
            "BUCHI NIRCal has {} sample ids but {} spectra",
            sections.sample_ids.len(),
            sections.spectrum_starts.len()
        )));
    }

    let mut records = Vec::with_capacity(sections.sample_ids.len());
    for (index, (sample_id, start)) in sections
        .sample_ids
        .iter()
        .zip(sections.spectrum_starts.iter())
        .enumerate()
    {
        let values = read_f64_vec(bytes, *start, sections.spectrum_len)?;
        let targets = sections.targets.get(index).cloned().unwrap_or_default();
        let mut metadata = BTreeMap::new();
        metadata.insert("sample_id".to_string(), json!(sample_id));
        metadata.insert("record_index".to_string(), json!(index));
        metadata.insert("project_title".to_string(), json!(sections.title));
        metadata.insert("spectrum_offset".to_string(), json!(start));
        metadata.insert("target_property_count".to_string(), json!(targets.len()));
        let mut warnings = vec!["buchi_nircal_reverse_engineered_sections".to_string()];
        warnings.extend(sections.warnings.clone());
        records.push(single_signal_record(
            "buchi-nircal",
            reader,
            source.clone(),
            SingleSignalSpec {
                axis_values: sections.axis.clone(),
                axis_unit: "cm-1".to_string(),
                axis_kind: AxisKind::Wavenumber,
                values,
                signal_name: "absorbance".to_string(),
                signal_type: SignalType::Absorbance,
                signal_unit: None,
                role: "absorbance".to_string(),
            },
            targets,
            metadata,
            warnings,
        )?);
    }
    Ok(records)
}

fn locate_sections(bytes: &[u8]) -> Result<NircalSections> {
    let values_positions = find_all(bytes, b"Values");
    if values_positions.len() < 7 {
        return Err(Error::InvalidRecord(
            "BUCHI NIRCal Values sections are incomplete".to_string(),
        ));
    }
    let one_values_positions = find_all(bytes, b"\n1 Values");
    let cutoff = values_positions[4];
    let after_sample_ids = one_values_positions
        .into_iter()
        .nth(one_values_positions_before(
            &find_all(bytes, b"\n1 Values"),
            cutoff,
        ))
        .ok_or_else(|| {
            Error::InvalidRecord("BUCHI NIRCal sample id section end not found".to_string())
        })?;
    let sample_ids = parse_sample_ids(&bytes[values_positions[4]..after_sample_ids])?;
    let spectrum_len = parse_spectrum_len(&bytes[values_positions[5]..values_positions[6]])?;
    let axis = parse_axis(bytes, spectrum_len, &values_positions)?;
    let spectrum_starts = spectrum_starts(bytes, spectrum_len, sample_ids.len())?;
    let (targets, warnings) = parse_property_targets(bytes, sample_ids.len())?;
    let title = first_line(bytes);
    Ok(NircalSections {
        sample_ids,
        axis,
        spectrum_starts,
        spectrum_len,
        title,
        targets,
        warnings,
    })
}

fn one_values_positions_before(positions: &[usize], cutoff: usize) -> usize {
    positions
        .iter()
        .filter(|position| **position <= cutoff)
        .count()
}

fn parse_sample_ids(section: &[u8]) -> Result<Vec<String>> {
    let text = decode_text(section);
    let ids = text
        .lines()
        .skip(1)
        .filter_map(|line| {
            line.split_once('/')
                .map(|(_, value)| value.trim().to_string())
        })
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return Err(Error::InvalidRecord(
            "BUCHI NIRCal sample id section is empty".to_string(),
        ));
    }
    Ok(ids)
}

fn parse_spectrum_len(section: &[u8]) -> Result<usize> {
    let text = decode_text(section);
    text.lines()
        .rev()
        .find_map(|line| line.trim().parse::<usize>().ok())
        .ok_or_else(|| Error::InvalidRecord("BUCHI NIRCal spectrum length is missing".to_string()))
}

fn parse_axis(bytes: &[u8], spectrum_len: usize, values_positions: &[usize]) -> Result<Vec<f64>> {
    let needle = format!("{spectrum_len} Values").into_bytes();
    let matches = find_all(bytes, &needle);
    if matches.len() < 3 {
        return Err(Error::InvalidRecord(
            "BUCHI NIRCal wavelength section is missing".to_string(),
        ));
    }
    let start = matches[2] + needle.len() + 1;
    let end = values_positions
        .iter()
        .copied()
        .find(|position| *position > start)
        .ok_or_else(|| {
            Error::InvalidRecord("BUCHI NIRCal wavelength section end not found".to_string())
        })?;
    let text = decode_text(&bytes[start..end]);
    let axis = text
        .lines()
        .filter_map(|line| {
            let (prefix, value) = line.split_once('/')?;
            prefix
                .chars()
                .all(|character| character.is_ascii_digit())
                .then(|| value.trim().parse::<f64>().ok())
                .flatten()
        })
        .collect::<Vec<_>>();
    if axis.len() != spectrum_len {
        return Err(Error::InvalidRecord(format!(
            "BUCHI NIRCal axis has {} points, expected {spectrum_len}",
            axis.len()
        )));
    }
    Ok(axis)
}

fn spectrum_starts(bytes: &[u8], spectrum_len: usize, n_samples: usize) -> Result<Vec<usize>> {
    let begin_positions = find_all(bytes, b"\nbegin\n")
        .into_iter()
        .map(|offset| offset + b"\nbegin\n".len())
        .collect::<Vec<_>>();
    let end_positions = find_all(bytes, b"\nend\n")
        .into_iter()
        .map(|offset| offset + 1)
        .collect::<Vec<_>>();
    let spcinfo = find_all(bytes, b"\n38/{")
        .into_iter()
        .skip(1)
        .map(|offset| offset + 1)
        .collect::<Vec<_>>();
    let first_info = spcinfo.first().copied().ok_or_else(|| {
        Error::InvalidRecord("BUCHI NIRCal spectrum metadata anchors are missing".to_string())
    })?;
    let expected_bytes = spectrum_len * 8;
    let starts = begin_positions
        .iter()
        .zip(end_positions.iter())
        .filter_map(|(begin, end)| {
            ((*end > *begin) && (*end - *begin - 1 == expected_bytes) && *begin > first_info)
                .then_some(*begin)
        })
        .take(n_samples)
        .collect::<Vec<_>>();
    if starts.len() != n_samples {
        return Err(Error::InvalidRecord(format!(
            "BUCHI NIRCal found {} spectra, expected {n_samples}",
            starts.len()
        )));
    }
    Ok(starts)
}

fn parse_property_targets(bytes: &[u8], n_samples: usize) -> Result<(TargetMaps, Vec<String>)> {
    let Some(property_start) = find_first(bytes, b"10/Properties\n18/Property Selection", 0) else {
        return Ok((vec![BTreeMap::new(); n_samples], Vec::new()));
    };
    let Some(first_begin) = find_first(bytes, b"begin", property_start) else {
        return Err(Error::InvalidRecord(
            "BUCHI NIRCal property section has no begin marker".to_string(),
        ));
    };
    let property_count = parse_property_count(&bytes[property_start..first_begin])?;
    if property_count == 0 {
        return Ok((vec![BTreeMap::new(); n_samples], Vec::new()));
    }

    let raw_names = parse_property_name_lines(bytes, property_start, property_count)?;
    let (names, mut warnings) = normalized_property_names(&raw_names);
    let value_starts = property_value_starts(bytes, &raw_names, property_count, n_samples)?;
    let mut targets = Vec::with_capacity(n_samples);
    let mut nonzero_values = 0_usize;

    for start in value_starts {
        let values = read_f64_values(bytes, start, property_count, "BUCHI NIRCal property block")?;
        let mut sample_targets = BTreeMap::new();
        for (name, value) in names.iter().zip(values.iter()) {
            if value.is_finite() && *value != 0.0 {
                nonzero_values += 1;
                sample_targets.insert(name.clone(), json!(value));
            } else {
                sample_targets.insert(name.clone(), Value::Null);
            }
        }
        targets.push(sample_targets);
    }

    if nonzero_values == 0 {
        warnings.push("buchi_nircal_zero_property_values_as_missing".to_string());
    }
    Ok((targets, warnings))
}

fn parse_property_count(section: &[u8]) -> Result<usize> {
    let text = decode_text(section);
    let line = text.lines().nth(4).ok_or_else(|| {
        Error::InvalidRecord("BUCHI NIRCal property count line is missing".to_string())
    })?;
    line.trim()
        .strip_suffix(" Values")
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| {
            Error::InvalidRecord(format!(
                "BUCHI NIRCal property count line is invalid: {line}"
            ))
        })
}

fn parse_property_name_lines(
    bytes: &[u8],
    property_start: usize,
    property_count: usize,
) -> Result<Vec<String>> {
    let marker = format!("{property_count} Values").into_bytes();
    let markers = find_all(bytes, &marker)
        .into_iter()
        .filter(|position| *position >= property_start)
        .collect::<Vec<_>>();
    let names_marker = markers.get(2).copied().ok_or_else(|| {
        Error::InvalidRecord("BUCHI NIRCal property name section is missing".to_string())
    })?;
    let mut start = names_marker + marker.len();
    if bytes.get(start) == Some(&b'\n') {
        start += 1;
    }
    let text = decode_text(&bytes[start..bytes.len().min(start + 4096)]);
    let names = text
        .lines()
        .take(property_count)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if names.len() != property_count {
        return Err(Error::InvalidRecord(format!(
            "BUCHI NIRCal parsed {} property names, expected {property_count}",
            names.len()
        )));
    }
    Ok(names)
}

fn normalized_property_names(raw_names: &[String]) -> (Vec<String>, Vec<String>) {
    let mut names = raw_names
        .iter()
        .map(|line| {
            line.split_once('/')
                .map(|(_, value)| value)
                .unwrap_or(line)
                .trim()
                .replace('/', "_")
        })
        .collect::<Vec<_>>();
    let mut counts = BTreeMap::new();
    for name in &names {
        *counts.entry(name.clone()).or_insert(0_usize) += 1;
    }
    let has_duplicates = counts.values().any(|count| *count > 1);
    if has_duplicates {
        let mut seen = BTreeMap::new();
        for name in &mut names {
            if counts.get(name).copied().unwrap_or(0) > 1 {
                let entry = seen.entry(name.clone()).or_insert(0_usize);
                *entry += 1;
                *name = format!("{name}_{entry}");
            }
        }
        (
            names,
            vec!["buchi_nircal_duplicate_property_names_normalized".to_string()],
        )
    } else {
        (names, Vec::new())
    }
}

fn property_value_starts(
    bytes: &[u8],
    raw_names: &[String],
    property_count: usize,
    n_samples: usize,
) -> Result<Vec<usize>> {
    let names_block = raw_names.join("\n").into_bytes();
    let marker = format!("{property_count} Values\nbegin\n").into_bytes();
    let mut starts = Vec::with_capacity(n_samples);
    for position in find_all(bytes, &names_block) {
        let mut cursor = position + names_block.len();
        if bytes.get(cursor) == Some(&b'\n') {
            cursor += 1;
        } else {
            continue;
        }
        let digit_start = cursor;
        while bytes
            .get(cursor)
            .is_some_and(|value| value.is_ascii_digit())
        {
            cursor += 1;
        }
        if cursor == digit_start || bytes.get(cursor) != Some(&b'\n') {
            continue;
        }
        cursor += 1;
        if bytes.get(cursor..cursor + marker.len()) == Some(marker.as_slice()) {
            starts.push(cursor + marker.len());
        }
    }
    if starts.len() < n_samples {
        return Err(Error::InvalidRecord(format!(
            "BUCHI NIRCal found {} property value blocks, expected {n_samples}",
            starts.len()
        )));
    }
    starts.truncate(n_samples);
    Ok(starts)
}

fn read_f64_vec(bytes: &[u8], offset: usize, len: usize) -> Result<Vec<f64>> {
    read_f64_values(bytes, offset, len, "BUCHI NIRCal spectrum")
}

fn read_f64_values(bytes: &[u8], offset: usize, len: usize, context: &str) -> Result<Vec<f64>> {
    let end = offset + len * 8;
    if end > bytes.len() {
        return Err(Error::InvalidRecord(format!(
            "{context} extends past end of file"
        )));
    }
    (0..len)
        .map(|index| {
            let start = offset + index * 8;
            let value = bytes
                .get(start..start + 8)
                .ok_or_else(|| Error::InvalidRecord(format!("truncated {context} f64 value")))?;
            Ok(f64::from_le_bytes(value.try_into().expect("slice len")))
        })
        .collect()
}

fn first_line(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|byte| *byte == b'\n').unwrap_or(0);
    decode_text(&bytes[..end]).trim().to_string()
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

fn find_first(bytes: &[u8], needle: &[u8], offset: usize) -> Option<usize> {
    bytes
        .get(offset..)?
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|position| offset + position)
}

fn decode_text(bytes: &[u8]) -> String {
    std::str::from_utf8(bytes)
        .map(|value| value.to_string())
        .unwrap_or_else(|_| bytes.iter().map(|byte| *byte as char).collect())
}
