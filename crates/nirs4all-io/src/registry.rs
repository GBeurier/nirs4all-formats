use std::path::Path;
use std::sync::Arc;

use nirs4all_io_core::{Confidence, Error, FormatProbe, Result, SidecarResolver, SpectralRecord};

#[cfg(feature = "fmt-matlab")]
use crate::readers::MatlabReader;
#[cfg(feature = "fmt-parquet")]
use crate::readers::ParquetReader;
#[cfg(feature = "fmt-hdf5")]
use crate::readers::{AllotropeAdfReader, FgiXmlReader, Hdf5Reader, NetcdfReader};
use crate::readers::{
    AllotropeAsmReader, AnimlReader, AsdReader, AvantesAsciiReader, AvantesBinaryReader,
    BrukerDptReader, BrukerOpusReader, BuchiNircalReader, CsvLikeReader, DigitalSurfReader,
    EnviSliReader, ErdasLanReader, ExcelReader, GalacticSpcReader, HamamatsuImgReader,
    HoribaLabSpecReader, JascoJwsReader, JcampReader, MsaReader, MzmlReader, NicoletOmnicReader,
    NumpyReader, OceanOpticsReader, PerkinElmerReader, PpSystemsReader, RenishawWdfReader,
    ScioCsvReader, SedReader, SiwareApiReader, SpectralMatrixReader, SpectralTableReader,
    SunPhotometerReader, SvcSigReader, TrivistaTvfReader, UsgsArefReader, WitecWipReader,
};
use crate::sidecars::NoSidecars;

/// Contract implemented by every native reader.
pub trait Reader: Send + Sync {
    fn name(&self) -> &'static str;
    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe>;

    /// Sniff a primary file with access to companion sidecars.
    ///
    /// Default implementation forwards to [`Reader::sniff`]. Readers
    /// whose detection depends on a companion file the primary alone
    /// cannot disambiguate override this — currently only ENVI SLI /
    /// ENVI Standard, because a `.sli`/`.img`/`.dat` primary needs the
    /// `.hdr` header to read the `file type` line. Other sidecar-bearing
    /// readers (FGI XML+HDF5, MATLAB Indian Pines, ARM MFRSR NetCDF)
    /// sniff from the primary's head bytes alone and inherit the
    /// default.
    fn sniff_with_sidecars(
        &self,
        head: &[u8],
        path: &Path,
        sidecars: &Arc<dyn SidecarResolver>,
    ) -> Option<FormatProbe> {
        let _ = sidecars;
        self.sniff(head, path)
    }

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

    /// Decode an in-memory byte slice. `name` is the input file name and is
    /// used for sniffing and provenance. The default implementation declares
    /// the reader path-only; readers should override this whenever their
    /// decoder is filesystem-free, both for performance and to support
    /// no-fs targets like wasm32-unknown-unknown.
    fn read_bytes(&self, name: &Path, _bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
        let _ = name;
        Err(Error::InvalidRecord(format!(
            "{} does not support in-memory reads (needs filesystem sidecars)",
            self.name()
        )))
    }

    /// Bytes-based counterpart of `read_path_with_options`.
    fn read_bytes_with_options(
        &self,
        name: &Path,
        bytes: &[u8],
        options: &ReadOptions,
    ) -> Result<Vec<SpectralRecord>> {
        if options.has_reader_options() {
            return Err(Error::InvalidRecord(format!(
                "{} does not support read options",
                self.name()
            )));
        }
        self.read_bytes(name, bytes)
    }

    /// Decode an in-memory primary payload while resolving sidecar files
    /// through the supplied [`SidecarResolver`].
    ///
    /// The default implementation ignores the resolver and delegates to
    /// [`Reader::read_bytes_with_options`], which means single-file readers
    /// automatically work with any resolver. Sidecar-bearing readers
    /// override this method.
    fn read_bytes_with_sidecars(
        &self,
        name: &Path,
        bytes: &[u8],
        sidecars: &Arc<dyn SidecarResolver>,
        options: &ReadOptions,
    ) -> Result<Vec<SpectralRecord>> {
        let _ = sidecars;
        self.read_bytes_with_options(name, bytes, options)
    }
}

/// Optional read controls for formats where loading every record is expensive.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReadOptions {
    pub cube_selection: Option<CubeSelection>,
}

impl ReadOptions {
    pub fn with_cube_window(mut self, window: CubeWindow) -> Self {
        self.cube_selection = Some(CubeSelection::Window(window));
        self
    }

    pub fn with_cube_mask(mut self, mask: CubeMask) -> Self {
        self.cube_selection = Some(CubeSelection::Mask(mask));
        self
    }

    pub(crate) fn has_reader_options(&self) -> bool {
        self.cube_selection.is_some()
    }
}

/// Pixel selection for image cubes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CubeSelection {
    /// Rectangular half-open window.
    Window(CubeWindow),
    /// Arbitrary sparse list of `(row, col)` pixels.
    Mask(CubeMask),
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

/// Sparse pixel mask for image cubes. Pixels are emitted in the order given.
///
/// Duplicates are preserved so callers can describe ordered sample paths.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CubeMask {
    pub pixels: Vec<(usize, usize)>,
}

