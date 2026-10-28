//! Raman Open Database CIF-like single-spectrum records.
use crate::readers::util::{
    read_bytes, single_signal_record, text_lossy_from_bytes, SingleSignalSpec,
};
use crate::Reader;
use nirs4all_formats_core::{
    AxisKind, Confidence, Error, FormatProbe, Result, SignalType, SpectralRecord,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;

pub struct RodReader;
impl Reader for RodReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::rod"
    }
    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        (path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("rod"))
            && String::from_utf8_lossy(head).contains("data_"))
        .then(|| {
            FormatProbe::new(
                "raman-open-database",
                self.name(),
                Confidence::Likely,
                "Raman Open Database record",
            )
        })
    }
    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        self.read_bytes(path, &read_bytes(path)?)
    }
    fn read_bytes(&self, path: &Path, bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
        let (text, source) = text_lossy_from_bytes(path, bytes);
        let mut metadata = BTreeMap::new();
        let mut x = Vec::new();
        let mut y = Vec::new();
        let lines: Vec<_> = text.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim();
            if line == "loop_" {
                i += 1;
                let mut columns = Vec::new();
                while i < lines.len() && lines[i].trim().starts_with('_') {
                    columns.push(lines[i].trim());
                    i += 1;
                }
                let sx = columns
                    .iter()
                    .position(|c| *c == "_raman_spectrum.raman_shift");
                let sy = columns
                    .iter()
                    .position(|c| *c == "_raman_spectrum.intensity");
                if let (Some(sx), Some(sy)) = (sx, sy) {
                    while i < lines.len() {
                        let line = lines[i].trim();
                        if line.is_empty() || line.starts_with('#') {
                            i += 1;
                            continue;
                        }
                        if line.starts_with('_') || line == "loop_" || line.starts_with("data_") {
                            break;
                        }
                        let fields: Vec<_> = line.split_whitespace().collect();
                        if fields.len() != columns.len() {
                            return Err(Error::InvalidRecord(
                                "ROD spectrum row has wrong number of fields".into(),
                            ));
                        }
                        x.push(
                            fields[sx].parse::<f64>().map_err(|_| {
                                Error::InvalidRecord("invalid ROD Raman shift".into())
                            })?,
                        );
                        y.push(
                            fields[sy].parse::<f64>().map_err(|_| {
                                Error::InvalidRecord("invalid ROD intensity".into())
                            })?,
                        );
                        i += 1;
                    }
                    continue;
                }
                continue;
            }
            if line.starts_with('_') {
                let mut parts = line.splitn(2, char::is_whitespace);
                let key = parts.next().unwrap_or_default();
                let mut value = parts.next().unwrap_or_default().trim().to_string();
                if value.is_empty() && i + 1 < lines.len() {
                    let next = lines[i + 1];
                    if next.starts_with(';') {
                        i += 2;
                        let mut pieces = Vec::new();
                        while i < lines.len() && !lines[i].starts_with(';') {
                            pieces.push(lines[i].trim());
                            i += 1;
                        }
                        if i == lines.len() {
                            return Err(Error::InvalidRecord("unterminated ROD text value".into()));
                        }
                        value = pieces.join(" ");
                    } else if !next.trim().starts_with('_') && next.trim() != "loop_" {
                        i += 1;
                        value = next.trim().to_string();
                    }
                }
                let value = value.trim().trim_matches('\'').trim_matches('"');
                if !value.is_empty() && value != "?" && value != "." {
                    metadata.insert(key.trim_start_matches('_').replace('.', "_"), json!(value));
                }
            }
            i += 1;
        }
        if x.is_empty() {
            return Err(Error::InvalidRecord(
                "ROD record has no Raman spectrum loop".into(),
            ));
        }
        let record = single_signal_record(
            "raman-open-database",
            self.name(),
            source,
            SingleSignalSpec {
                axis_values: x,
                axis_unit: "cm-1".into(),
                axis_kind: AxisKind::Wavenumber,
                values: y,
                signal_name: "intensity".into(),
                signal_type: SignalType::Unknown,
                signal_unit: None,
                role: "intensity".into(),
            },
            BTreeMap::new(),
            metadata,
            vec![],
        )?;
        Ok(vec![record])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_axis_values_and_metadata_from_rod_loop() {
        let input = b"data_3500102\n_chemical_name_mineral Corundum\n_publ_section_title\n;\nA title\n;\nloop_\n_raman_spectrum.raman_shift\n_raman_spectrum.intensity\n50.5 10\n51.5 20\nloop_\n_other.field\na\n";
        let records = RodReader
            .read_bytes(Path::new("3500102.rod"), input)
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].metadata["chemical_name_mineral"],
            json!("Corundum")
        );
        assert_eq!(records[0].metadata["publ_section_title"], json!("A title"));
        assert_eq!(
            records[0].signals["intensity"].axis.values,
            vec![50.5, 51.5]
        );
        assert_eq!(records[0].signals["intensity"].values, vec![10.0, 20.0]);
    }
}
