//! WebAssembly bridge for `nirs4all-io`.
//!
//! The browser context cannot call `std::fs::read`, so this binding currently
//! exposes the byte-based sniffer surface. Callers pass the file name (used to
//! drive extension-based sniffers) together with the file bytes. Full reads
//! land here once the readers grow a path-free entry point.

use std::path::Path;

use nirs4all_io::builtin_probes;
use serde::Serialize;
use wasm_bindgen::prelude::*;

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
    serde_wasm_bindgen::to_value(&flags).map_err(|err| JsError::new(&err.to_string()))
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
    serde_wasm_bindgen::to_value(&probes).map_err(|err| JsError::new(&err.to_string()))
}

#[derive(Serialize)]
struct FeatureFlags {
    hdf5: bool,
    matlab: bool,
    parquet: bool,
}
