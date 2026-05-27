use std::path::Path;

use nirs4all_formats_core::{Confidence, Error, FormatProbe, Result};

use crate::Reader;

const FORMAT: &str = "mzml-ms";

pub struct MzmlReader;

impl Reader for MzmlReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::mzml"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "mzml" | "mzmlb") {
            return None;
        }
        let text = String::from_utf8_lossy(head);
        if text.contains("<mzML") || text.contains("<indexedmzML") {
            return Some(FormatProbe::new(
                FORMAT,
                self.name(),
                Confidence::Definite,
                "HUPO PSI mzML mass-spectrometry container",
            ));
        }
        None
    }

    fn read_path(&self, path: &Path) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        self.read_bytes(path, &bytes)
    }

    fn read_bytes(
        &self,
        path: &Path,
        bytes: &[u8],
    ) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(16_384)]);
        let spectra = head.matches("<spectrum ").count();
        let chromatograms = head.matches("<chromatogram ").count();
        Err(Error::InvalidRecord(format!(
            "mzML is mass-spectrometry data, not NIRS spectroscopy; detected at least {spectra} spectrum elements and {chromatograms} chromatogram elements in {}. Use pyteomics, pymzML or pyOpenMS for mzML, or convert an intentional optical spectrum to a supported NIRS table format.",
            path.display()
        )))
    }
}
