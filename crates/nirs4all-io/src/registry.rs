use std::path::Path;

use nirs4all_io_core::{Confidence, Error, FormatProbe, Result, SpectralRecord};

use crate::readers::{
    AllotropeAsmReader, AnimlReader, AsdReader, AvantesAsciiReader, AvantesBinaryReader,
    BrukerDptReader, BrukerOpusReader, CsvLikeReader, EnviSliReader, GalacticSpcReader, Hdf5Reader,
    JcampReader, MsaReader, NetcdfReader, OceanOpticsReader, SedReader, SiwareApiReader,
    SpectralMatrixReader, SpectralTableReader, SunPhotometerReader, SvcSigReader,
};

/// Contract implemented by every native reader.
pub trait Reader: Send + Sync {
    fn name(&self) -> &'static str;
    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe>;
    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>>;
}

fn readers() -> Vec<Box<dyn Reader>> {
    vec![
        Box::new(JcampReader),
        Box::new(BrukerOpusReader),
        Box::new(GalacticSpcReader),
        Box::new(EnviSliReader),
        Box::new(AsdReader),
        Box::new(AvantesBinaryReader),
        Box::new(OceanOpticsReader),
        Box::new(MsaReader),
        Box::new(NetcdfReader),
        Box::new(Hdf5Reader),
        Box::new(AnimlReader),
        Box::new(SiwareApiReader),
        Box::new(AllotropeAsmReader),
        Box::new(SpectralTableReader),
        Box::new(SpectralMatrixReader),
        Box::new(SunPhotometerReader),
        Box::new(CsvLikeReader),
        Box::new(BrukerDptReader),
        Box::new(AvantesAsciiReader),
        Box::new(SedReader),
        Box::new(SvcSigReader),
    ]
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
pub fn open_path(path: impl AsRef<Path>) -> Result<Vec<SpectralRecord>> {
    let path_ref = path.as_ref();
    let bytes = std::fs::read(path_ref).map_err(|source| Error::Io {
        path: path_ref.to_path_buf(),
        source,
    })?;
    let head = &bytes[..bytes.len().min(8192)];
    let mut candidates: Vec<(FormatProbe, Box<dyn Reader>)> = readers()
        .into_iter()
        .filter_map(|reader| reader.sniff(head, path_ref).map(|probe| (probe, reader)))
        .collect();
    candidates.sort_by(|a, b| {
        b.0.confidence
            .cmp(&a.0.confidence)
            .then(a.0.format.cmp(&b.0.format))
    });
    let Some((_, reader)) = candidates.into_iter().next() else {
        return Err(Error::UnsupportedFormat {
            path: path_ref.to_path_buf(),
        });
    };
    reader.read_path(path_ref)
}

/// Built-in format sniffers backed by the native readers.
pub fn builtin_probes(head: &[u8], path: &Path) -> Vec<FormatProbe> {
    let mut out: Vec<FormatProbe> = readers()
        .iter()
        .filter_map(|reader| reader.sniff(head, path))
        .collect();
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if matches!(ext.as_str(), "asd" | "hdr" | "spc" | "spa" | "spg" | "srs") && out.is_empty() {
        out.push(FormatProbe::new(
            format!("candidate-{ext}"),
            "nirs4all_io::registry",
            Confidence::Possible,
            "extension known but no definite magic matched yet",
        ));
    }

    out
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
