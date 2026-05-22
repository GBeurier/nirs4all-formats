//! Core types for nirs4all-io.
//!
//! This crate is deliberately independent from Python, R, and any vendor
//! reader. Bindings translate these Rust records into language-native shapes.

pub mod error;
pub mod model;
pub mod sidecar;
pub mod signal;
pub mod sniff;

pub use error::{Error, Result};
pub use model::{
    AxisKind, AxisOrder, Provenance, SourceFile, SpectralArray, SpectralAxis, SpectralRecord,
};
pub use sidecar::SidecarResolver;
pub use signal::SignalType;
pub use sniff::{Confidence, FormatProbe};
