//! Public Rust facade for nirs4all-formats.
//!
//! The crate currently exposes the dispatch and probing contract. Full readers
//! are added format-by-format behind this stable surface.

mod multimodal;
mod readers;
mod registry;
mod sidecars;
mod walker;
mod writers;
mod xlsx_cells;

pub use multimodal::{
    decode_multimodal_bytes, multimodal_reader_catalog, DecodedMultimodal, MULTIMODAL_OPERATION,
};

pub use nirs4all_formats_core::xlsx_cells::{
    DecodedCell, DecodedSheet, DecodedWorkbook, XlsxCellOrigin, XlsxCellValue, XlsxDateKind,
    XlsxDecodeError, XlsxDecodeLimits, XlsxDecodeUsage, XlsxUsedExtent, XLSX_CELLS_OPERATION,
    XLSX_CELLS_VERSION,
};
pub use nirs4all_formats_core::{
    AxisKind, AxisOrder, Confidence, Error, FormatProbe, Provenance, Result, SidecarResolver,
    SignalType, SourceFile, SpectralArray, SpectralAxis, SpectralRecord,
};
pub use registry::{
    builtin_probes, open_bytes, open_bytes_with_options, open_path, open_path_with_options,
    open_with_sidecars, open_with_sidecars_and_options, probe_path, reader_catalog,
    sidecar_requirements, CubeMask, CubeSelection, CubeWindow, ReadOptions, Reader,
    ReaderCatalogEntry, SidecarRequirement,
};
pub use sidecars::{FsSidecars, InMemorySidecars, NoSidecars};
pub use walker::{walk_path, WalkEntry, WalkOptions, WalkOutcome, WalkStats};
pub use writers::{
    export_targets, plan_conversion, software_catalog, target_requirements, write_records,
    writer_catalog, writer_requirements, AppliedConversion, ConversionPlan, EvenAxis, Grid,
    OtherGrids, PlanChoice, PlanOptions, PlanQuestion, PlanRefusal, PlanStatus, PlanStep,
    PlannedSignal, SignalCandidate, SoftwareProfile, TargetRequirements, WriteError, WriteLoss,
    WriteOptions, WriteOutput, WriterCatalogEntry, WriterMaturity, WrittenFile, CONVERSIONS_KEY,
};
pub use xlsx_cells::decode_xlsx_cells;
