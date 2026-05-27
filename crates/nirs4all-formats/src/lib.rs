//! Public Rust facade for nirs4all-formats.
//!
//! The crate currently exposes the dispatch and probing contract. Full readers
//! are added format-by-format behind this stable surface.

mod readers;
mod registry;
mod sidecars;
mod walker;

pub use nirs4all_formats_core::{
    AxisKind, AxisOrder, Confidence, Error, FormatProbe, Provenance, Result, SidecarResolver,
    SignalType, SourceFile, SpectralArray, SpectralAxis, SpectralRecord,
};
pub use registry::{
    builtin_probes, open_bytes, open_bytes_with_options, open_path, open_path_with_options,
    open_with_sidecars, open_with_sidecars_and_options, probe_path, CubeMask, CubeSelection,
    CubeWindow, ReadOptions, Reader,
};
pub use sidecars::{FsSidecars, InMemorySidecars, NoSidecars};
pub use walker::{walk_path, WalkEntry, WalkOptions, WalkOutcome, WalkStats};
