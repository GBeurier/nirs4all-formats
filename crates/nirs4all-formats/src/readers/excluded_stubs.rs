//! Graceful-degradation stubs for readers excluded from a feature-trimmed build.
//!
//! The heavy readers (`fmt-hdf5`, `fmt-matlab`, `fmt-parquet`) pull in large
//! crates.io closures (Apache Arrow / Parquet, pure-Rust HDF5 / NetCDF, the
//! MATLAB + xz codecs). A size-constrained distribution may drop some of them —
//! notably the smaller `nirs4allformats.lite` R package, which compiles the
//! facade with `default-features = false, features = ["fmt-hdf5", "fmt-matlab"]`
//! so it keeps HDF5/netCDF + MATLAB and drops only the Parquet/Arrow reader.
//!
//! Without these stubs a user who feeds an HDF5 / NetCDF / Parquet / MATLAB
//! file to a trimmed build would get the generic
//! [`Error::UnsupportedFormat`](nirs4all_formats_core::Error::UnsupportedFormat)
//! ("unsupported format for {path}") — the same message a genuinely unknown
//! file produces. That is misleading: the format IS supported by the project,
//! just not compiled into this build.
//!
//! Each stub below sniffs ONLY the excluded format's magic bytes (a few bytes
//! of head, no heavy parsing) and, on read, returns an actionable
//! [`Error::InvalidRecord`] telling the user the reader was excluded and how to
//! get the full build. The stubs are gated to compile EXACTLY when the matching
//! `fmt-*` feature is OFF, so they never shadow a real reader.

use std::path::Path;

use nirs4all_formats_core::{Confidence, Error, FormatProbe, Result, SpectralRecord};

use crate::Reader;

#[cfg(not(feature = "fmt-hdf5"))]
const HDF5_MAGIC: &[u8] = b"\x89HDF\r\n\x1a\n";

/// Build the "this reader is excluded from this build" error message.
///
/// `format` is the human-facing format label; `feature` is the Cargo feature
/// that would re-enable it. The message names the CRAN lite build explicitly
/// because that is the only shipped configuration that trims readers, and
/// points at the full R-universe build that carries every reader.
fn excluded_error(format: &str, feature: &str) -> Error {
    Error::InvalidRecord(format!(
        "the {format} reader is not available in this build of 'nirs4all-formats' \
         (the '{feature}' feature is disabled). This is the smaller R package \
         'nirs4allformats.lite', which drops the Parquet/Arrow reader. Install the \
         complete package 'nirs4allformats', which ships every reader \
         (HDF5/netCDF, Parquet/Arrow, MATLAB), from R-universe: \
         install.packages(\"nirs4allformats\", repos = c(nirs4all = \
         \"https://gbeurier.r-universe.dev\", CRAN = \
         \"https://cloud.r-project.org\"))"
    ))
}

/// Sniff result for an excluded reader: recognised (so the registry stops at
/// this candidate and surfaces the actionable error) but flagged `Possible`
/// so a real reader for the same bytes — if any were ever compiled — would
/// still win on confidence.
fn excluded_probe(format: &str, reader: &'static str) -> FormatProbe {
    FormatProbe::new(
        format,
        reader,
        Confidence::Possible,
        "format recognised but its reader is excluded from this trimmed build",
    )
}

/// HDF5 / NetCDF container stub (`fmt-hdf5` disabled).
///
/// Covers the readers gated behind `fmt-hdf5`: `Hdf5Reader`, `NetcdfReader`,
/// `AllotropeAdfReader` and `FgiXmlReader`. They are recognised by the HDF5
/// magic (`.h5`/`.hdf5`/`.adf` containers and HDF5-backed NetCDF-4) or the
/// classic NetCDF `CDF\x01/02/05` magic.
#[cfg(not(feature = "fmt-hdf5"))]
pub struct ExcludedHdf5NetcdfReader;

#[cfg(not(feature = "fmt-hdf5"))]
impl Reader for ExcludedHdf5NetcdfReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::excluded_hdf5_netcdf"
    }

    fn sniff(&self, head: &[u8], _path: &Path) -> Option<FormatProbe> {
        let is_netcdf_classic = head.starts_with(b"CDF\x01")
            || head.starts_with(b"CDF\x02")
            || head.starts_with(b"CDF\x05");
        if head.starts_with(HDF5_MAGIC) || is_netcdf_classic {
            Some(excluded_probe("HDF5/netCDF", self.name()))
        } else {
            None
        }
    }

    fn read_path(&self, _path: &Path) -> Result<Vec<SpectralRecord>> {
        Err(excluded_error("HDF5/netCDF", "fmt-hdf5"))
    }

    fn read_bytes(&self, _name: &Path, _bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
        Err(excluded_error("HDF5/netCDF", "fmt-hdf5"))
    }
}

