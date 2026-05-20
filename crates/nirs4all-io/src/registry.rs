use std::ops::Range;
use std::path::Path;

use nirs4all_io_core::{Confidence, Error, FormatProbe, Result, SpectralRecord};

use crate::readers::{
    AllotropeAdfReader, AllotropeAsmReader, AnimlReader, AsdReader, AvantesAsciiReader,
    AvantesBinaryReader, BrukerDptReader, BrukerOpusReader, BuchiNircalReader, CsvLikeReader,
    DigitalSurfReader, EnviSliReader, ErdasLanReader, ExcelReader, FgiXmlReader, GalacticSpcReader,
    HamamatsuImgReader, Hdf5Reader, HoribaLabSpecReader, JascoJwsReader, JcampReader, MatlabReader,
    MsaReader, MzmlReader, NetcdfReader, NicoletOmnicReader, NumpyReader, OceanOpticsReader,
    ParquetReader, PerkinElmerReader, RenishawWdfReader, ScioCsvReader, SedReader, SiwareApiReader,
    SpectralMatrixReader, SpectralTableReader, SunPhotometerReader, SvcSigReader,
    TrivistaTvfReader, UsgsArefReader, WitecWipReader,
};

/// Contract implemented by every native reader.
pub trait Reader: Send + Sync {
    fn name(&self) -> &'static str;
    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe>;
    fn read_path(&self, path: &Path) -> Result<Vec<SpectralRecord>>;
    fn read_path_with_options(
        &self,
        path: &Path,
        options: &ReadOptions,
    ) -> Result<Vec<SpectralRecord>> {
        if options.has_reader_options() {
            return Err(Error::InvalidRecord(format!(
                "{} does not support read options",
                self.name()
            )));
        }
        self.read_path(path)
    }
}

/// Optional read controls for formats where loading every record is expensive.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReadOptions {
    pub cube_window: Option<CubeWindow>,
}

impl ReadOptions {
    pub fn with_cube_window(mut self, window: CubeWindow) -> Self {
        self.cube_window = Some(window);
        self
    }

    pub(crate) fn has_reader_options(&self) -> bool {
        self.cube_window.is_some()
    }
}

/// Half-open pixel window for image cubes: rows `[start, end)` and columns
/// `[start, end)`. Missing ends default to the cube dimensions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CubeWindow {
    pub row_start: usize,
    pub row_end: Option<usize>,
    pub col_start: usize,
    pub col_end: Option<usize>,
}

impl CubeWindow {
    pub fn new(
        row_start: usize,
        row_end: Option<usize>,
        col_start: usize,
        col_end: Option<usize>,
    ) -> Self {
        Self {
            row_start,
            row_end,
            col_start,
            col_end,
        }
    }
}

pub(crate) fn cube_window_ranges(
    options: &ReadOptions,
    rows: usize,
    cols: usize,
    context: &str,
) -> Result<(Range<usize>, Range<usize>)> {
    let Some(window) = &options.cube_window else {
        return Ok((0..rows, 0..cols));
    };
    let row_end = window.row_end.unwrap_or(rows);
    let col_end = window.col_end.unwrap_or(cols);
    if window.row_start >= row_end || row_end > rows {
        return Err(Error::InvalidRecord(format!(
            "{context} row window {}..{} is outside 0..{rows}",
            window.row_start, row_end
        )));
    }
    if window.col_start >= col_end || col_end > cols {
        return Err(Error::InvalidRecord(format!(
            "{context} column window {}..{} is outside 0..{cols}",
            window.col_start, col_end
        )));
    }
    Ok((window.row_start..row_end, window.col_start..col_end))
}

fn readers() -> Vec<Box<dyn Reader>> {
    vec![
        Box::new(JcampReader),
        Box::new(BrukerOpusReader),
        Box::new(NicoletOmnicReader),
        Box::new(PerkinElmerReader),
        Box::new(BuchiNircalReader),
        Box::new(JascoJwsReader),
        Box::new(HoribaLabSpecReader),
        Box::new(RenishawWdfReader),
        Box::new(TrivistaTvfReader),
        Box::new(DigitalSurfReader),
        Box::new(ErdasLanReader),
        Box::new(HamamatsuImgReader),
        Box::new(WitecWipReader),
        Box::new(GalacticSpcReader),
        Box::new(EnviSliReader),
        Box::new(AsdReader),
        Box::new(AvantesBinaryReader),
        Box::new(OceanOpticsReader),
        Box::new(MsaReader),
        Box::new(MzmlReader),
        Box::new(NetcdfReader),
        Box::new(FgiXmlReader),
        Box::new(AllotropeAdfReader),
        Box::new(Hdf5Reader),
        Box::new(MatlabReader),
        Box::new(NumpyReader),
        Box::new(ParquetReader),
        Box::new(AnimlReader),
        Box::new(SiwareApiReader),
        Box::new(AllotropeAsmReader),
        Box::new(ExcelReader),
        Box::new(ScioCsvReader),
        Box::new(UsgsArefReader),
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
    open_path_with_options(path, &ReadOptions::default())
}

/// Open a file through the native registry with optional format-specific read controls.
pub fn open_path_with_options(
    path: impl AsRef<Path>,
    options: &ReadOptions,
) -> Result<Vec<SpectralRecord>> {
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
    reader.read_path_with_options(path_ref, options)
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

    if matches!(
        ext.as_str(),
        "asd" | "hdr" | "spc" | "spa" | "spg" | "srs" | "srsx"
    ) && out.is_empty()
    {
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

    #[test]
    fn marks_srsx_extension_as_possible_without_magic() {
        let probes = builtin_probes(b"not enough bytes", Path::new("sample.srsx"));
        assert_eq!(probes[0].format, "candidate-srsx");
        assert_eq!(probes[0].confidence, Confidence::Possible);
    }
}
