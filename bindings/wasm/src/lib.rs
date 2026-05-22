//! WebAssembly bridge for `nirs4all-io`.
//!
//! The browser context cannot call `std::fs::read`, so this binding currently
//! exposes the byte-based sniffer surface. Callers pass the file name (used to
//! drive extension-based sniffers) together with the file bytes. Full reads
//! land here once the readers grow a path-free entry point.

use std::path::Path;

use nirs4all_io::{builtin_probes, open_bytes as open_bytes_native};
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
/// In the default WASM build the heavy native-dep readers (HDF5/MATLAB/Parquet)
/// are disabled because their underlying C libraries do not cross-compile to
/// wasm32-unknown-unknown.
#[wasm_bindgen(js_name = features)]
pub fn features() -> Result<JsValue, JsError> {
    let flags = FeatureFlags {
        hdf5: false,
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
/// Sidecar formats (ENVI Standard, AVIRIS LAN) and HDF5/MATLAB/Parquet
/// readers are not available in the default WASM build because they need
/// access to the host filesystem or C libraries that do not cross-compile to
/// wasm32. For those formats this entry point returns an error.
#[wasm_bindgen(js_name = openBytes)]
pub fn open_bytes(filename: &str, bytes: &[u8]) -> Result<JsValue, JsError> {
    let records = open_bytes_native(Path::new(filename), bytes)
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
