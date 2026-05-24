//! WebAssembly bridge for `nirs4all-io`.
//!
//! The browser context cannot call `std::fs::read`, so this binding exposes
//! byte-based entry points. Callers pass the file name (used to drive
//! extension-based sniffers) plus the file bytes, and optionally a map of
//! sidecar names → byte payloads for formats that need a companion file
//! (ENVI Standard, ENVI SLI, AVIRIS/ERDAS LAN, FGI XML+HDF5, NetCDF MFRSR).
//! The `fmt-hdf5` feature is on by default, so HDF5/NetCDF-backed readers
//! (generic HDF5, FGI XML+HDF5, NetCDF MFRSR, Allotrope ADF) are available;
//! `fmt-matlab` and `fmt-parquet` remain off.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nirs4all_io::{
    builtin_probes, open_bytes as open_bytes_native, open_with_sidecars as open_with_sidecars_native,
    InMemorySidecars, SidecarResolver,
};
use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::prelude::*;

fn js_serializer() -> Serializer {
    // Produce plain JS objects (not Map) and JS numbers (not BigInt) to keep
    // the surface compatible with vanilla `JSON.stringify` consumers.
    Serializer::new().serialize_maps_as_objects(true)
}

#[wasm_bindgen(start)]
pub fn _start() {
    #[cfg(feature = "console-errors")]
    console_error_panic_hook::set_once();
}

/// Crate version exposed to JS.
#[wasm_bindgen(js_name = version)]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Feature flags the WASM build was compiled with. Useful for runtime checks
/// from JS ("does this WASM bundle have HDF5 support?").
///
/// `fmt-hdf5` is on by default (pure-Rust HDF5/NetCDF decoders cross-compile
/// to wasm32 since the upstream `read_exact_at` fix). `fmt-matlab` and
/// `fmt-parquet` stay off in the WASM build.
#[wasm_bindgen(js_name = features)]
pub fn features() -> Result<JsValue, JsError> {
    let flags = FeatureFlags {
        hdf5: cfg!(feature = "fmt-hdf5"),
        matlab: false,
        parquet: false,
    };
    flags
        .serialize(&js_serializer())
        .map_err(|err| JsError::new(&err.to_string()))
}

/// Probe a file by name + bytes. Returns the ordered candidate readers.
///
/// The file name is required because several sniffers disambiguate by
/// extension (`.lan`, `.spc`, `.hdr`, ...). The first 8 KB of `bytes` is
/// inspected; pass the entire buffer or just the head — the implementation is
/// the same.
#[wasm_bindgen(js_name = probeBytes)]
pub fn probe_bytes(filename: &str, bytes: &[u8]) -> Result<JsValue, JsError> {
    let head_len = bytes.len().min(8192);
    let probes = builtin_probes(&bytes[..head_len], Path::new(filename));
    probes
        .serialize(&js_serializer())
        .map_err(|err| JsError::new(&err.to_string()))
}

/// Decode a file by name + bytes. Returns the spectral records as a JS array
/// matching the JSON shape produced by `nirs4all-io read-json`.
///
/// Sidecar formats (ENVI Standard, AVIRIS LAN, FGI HDF5+XML, ...) return an
/// `UnsupportedSidecar` error here; use `openWithSidecars` instead.
/// Single-file HDF5 / NetCDF payloads decode directly through this entry
/// point now that `fmt-hdf5` is on.
#[wasm_bindgen(js_name = openBytes)]
pub fn open_bytes(filename: &str, bytes: &[u8]) -> Result<JsValue, JsError> {
    let records = open_bytes_native(Path::new(filename), bytes)
        .map_err(|err| JsError::new(&err.to_string()))?;
    records
        .serialize(&js_serializer())
        .map_err(|err| JsError::new(&err.to_string()))
}

/// Decode a file by name + bytes plus a map of sidecar names → byte
/// payloads. Keys are relative path names (e.g. `"foo.hdr"` next to the
/// primary file). For the WASM build this powers ENVI Standard, ENVI SLI,
/// AVIRIS/ERDAS LAN and the HDF5-backed sidecar formats (FGI XML+HDF5,
/// NetCDF MFRSR with its QC YAML) now that `fmt-hdf5` is on.
#[wasm_bindgen(js_name = openWithSidecars)]
pub fn open_with_sidecars(
    filename: &str,
    bytes: &[u8],
    sidecars: JsValue,
) -> Result<JsValue, JsError> {
    let map: HashMap<String, Vec<u8>> = serde_wasm_bindgen::from_value(sidecars)
        .map_err(|err| JsError::new(&format!("sidecars must be an object of Uint8Array: {err}")))?;
    let mut resolver = InMemorySidecars::new();
    for (key, value) in map {
        resolver.insert(PathBuf::from(key), value);
    }
    let arc: Arc<dyn SidecarResolver> = Arc::new(resolver);
    let records = open_with_sidecars_native(Path::new(filename), bytes, arc)
        .map_err(|err| JsError::new(&err.to_string()))?;
    records
        .serialize(&js_serializer())
        .map_err(|err| JsError::new(&err.to_string()))
}

#[derive(Serialize)]
struct FeatureFlags {
    hdf5: bool,
    matlab: bool,
    parquet: bool,
}
