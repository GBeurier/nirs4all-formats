use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_formats_core::{AxisKind, Confidence, FormatProbe, Result, SignalType};

use crate::readers::util::{
    parse_number, read_bytes, single_signal_record, text_lossy_from_bytes, SingleSignalSpec,
};
use crate::Reader;

pub struct BrukerDptReader;

impl Reader for BrukerDptReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::bruker_dpt"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        if ext != "dpt" {
            return None;
        }
        let text = String::from_utf8_lossy(head);
        let numeric_rows = text
            .lines()
            .take(20)
            .filter(|line| parse_pair(line).is_some())
            .count();
        (numeric_rows >= 3).then(|| {
            FormatProbe::new(
                "bruker-dpt",
                self.name(),
                Confidence::Likely,
                "two-column OPUS DPT export",
            )
        })
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
        let bytes = read_bytes(path)?;
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(
        &self,
        path: &Path,
        bytes: &[u8],
    ) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
        let (text, source) = text_lossy_from_bytes(path, bytes);
        let mut axis = Vec::new();
        let mut values = Vec::new();
        for line in text.lines() {
            if let Some((x, y)) = parse_pair(line) {
                axis.push(x);
                values.push(y);
            }
        }
        let record = single_signal_record(
            "bruker-dpt",
            self.name(),
            source,
            SingleSignalSpec {
                axis_values: axis,
                axis_unit: "cm-1".to_string(),
                axis_kind: AxisKind::Wavenumber,
                values,
                signal_name: "absorbance".to_string(),
                signal_type: SignalType::Absorbance,
                signal_unit: None,
                role: "absorbance".to_string(),
            },
            BTreeMap::new(),
            BTreeMap::new(),
            Vec::new(),
        )?;
        Ok(vec![record])
    }
}

fn parse_pair(line: &str) -> Option<(f64, f64)> {
    let normalized = line.replace(',', " ");
    let mut parts = normalized.split_whitespace();
    let x = parse_number(parts.next()?)?;
    let y = parse_number(parts.next()?)?;
    Some((x, y))
}
