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
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(&self, path: &Path, bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
        let source = SourceFile::from_bytes(path, bytes, "primary");
        parse_nircal(bytes, source, self.name())
    }
}

struct NircalSections {
    sample_ids: Vec<String>,
    axis: Vec<f64>,
    spectrum_starts: Vec<usize>,
    spectrum_len: usize,
    title: String,
    project_guid: Option<String>,
    project_file_version: Option<String>,
    spectrum_infos: Vec<SpectrumInfo>,
    targets: TargetMaps,
    warnings: Vec<String>,
}

type TargetMaps = Vec<BTreeMap<String, Value>>;

struct SampleReplicate {
    index: usize,
    count: usize,
}

#[derive(Clone, Default)]
struct SpectrumInfo {
    sample_guid: Option<String>,
    comment: Option<String>,
    description: Option<String>,
    scans: Option<u64>,
    resolution: Option<u64>,
    declared_wavenumber_count: Option<u64>,
    declared_wavenumber_step: Option<f64>,
    declared_wavenumber_start: Option<f64>,
    device: Option<String>,
    software_version: Option<String>,
    created: Option<String>,
    modified: Option<String>,
    creator: Option<String>,
    creator_login: Option<String>,
    modified_by: Option<String>,
    modifier_login: Option<String>,
    instrument_serial: Option<String>,
    measurement_cell: Option<String>,
    option_serial: Option<String>,
    reference_substance: Option<String>,
    instrument_version: Option<String>,
    computer_name: Option<String>,
    gain_factor: Option<f64>,
    gain: Option<f64>,
    instrument_temperature_c: Option<f64>,
    sample_temperature_c: Option<f64>,
}

impl SpectrumInfo {
    fn insert_into(&self, metadata: &mut BTreeMap<String, Value>) {
        insert_opt_string(metadata, "sample_guid", self.sample_guid.as_ref());
        insert_opt_string(metadata, "comment", self.comment.as_ref());
        insert_opt_string(metadata, "description", self.description.as_ref());
        insert_opt_u64(metadata, "scans", self.scans);
        insert_opt_u64(metadata, "resolution", self.resolution);
        insert_opt_u64(
            metadata,
            "declared_wavenumber_count",
            self.declared_wavenumber_count,
        );
        insert_opt_f64(
            metadata,
            "declared_wavenumber_step",
            self.declared_wavenumber_step,
        );
        insert_opt_f64(
            metadata,
            "declared_wavenumber_start",
            self.declared_wavenumber_start,
        );
        insert_opt_string(metadata, "device", self.device.as_ref());
        insert_opt_string(metadata, "software_version", self.software_version.as_ref());
        insert_opt_string(metadata, "created", self.created.as_ref());
        insert_opt_string(metadata, "modified", self.modified.as_ref());
        insert_opt_string(metadata, "creator", self.creator.as_ref());
        insert_opt_string(metadata, "creator_login", self.creator_login.as_ref());
        insert_opt_string(metadata, "modified_by", self.modified_by.as_ref());
        insert_opt_string(metadata, "modifier_login", self.modifier_login.as_ref());
        insert_opt_string(
            metadata,
            "instrument_serial",
            self.instrument_serial.as_ref(),
        );
        insert_opt_string(metadata, "measurement_cell", self.measurement_cell.as_ref());
        insert_opt_string(metadata, "option_serial", self.option_serial.as_ref());
        insert_opt_string(
            metadata,
            "reference_substance",
            self.reference_substance.as_ref(),
        );
        insert_opt_string(
            metadata,
            "instrument_version",
            self.instrument_version.as_ref(),
        );
        insert_opt_string(metadata, "computer_name", self.computer_name.as_ref());
        insert_opt_f64(metadata, "gain_factor", self.gain_factor);
        insert_opt_f64(metadata, "gain", self.gain);
        insert_opt_f64(
            metadata,
            "instrument_temperature_c",
            self.instrument_temperature_c,
        );
        insert_opt_f64(metadata, "sample_temperature_c", self.sample_temperature_c);
    }
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
    let replicate_info = sample_replicate_info(&sections.sample_ids);
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
        if let Some(project_guid) = &sections.project_guid {
            metadata.insert("project_guid".to_string(), json!(project_guid));
        }
        if let Some(version) = &sections.project_file_version {
            metadata.insert("project_file_version".to_string(), json!(version));
        }
        metadata.insert("spectrum_offset".to_string(), json!(start));
        metadata.insert("target_property_count".to_string(), json!(targets.len()));
        let replicate = &replicate_info[index];
        metadata.insert("sample_replicate_index".to_string(), json!(replicate.index));
        metadata.insert("sample_replicate_count".to_string(), json!(replicate.count));
        if let Some(info) = sections.spectrum_infos.get(index) {
            info.insert_into(&mut metadata);
        }
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
    let (spectrum_infos, mut warnings) = parse_spectrum_infos(bytes, sample_ids.len());
    let (targets, target_warnings) = parse_property_targets(bytes, sample_ids.len())?;
    warnings.extend(target_warnings);
    let title = first_line(bytes);
    let project_guid = project_guid(bytes);
    let project_file_version = project_file_version(&title);
    Ok(NircalSections {
        sample_ids,
        axis,
        spectrum_starts,
        spectrum_len,
        title,
        project_guid,
        project_file_version,
        spectrum_infos,
        targets,
        warnings,
    })
}