/// FGI XML sidecar stub (`fmt-hdf5` disabled).
///
/// Unlike the binary container stubs above, the real
/// [`FgiXmlReader`](crate::readers::FgiXmlReader) — also gated behind
/// `fmt-hdf5`, because it reads the HDF5 payload the XML references — detects a
/// TEXT (XML) primary file: a `.xml` whose head contains both `<FGIMeasurement`
/// and `<DataReference`. The binary-magic HDF5/netCDF stub never matches it, so
/// without this stub an FGI XML primary falls through to the generic
/// "unsupported format" error. This stub mirrors the real reader's sniff and
/// returns the same actionable "install the full build" message.
#[cfg(not(feature = "fmt-hdf5"))]
pub struct ExcludedFgiXmlReader;

#[cfg(not(feature = "fmt-hdf5"))]
impl Reader for ExcludedFgiXmlReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::excluded_fgi_xml"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext != "xml" {
            return None;
        }
        let text = String::from_utf8_lossy(head);
        (text.contains("<FGIMeasurement") && text.contains("<DataReference"))
            .then(|| excluded_probe("FGI XML/HDF5", self.name()))
    }

    fn read_path(&self, _path: &Path) -> Result<Vec<SpectralRecord>> {
        Err(excluded_error("FGI XML/HDF5", "fmt-hdf5"))
    }

    fn read_bytes(&self, _name: &Path, _bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
        Err(excluded_error("FGI XML/HDF5", "fmt-hdf5"))
    }
}

/// Apache Parquet container stub (`fmt-parquet` disabled).
///
/// Recognised by the `PAR1` magic on a `.parquet` file, matching the real
/// [`ParquetReader`](crate::readers::ParquetReader) sniff exactly.
#[cfg(not(feature = "fmt-parquet"))]
pub struct ExcludedParquetReader;

#[cfg(not(feature = "fmt-parquet"))]
impl Reader for ExcludedParquetReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::excluded_parquet"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext == "parquet" && head.starts_with(b"PAR1") {
            Some(excluded_probe("Parquet/Arrow", self.name()))
        } else {
            None
        }
    }

    fn read_path(&self, _path: &Path) -> Result<Vec<SpectralRecord>> {
        Err(excluded_error("Parquet/Arrow", "fmt-parquet"))
    }

    fn read_bytes(&self, _name: &Path, _bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
        Err(excluded_error("Parquet/Arrow", "fmt-parquet"))
    }
}

/// MATLAB MAT v5 / v7.3 stub (`fmt-matlab` disabled).
///
/// `fmt-matlab` requires `fmt-hdf5` (MATLAB v7.3 files are HDF5 containers), so
/// when `fmt-matlab` is off so is the v7.3 path. This stub therefore only needs
/// to catch the MAT-v5 magic on a `.mat` file; a `.mat` carrying the HDF5 magic
/// (a v7.3 file) is already caught by [`ExcludedHdf5NetcdfReader`] above. The
/// `prospectr` RData (`.rdata`/`.rda`) readers also live behind `fmt-matlab`;
/// they are recognised by the XZ / RDX3 magic.
#[cfg(not(feature = "fmt-matlab"))]
pub struct ExcludedMatlabReader;

#[cfg(not(feature = "fmt-matlab"))]
impl Reader for ExcludedMatlabReader {
    fn name(&self) -> &'static str {
        "nirs4all_formats::readers::excluded_matlab"
    }

    fn sniff(&self, head: &[u8], path: &Path) -> Option<FormatProbe> {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        match ext.as_str() {
            "mat" if head.starts_with(b"MATLAB 5.0 MAT-file") => {
                Some(excluded_probe("MATLAB", self.name()))
            }
            "rdata" | "rda" if head.starts_with(b"\xfd7zXZ\0") || head.starts_with(b"RDX3\n") => {
                Some(excluded_probe("MATLAB/RData", self.name()))
            }
            _ => None,
        }
    }

    fn read_path(&self, _path: &Path) -> Result<Vec<SpectralRecord>> {
        Err(excluded_error("MATLAB/RData", "fmt-matlab"))
    }

    fn read_bytes(&self, _name: &Path, _bytes: &[u8]) -> Result<Vec<SpectralRecord>> {
        Err(excluded_error("MATLAB/RData", "fmt-matlab"))
    }
}
