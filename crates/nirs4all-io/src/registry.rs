use std::path::Path;

use nirs4all_io_core::{Confidence, Error, FormatProbe, Result, SpectralRecord};

/// Contract implemented by every native reader.
pub trait Reader: Send + Sync {
    fn name(&self) -> &'static str;
    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe>;
    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>>;
}

/// Probe a file and return every positive candidate ordered by confidence.
pub fn probe_path(path: impl AsRef<Path>) -> Result<Vec<FormatProbe>> {
    let path_ref = path.as_ref();
    let bytes = std::fs::read(path_ref).map_err(|source| Error::Io {
        path: path_ref.to_path_buf(),
        source,
    })?;
    let head = &bytes[..bytes.len().min(8192)];
    let mut probes = builtin_probes(head, path_ref);
    probes.sort_by(|a, b| {
        b.confidence
            .cmp(&a.confidence)
            .then(a.format.cmp(&b.format))
    });
    Ok(probes)
}

/// Open a file through the native registry.
///
/// Full parsing is intentionally not guessed here. The first production readers
/// will land one format at a time with conformance gates.
pub fn open_path(path: impl AsRef<Path>) -> Result<Vec<SpectralRecord>> {
    let path_ref = path.as_ref();
    let probes = probe_path(path_ref)?;
    let best = probes.first().ok_or_else(|| Error::UnsupportedFormat {
        path: path_ref.to_path_buf(),
    })?;
    Err(Error::UnsupportedFormat {
        path: path_ref
            .with_extension(format!("{}-parser-pending", best.format))
            .to_path_buf(),
    })
}

/// Built-in format sniffers used before full reader modules exist.
pub fn builtin_probes(head: &[u8], path: &Path) -> Vec<FormatProbe> {
    let mut out = Vec::new();
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let text_head = String::from_utf8_lossy(head);
    let trimmed = text_head.trim_start_matches('\u{feff}').trim_start();

    if trimmed.starts_with("##TITLE=") || text_head.contains("##JCAMP-DX=") {
        out.push(FormatProbe::new(
            "jcamp-dx",
            "nirs4all_io::readers::jcamp",
            Confidence::Definite,
            "JCAMP-DX labeled-data records detected",
        ));
    }

    if trimmed.starts_with("ENVI") && ext == "hdr" {
        out.push(FormatProbe::new(
            "envi-header",
            "nirs4all_io::readers::envi",
            Confidence::Definite,
            "ENVI header magic detected",
        ));
    }

    if matches!(ext.as_str(), "csv" | "tsv" | "txt") && looks_tabular(trimmed) {
        out.push(FormatProbe::new(
            "delimited-text",
            "nirs4all_io::readers::csv_like",
            Confidence::Likely,
            "text extension with delimited numeric header",
        ));
    }

    if matches!(
        ext.as_str(),
        "asd" | "sig" | "sed" | "spc" | "spa" | "spg" | "srs"
    ) {
        out.push(FormatProbe::new(
            format!("candidate-{ext}"),
            "nirs4all_io::registry",
            Confidence::Possible,
            "extension known but no definite magic matched yet",
        ));
    }

    out
}

fn looks_tabular(text: &str) -> bool {
    text.lines().take(5).any(|line| {
        let commas = line.matches(',').count();
        let semicolons = line.matches(';').count();
        let tabs = line.matches('\t').count();
        commas.max(semicolons).max(tabs) >= 2
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probes_jcamp_before_extension() {
        let probes = builtin_probes(b"##TITLE=Water\n##JCAMP-DX=5.01\n", Path::new("x.txt"));
        assert_eq!(probes[0].format, "jcamp-dx");
        assert_eq!(probes[0].confidence, Confidence::Definite);
    }

    #[test]
    fn marks_known_collision_extension_as_possible() {
        let probes = builtin_probes(b"not enough bytes", Path::new("sample.spc"));
        assert_eq!(probes[0].confidence, Confidence::Possible);
    }
}
