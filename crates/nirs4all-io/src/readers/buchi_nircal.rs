use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SourceFile, SpectralRecord,
};
use serde_json::json;

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
}

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
        let mut metadata = BTreeMap::new();
        metadata.insert("sample_id".to_string(), json!(sample_id));
        metadata.insert("record_index".to_string(), json!(index));
        metadata.insert("project_title".to_string(), json!(sections.title));
        metadata.insert("spectrum_offset".to_string(), json!(start));
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
            BTreeMap::new(),
            metadata,
            vec!["buchi_nircal_reverse_engineered_sections".to_string()],
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
    let title = first_line(bytes);
    Ok(NircalSections {
        sample_ids,
        axis,
        spectrum_starts,
        spectrum_len,
        title,
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

fn read_f64_vec(bytes: &[u8], offset: usize, len: usize) -> Result<Vec<f64>> {
    let end = offset + len * 8;
    if end > bytes.len() {
        return Err(Error::InvalidRecord(
            "BUCHI NIRCal spectrum extends past end of file".to_string(),
        ));
    }
    (0..len)
        .map(|index| {
            let start = offset + index * 8;
            let value = bytes.get(start..start + 8).ok_or_else(|| {
                Error::InvalidRecord("truncated BUCHI NIRCal f64 value".to_string())
            })?;
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

fn decode_text(bytes: &[u8]) -> String {
    std::str::from_utf8(bytes)
        .map(|value| value.to_string())
        .unwrap_or_else(|_| bytes.iter().map(|byte| *byte as char).collect())
}
