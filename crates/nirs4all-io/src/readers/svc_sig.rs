use std::collections::BTreeMap;
use std::path::Path;

use nirs4all_io_core::{
    AxisKind, Confidence, FormatProbe, Result, SignalType, SpectralArray, SpectralAxis,
};

use crate::readers::util::{
    metadata_from_pairs, parse_number, read_text_lossy, record_from_signals,
};
use crate::Reader;

pub struct SvcSigReader;

impl Reader for SvcSigReader {
    fn name(&self) -> &'static str {
        "nirs4all_io::readers::svc_sig"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        let text = String::from_utf8_lossy(head);
        (ext == "sig" && text.contains("Spectra Vista SIG Data")).then(|| {
            FormatProbe::new(
                "svc-ger-sig",
                self.name(),
                Confidence::Definite,
                "Spectra Vista SIG magic detected",
            )
        })
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_io_core::SpectralRecord>> {
        let (text, source) = read_text_lossy(path)?;
        let lines: Vec<&str> = text.lines().collect();
        let data_idx = lines
            .iter()
            .position(|line| line.trim().eq_ignore_ascii_case("data="))
            .ok_or_else(|| {
                nirs4all_io_core::Error::InvalidRecord("SIG missing data=".to_string())
            })?;
        let mut metadata_pairs = Vec::new();
        let mut is_moc = false;
        for line in &lines[..data_idx] {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim().eq_ignore_ascii_case("comm")
                    && value.to_ascii_lowercase().contains("overlap")
                {
                    is_moc = true;
                }
                metadata_pairs.push((key.to_string(), value.trim().to_string()));
            }
        }
        let mut axis = Vec::new();
        let mut reference = Vec::new();
        let mut target = Vec::new();
        let mut reflectance = Vec::new();
        for line in lines.iter().skip(data_idx + 1) {
            let numbers: Vec<f64> = line.split_whitespace().filter_map(parse_number).collect();
            if numbers.len() >= 4 {
                axis.push(numbers[0]);
                reference.push(numbers[1]);
                target.push(numbers[2]);
                reflectance.push(numbers[3]);
            }
        }
        let mut signals = BTreeMap::new();
        for (name, values, signal_type, unit) in [
            ("reference", reference, SignalType::Radiance, None),
            ("target", target, SignalType::Radiance, None),
            (
                "reflectance",
                reflectance,
                SignalType::Reflectance,
                Some("%".to_string()),
            ),
        ] {
            let signal = SpectralArray::new(
                SpectralAxis::new(axis.clone(), "nm", AxisKind::Wavelength)?,
                values,
                vec!["x".to_string()],
                signal_type,
                unit,
                name,
                "file",
            )?;
            signals.insert(name.to_string(), signal);
        }
        let mut record = record_from_signals(
            "svc-ger-sig",
            self.name(),
            source,
            signals,
            SignalType::Reflectance,
            metadata_from_pairs(metadata_pairs),
            Vec::new(),
        )?;
        if is_moc {
            record
                .quality_flags
                .push("matched_overlap_corrected".to_string());
        }
        Ok(vec![record])
    }
}
