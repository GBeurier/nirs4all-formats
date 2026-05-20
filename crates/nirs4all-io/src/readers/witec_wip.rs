use std::path::Path;

use nirs4all_io_core::{Confidence, Error, FormatProbe, Result, SpectralRecord};

use crate::Reader;

const FORMAT: &str = "witec-wip";

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
        if head.starts_with(b"WIT^") {
            return Some(FormatProbe::new(
                FORMAT,
                self.name(),
                Confidence::Definite,
                "WiTec WIP/WID binary project container; native parser pending",
            ));
        }
        None
    }

    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>> {
        Err(Error::InvalidRecord(format!(
            "WiTec WIP/WID binary project files are not supported yet in nirs4all-io: {}. Export spectra from WiTec Project/FIVE as ASCII text and load the .txt export; WiTec ASCII exports are covered by the row-spectral-table reader.",
            path.display()
        )))
    }
}