fn sample_replicate_info(sample_ids: &[String]) -> Vec<SampleReplicate> {
    let mut counts = BTreeMap::new();
    for sample_id in sample_ids {
        *counts.entry(sample_id.clone()).or_insert(0_usize) += 1;
    }

    let mut seen = BTreeMap::new();
    sample_ids
        .iter()
        .map(|sample_id| {
            let index = seen.entry(sample_id.clone()).or_insert(0_usize);
            *index += 1;
            SampleReplicate {
                index: *index,
                count: counts.get(sample_id).copied().unwrap_or(1),
            }
        })
        .collect()
}

fn project_guid(bytes: &[u8]) -> Option<String> {
    let text = decode_text(&bytes[..bytes.len().min(2048)]);
    text.lines().find_map(|line| {
        line.strip_prefix("38/{")
            .and_then(|value| value.strip_suffix('}'))
            .map(|value| value.to_string())
    })
}

fn project_file_version(title: &str) -> Option<String> {
    let marker = "NIRCAL Project File Version ";
    let tail = title.strip_prefix(marker)?;
    let end = tail.find(" / ").unwrap_or(tail.len());
    let version = tail[..end].trim();
    (!version.is_empty()).then(|| version.to_string())
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

fn parse_spectrum_infos(bytes: &[u8], n_samples: usize) -> (Vec<SpectrumInfo>, Vec<String>) {
    let anchors = find_all(bytes, b"\n38/{")
        .into_iter()
        .skip(1)
        .map(|offset| offset + 1)
        .take(n_samples)
        .collect::<Vec<_>>();
    if anchors.len() != n_samples {
        return (
            vec![SpectrumInfo::default(); n_samples],
            vec![format!(
                "buchi_nircal_spectrum_metadata_count_mismatch: found {}, expected {n_samples}",
                anchors.len()
            )],
        );
    }
    (
        anchors
            .iter()
            .map(|anchor| parse_spectrum_info(bytes, *anchor))
            .collect(),
        Vec::new(),
    )
}

fn parse_spectrum_info(bytes: &[u8], guid_line_start: usize) -> SpectrumInfo {
    let guid_line = line_at(bytes, guid_line_start);
    let sample_guid = length_prefixed_value(guid_line).and_then(|value| normalize_guid(&value));
    let previous = previous_length_prefixed_values(bytes, guid_line_start, 3);
    let comment = previous.get(1).and_then(|value| non_empty_value(value));
    let mut description = previous.get(2).and_then(|value| non_empty_value(value));
    let tokens = following_length_prefixed_values(bytes, guid_line_start, 90);
    if description.is_none() {
        description = token_string(&tokens, 16);
    }

    SpectrumInfo {
        sample_guid,
        comment,
        description,
        scans: token_u64(&tokens, 5),
        resolution: token_u64(&tokens, 6),
        declared_wavenumber_count: token_u64(&tokens, 7),
        declared_wavenumber_step: token_f64(&tokens, 8),
        declared_wavenumber_start: token_f64(&tokens, 9),
        device: token_string(&tokens, 49),
        software_version: token_string(&tokens, 26),
        created: timestamp_from_tokens(&tokens, 19),
        modified: timestamp_from_tokens(&tokens, 32),
        creator: token_string(&tokens, 27),
        creator_login: token_string(&tokens, 30),
        modified_by: token_string(&tokens, 40),
        modifier_login: token_string(&tokens, 43),
        instrument_serial: token_string(&tokens, 45),
        measurement_cell: token_string(&tokens, 46),
        option_serial: token_string(&tokens, 47),
        reference_substance: token_string(&tokens, 48),
        instrument_version: token_string(&tokens, 50),
        computer_name: token_string(&tokens, 51),
        gain_factor: numeric_after_label(&tokens, |label| label == "Gain Factor"),
        gain: numeric_after_label(&tokens, |label| label == "Gain"),
        instrument_temperature_c: numeric_after_label(&tokens, |label| {
            label.starts_with("Instrument Temperature")
        }),
        sample_temperature_c: numeric_after_label(&tokens, |label| {
            label.starts_with("Sample Temperature")
        }),
    }
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
    let mut value_rows = Vec::with_capacity(n_samples);
    let mut has_nonzero_value = false;

    for start in value_starts {
        let values = read_f64_values(bytes, start, property_count, "BUCHI NIRCal property block")?;
        if values
            .iter()
            .any(|value| value.is_finite() && *value != 0.0)
        {
            has_nonzero_value = true;
        }
        value_rows.push(values);
    }

    let zero_values_are_missing = !has_nonzero_value;
    if zero_values_are_missing {
        warnings.push("buchi_nircal_zero_property_values_as_missing".to_string());
    }
    let targets = value_rows
        .iter()
        .map(|values| property_targets_from_values(&names, values, zero_values_are_missing))
        .collect::<Vec<_>>();
    Ok((targets, warnings))
}

fn property_targets_from_values(
    names: &[String],
    values: &[f64],
    zero_values_are_missing: bool,
) -> BTreeMap<String, Value> {
    names
        .iter()
        .zip(values.iter())
        .map(|(name, value)| {
            let value = if value.is_finite() && !(zero_values_are_missing && *value == 0.0) {
                json!(value)
            } else {
                Value::Null
            };
            (name.clone(), value)
        })
        .collect()
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

fn line_at(bytes: &[u8], start: usize) -> &[u8] {
    let end = bytes[start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|relative| start + relative)
        .unwrap_or(bytes.len());
    &bytes[start..end]
}

fn previous_length_prefixed_values(bytes: &[u8], offset: usize, count: usize) -> Vec<String> {
    let mut values = Vec::with_capacity(count);
    let mut end = offset.saturating_sub(1);
    while values.len() < count && end > 0 {
        let start = bytes[..end]
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map(|position| position + 1)
            .unwrap_or(0);
        if let Some(value) = length_prefixed_value(&bytes[start..end]) {
            values.push(value);
        }
        if start == 0 {
            break;
        }
        end = start - 1;
    }
    values.reverse();
    values
}

fn following_length_prefixed_values(
    bytes: &[u8],
    guid_line_start: usize,
    limit: usize,
) -> Vec<String> {
    let mut values = Vec::with_capacity(limit);
    let Some(mut cursor) = bytes[guid_line_start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|relative| guid_line_start + relative + 1)
    else {
        return values;
    };
    while values.len() < limit && cursor < bytes.len() {
        let end = bytes[cursor..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|relative| cursor + relative)
            .unwrap_or(bytes.len());
        let line = &bytes[cursor..end];
        if line == b"begin" {
            break;
        }
        if let Some(value) = length_prefixed_value(line) {
            values.push(value);
        }
        if end == bytes.len() {
            break;
        }
        cursor = end + 1;
    }
    values
}

fn length_prefixed_value(line: &[u8]) -> Option<String> {
    let slash = line.iter().position(|byte| *byte == b'/')?;
    if slash == 0 || !line[..slash].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let value = decode_text(&line[slash + 1..]).trim().to_string();
    Some(value)
}

fn normalize_guid(value: &str) -> Option<String> {
    value
        .trim()
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .or_else(|| (!value.trim().is_empty()).then_some(value.trim()))
        .map(ToOwned::to_owned)
}

fn token_string(tokens: &[String], index: usize) -> Option<String> {
    tokens.get(index).and_then(|value| non_empty_value(value))
}

fn token_u64(tokens: &[String], index: usize) -> Option<u64> {
    tokens.get(index)?.trim().parse().ok()
}

fn token_f64(tokens: &[String], index: usize) -> Option<f64> {
    tokens.get(index)?.trim().parse().ok()
}

fn timestamp_from_tokens(tokens: &[String], start: usize) -> Option<String> {
    let second: u32 = tokens.get(start)?.trim().parse().ok()?;
    let minute: u32 = tokens.get(start + 1)?.trim().parse().ok()?;
    let hour: u32 = tokens.get(start + 2)?.trim().parse().ok()?;
    let day: u32 = tokens.get(start + 3)?.trim().parse().ok()?;
    let month: u32 = tokens.get(start + 4)?.trim().parse().ok()?;
    let year: u32 = tokens.get(start + 5)?.trim().parse().ok()?;
    Some(format!(
        "{year:04}/{month:02}/{day:02} {hour:02}:{minute:02}:{second:02}"
    ))
}

fn numeric_after_label(tokens: &[String], predicate: impl Fn(&str) -> bool) -> Option<f64> {
    tokens.windows(2).find_map(|pair| {
        predicate(pair[0].trim())
            .then(|| pair[1].trim().parse().ok())
            .flatten()
    })
}

fn non_empty_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn insert_opt_string(metadata: &mut BTreeMap<String, Value>, key: &str, value: Option<&String>) {
    if let Some(value) = value {
        metadata.insert(key.to_string(), json!(value));
    }
}

fn insert_opt_u64(metadata: &mut BTreeMap<String, Value>, key: &str, value: Option<u64>) {
    if let Some(value) = value {
        metadata.insert(key.to_string(), json!(value));
    }
}

fn insert_opt_f64(metadata: &mut BTreeMap<String, Value>, key: &str, value: Option<f64>) {
    if let Some(value) = value {
        metadata.insert(key.to_string(), json!(value));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buchi_targets_preserve_zero_when_property_table_has_values() {
        let names = vec!["zero".to_string(), "positive".to_string()];

        let targets = property_targets_from_values(&names, &[0.0, 1.5], false);
        assert_eq!(targets["zero"].as_f64(), Some(0.0));
        assert_eq!(targets["positive"].as_f64(), Some(1.5));

        let missing = property_targets_from_values(&names, &[0.0, f64::NAN], true);
        assert!(missing["zero"].is_null());
        assert!(missing["positive"].is_null());
    }
}
