//! Graceful-degradation tests for the feature-trimmed (lite) build.
//!
//! Compiled only when a heavy reader feature is OFF — i.e. the configuration the
//! CRAN lite R package (`nirs4allformats`) ships. They assert that an excluded
//! format is RECOGNISED (so the registry stops at the stub) and that the read
//! returns the actionable "install the full build" error rather than the
//! generic "unsupported format".
//!
//! In-memory magic bytes are used so the test needs no fixture files.

#![cfg(not(all(feature = "fmt-hdf5", feature = "fmt-matlab", feature = "fmt-parquet")))]

use nirs4all_formats::open_bytes;

const HDF5_MAGIC: &[u8] = b"\x89HDF\r\n\x1a\n";

/// Every excluded-reader error must name the format and point at the full
/// R-universe build.
fn assert_excluded(name: &str, magic: &[u8]) {
    let err = open_bytes(name, magic).expect_err("excluded format must be refused");
    let message = err.to_string();
    assert!(
        message.contains("not available in this build"),
        "{name}: expected the excluded-build error, got {message:?}"
    );
    assert!(
        message.contains("nirs4allformats.full"),
        "{name}: error must point at the full R-universe build, got {message:?}"
    );
    // It must NOT be the generic catch-all.
    assert!(
        !message.contains("unsupported format"),
        "{name}: must not fall through to the generic unsupported-format error, got {message:?}"
    );
}

#[cfg(not(feature = "fmt-hdf5"))]
#[test]
fn hdf5_input_yields_actionable_error() {
    assert_excluded("sample.h5", HDF5_MAGIC);
}

#[cfg(not(feature = "fmt-hdf5"))]
#[test]
fn netcdf_classic_input_yields_actionable_error() {
    assert_excluded("sample.nc", b"CDF\x01\0\0\0\0");
}

/// FGI is an XML *text* primary (not a binary magic), so the HDF5/netCDF stub's
/// magic sniff never matches it. The dedicated FGI XML stub must catch a `.xml`
/// carrying the `<FGIMeasurement>` / `<DataReference>` markers.
#[cfg(not(feature = "fmt-hdf5"))]
#[test]
fn fgi_xml_input_yields_actionable_error() {
    let xml = br#"<?xml version="1.0"?>
<FGIMeasurement>
  <DataReference path="payload.h5"/>
</FGIMeasurement>"#;
    assert_excluded("sample.xml", xml);
}

#[cfg(not(feature = "fmt-parquet"))]
#[test]
fn parquet_input_yields_actionable_error() {
    assert_excluded("sample.parquet", b"PAR1\0\0\0\0");
}

#[cfg(not(feature = "fmt-matlab"))]
#[test]
fn matlab_v5_input_yields_actionable_error() {
    assert_excluded("sample.mat", b"MATLAB 5.0 MAT-file, Platform: X");
}

/// A genuinely unknown file must still produce the generic unsupported-format
/// error — the stubs must not over-claim.
#[test]
fn unknown_input_still_unsupported() {
    let err = open_bytes("mystery.bin", b"\x00\x01\x02\x03not a known format")
        .expect_err("unknown format must be refused");
    let message = err.to_string();
    assert!(
        message.contains("unsupported format"),
        "unknown input should be the generic unsupported-format error, got {message:?}"
    );
}
