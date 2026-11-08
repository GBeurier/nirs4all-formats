//! Core types for nirs4all-formats.
//!
//! This crate is deliberately independent from Python, R, and any vendor
//! reader. Bindings translate these Rust records into language-native shapes.

pub mod error;
pub mod model;
pub mod sidecar;
pub mod signal;
pub mod sniff;
pub mod xlsx_cells;

pub use error::{Error, Result};
pub use model::{
    AxisKind, AxisOrder, Provenance, SourceFile, SpectralArray, SpectralAxis, SpectralRecord,
};
pub use sidecar::SidecarResolver;
pub use signal::SignalType;
pub use sniff::{Confidence, FormatProbe};
pub use xlsx_cells::{
    DecodedCell, DecodedSheet, DecodedWorkbook, XlsxCellOrigin, XlsxCellValue, XlsxDateKind,
    XlsxDecodeError, XlsxDecodeLimits, XlsxDecodeUsage, XlsxUsedExtent, XLSX_CELLS_OPERATION,
    XLSX_CELLS_VERSION,
};