impl CubeMask {
    pub fn new(pixels: Vec<(usize, usize)>) -> Self {
        Self { pixels }
    }
}

pub(crate) fn cube_pixels(
    options: &ReadOptions,
    rows: usize,
    cols: usize,
    context: &str,
) -> Result<Vec<(usize, usize)>> {
    let Some(selection) = &options.cube_selection else {
        return Ok(enumerate_window(0..rows, 0..cols));
    };
    match selection {
        CubeSelection::Window(window) => {
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
            Ok(enumerate_window(
                window.row_start..row_end,
                window.col_start..col_end,
            ))
        }
        CubeSelection::Mask(mask) => {
            if mask.pixels.is_empty() {
                return Err(Error::InvalidRecord(format!("{context} mask is empty")));
            }
            for &(row, col) in &mask.pixels {
                if row >= rows || col >= cols {
                    return Err(Error::InvalidRecord(format!(
                        "{context} mask pixel ({row}, {col}) is outside 0..{rows} x 0..{cols}"
                    )));
                }
            }
            Ok(mask.pixels.clone())
        }
    }
}

fn enumerate_window(
    rows: std::ops::Range<usize>,
    cols: std::ops::Range<usize>,
) -> Vec<(usize, usize)> {
    let mut out = Vec::with_capacity(rows.len() * cols.len());
    for row in rows {
        for col in cols.clone() {
            out.push((row, col));
        }
    }
    out
}

fn readers() -> Vec<Box<dyn Reader>> {
    let mut readers: Vec<Box<dyn Reader>> = vec![
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
    ];
    #[cfg(feature = "fmt-hdf5")]
    {
        readers.push(Box::new(NetcdfReader));
        readers.push(Box::new(FgiXmlReader));
        readers.push(Box::new(AllotropeAdfReader));
        readers.push(Box::new(Hdf5Reader));
    }
    #[cfg(feature = "fmt-matlab")]
    readers.push(Box::new(MatlabReader));
    readers.push(Box::new(NumpyReader));
    #[cfg(feature = "fmt-parquet")]
    readers.push(Box::new(ParquetReader));
    readers.extend([
        Box::new(AnimlReader) as Box<dyn Reader>,
        Box::new(SiwareApiReader),
        Box::new(AllotropeAsmReader),
        Box::new(PpSystemsReader),
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
    ]);
    readers
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

/// Open an in-memory byte slice through the native registry. `name` is the
/// input file name, used for sniffing and provenance only.
///
/// Sidecar-bearing formats (ENVI Standard, AVIRIS LAN, FGI HDF5+XML, MATLAB
/// Indian Pines, NetCDF MFRSR-with-QC) error here with
/// [`Error::UnsupportedSidecar`]; use [`open_with_sidecars`] to decode them
/// without a filesystem.
pub fn open_bytes(name: impl AsRef<Path>, bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
    open_bytes_with_options(name, bytes, &ReadOptions::default())
}

/// Open an in-memory byte slice with read options.
///
/// Sidecar-bearing formats error with [`Error::UnsupportedSidecar`]; use
/// [`open_with_sidecars_and_options`] if you need them off the filesystem.
pub fn open_bytes_with_options(
    name: impl AsRef<Path>,
    bytes: &[u8],
    options: &ReadOptions,
) -> Result<Vec<SpectralRecord>> {
    let sidecars: Arc<dyn SidecarResolver> = Arc::new(NoSidecars);
    open_with_sidecars_and_options(name, bytes, sidecars, options)
}

/// Open an in-memory byte slice using `sidecars` for companion files.
pub fn open_with_sidecars(
    name: impl AsRef<Path>,
    bytes: &[u8],
    sidecars: Arc<dyn SidecarResolver>,
) -> Result<Vec<SpectralRecord>> {
    open_with_sidecars_and_options(name, bytes, sidecars, &ReadOptions::default())
}

/// Open an in-memory byte slice using `sidecars` and `options`.
pub fn open_with_sidecars_and_options(
    name: impl AsRef<Path>,
    bytes: &[u8],
    sidecars: Arc<dyn SidecarResolver>,
    options: &ReadOptions,
) -> Result<Vec<SpectralRecord>> {
    let name_ref = name.as_ref();
    let head = &bytes[..bytes.len().min(8192)];
    let mut candidates: Vec<(FormatProbe, Box<dyn Reader>)> = readers()
        .into_iter()
        .filter_map(|reader| {
            reader
                .sniff_with_sidecars(head, name_ref, &sidecars)
                .map(|probe| (probe, reader))
        })
        .collect();
    candidates.sort_by(|a, b| {
        b.0.confidence
            .cmp(&a.0.confidence)
            .then(a.0.format.cmp(&b.0.format))
    });
    let Some((_, reader)) = candidates.into_iter().next() else {
        return Err(Error::UnsupportedFormat {
            path: name_ref.to_path_buf(),
        });
    };
    reader.read_bytes_with_sidecars(name_ref, bytes, &sidecars, options)
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
