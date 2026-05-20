//! Public Rust facade for nirs4all-io.
//!
//! The crate currently exposes the dispatch and probing contract. Full readers
//! are added format-by-format behind this stable surface.

mod readers;
mod registry;

pub use nirs4all_io_core::{
    AxisKind, AxisOrder, Confidence, Error, FormatProbe, Provenance, Result, SignalType,
    SourceFile, SpectralArray, SpectralAxis, SpectralRecord,
};
pub use registry::{
    builtin_probes, open_path, open_path_with_options, probe_path, CubeWindow, ReadOptions, Reader,
};
