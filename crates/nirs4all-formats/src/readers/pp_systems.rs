use std::path::Path;

use nirs4all_formats_core::{Confidence, Error, FormatProbe, Result};

use crate::readers::spectral_table::{read_spectral_table_bytes, spectral_table_can_parse_text};
use crate::readers::util::read_bytes;
use crate::Reader;

pub struct PpSystemsReader;

impl Reader for PpSystemsReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::pp_systems"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        if is_arc_lter_indices_product(head, path) {
            return Some(FormatProbe::new(
                "pp-systems-unispec-derived-indices",
                self.name(),
                Confidence::Definite,
                "Arctic LTER UniSpec vegetation-index product detected; refused as non-spectral",
            ));
        }
        if is_raw_unispec_export(head, path) {
            return Some(FormatProbe::new(
                "pp-systems-unispec",
                self.name(),
                Confidence::Definite,
                "PP Systems UniSpec raw spectral text export",
            ));
        }
        None
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
        if is_arc_lter_indices_product(bytes, path) {
            return reject_indices_product(path);
        }
        if is_raw_unispec_export(bytes, path) {
            return read_spectral_table_bytes(
                path,
                bytes,
                "pp-systems-unispec",
                self.name(),
                vec!["pp_systems_unispec_text_export".to_string()],
            );
        }
        Err(Error::InvalidRecord(format!(
            "{} is not a recognized PP Systems UniSpec spectral export",
            path.display()
        )))
    }
}

fn reject_indices_product(path: &Path) -> Result<Vec<nirs4all_formats_core::SpectralRecord>> {
    Err(Error::InvalidRecord(format!(
        "{} is a PP Systems UniSpec Arctic LTER derived vegetation-index product \
         (NDVI/EVI/PRI/WBI/Chl/LAI), not wavelength-indexed spectra; provide raw \
         .SPT/.SPU files or the referenced reflectance data scan table with wavelength columns",
        path.display()
    )))
}

fn is_raw_unispec_export(head: &[u8], path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "spt" | "spu") {
        return false;
    }

    let text = String::from_utf8_lossy(head);
    if !spectral_table_can_parse_text(&text, path) {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    lower.contains("wavelength")
        && (lower.contains("reflectance")
            || lower.contains("dn_white")
            || lower.contains("dn_target")
            || lower.contains("channel_a_dn")
            || lower.contains("channel_b_dn"))
}

fn is_arc_lter_indices_product(head: &[u8], path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "csv" | "xlsx") {
        return false;
    }
    if is_arc_lter_indices_filename(path) {
        return true;
    }
    ext == "csv" && is_arc_lter_indices_header(head)
}

fn is_arc_lter_indices_filename(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        file_name.as_str(),
        "arc_lter_unispec_dc_2007_2019_indices.csv" | "arc_lter_unispec_dc_2007_2019_indices.xlsx"
    )
}

fn is_arc_lter_indices_header(head: &[u8]) -> bool {
    let text = String::from_utf8_lossy(head).to_ascii_lowercase();
    text.contains("scan id,year,date,doy")
        && text.contains("ndvi (modis)")
        && text.contains("evi (modis)")
        && text.contains("pri (550 reference)")
        && text.contains("wbi")
        && text.contains("chl index")
        && text.contains("lai")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_arc_lter_indices_csv_header() {
        let head = b"SCAN ID,YEAR,DATE,DOY,DegDay,TIME,SITE,EXPERIMENT,BLOCK,TREATMENT,REP,NDVI (MODIS),EVI (MODIS),EVI2 (MODIS),PRI (550 Reference),PRI (570 Ref),WBI,Chl Index,LAI\n";

        assert!(is_arc_lter_indices_product(
            head,
            Path::new("toolik_indices.csv")
        ));
    }

    #[test]
    fn detects_raw_unispec_extensions() {
        let head =
            b"Wavelength,Channel_A_DN,Channel_B_DN,Reflectance\n1100,1018,804,1.2646\n1101,1020,806,1.2700\n";

        assert!(is_raw_unispec_export(
            head,
            Path::new("synthetic_unispec_dc.SPU")
        ));
    }
}
