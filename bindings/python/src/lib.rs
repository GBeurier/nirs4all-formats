//! PyO3 bridge for `nirs4all-io`.
//!
//! Exposes the registry's probe/read/walk APIs to Python without going through
//! the CLI. Records are returned as plain Python dict/list trees that mirror
//! the JSON shape produced by `nirs4all-io read-json`.

// Triggered by the `#[pyfunction]` macro expansion in pyo3 0.22.
#![allow(clippy::useless_conversion)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use nirs4all_io::{
    open_bytes_with_options, open_path_with_options, open_with_sidecars_and_options, probe_path,
    walk_path, CubeMask, CubeWindow, InMemorySidecars, ReadOptions, SidecarResolver, WalkOptions,
    WalkOutcome,
};
use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::pythonize;

fn map_err<E: std::fmt::Display>(err: E) -> PyErr {
    PyIOError::new_err(err.to_string())
}

fn to_json(value: impl serde::Serialize) -> Result<serde_json::Value, PyErr> {
    serde_json::to_value(value).map_err(|err| PyValueError::new_err(err.to_string()))
}

fn build_options(
    rows: Option<(usize, Option<usize>)>,
    cols: Option<(usize, Option<usize>)>,
    pixels: Option<Vec<(usize, usize)>>,
    single_record: bool,
) -> PyResult<ReadOptions> {
    let has_window = rows.is_some() || cols.is_some();
    let has_mask = pixels.is_some();
    if has_window && has_mask {
        return Err(PyValueError::new_err(
            "rows/cols cannot be combined with pixels",
        ));
    }
    let mut options = if let Some(pixels) = pixels {
        ReadOptions::default().with_cube_mask(CubeMask::new(pixels))
    } else if has_window {
        let (row_start, row_end) = rows.unwrap_or((0, None));
        let (col_start, col_end) = cols.unwrap_or((0, None));
        ReadOptions::default().with_cube_window(CubeWindow::new(
            row_start, row_end, col_start, col_end,
        ))
    } else {
        ReadOptions::default()
    };
    if single_record {
        options = options.single_record();
    }
    Ok(options)
}

/// Probe a file and return JSON-like candidates ordered by confidence.
#[pyfunction]
#[pyo3(name = "probe_path", text_signature = "(path)")]
fn py_probe_path(py: Python<'_>, path: PathBuf) -> PyResult<PyObject> {
    let probes = probe_path(&path).map_err(map_err)?;
    let value = to_json(probes)?;
    Ok(pythonize(py, &value).map_err(map_err)?.into())
}

/// Read a file and return JSON-like records.
///
/// Cube readers (ENVI Standard, AVIRIS/ERDAS LAN) accept optional
/// `rows`/`cols` half-open windows as `(start, end)` tuples, or a sparse
/// `pixels` mask as a list of `(row, col)` pairs. The two paths cannot be
/// combined.
#[pyfunction]
#[pyo3(
    name = "open_path",
    signature = (path, rows=None, cols=None, pixels=None, single_record=false),
    text_signature = "(path, *, rows=None, cols=None, pixels=None, single_record=False)"
)]
fn py_open_path(
    py: Python<'_>,
    path: PathBuf,
    rows: Option<(usize, Option<usize>)>,
    cols: Option<(usize, Option<usize>)>,
    pixels: Option<Vec<(usize, usize)>>,
    single_record: bool,
) -> PyResult<PyObject> {
    let options = build_options(rows, cols, pixels, single_record)?;
    let records = open_path_with_options(&path, &options).map_err(map_err)?;
    let value = to_json(records)?;
    Ok(pythonize(py, &value).map_err(map_err)?.into())
}

/// Read raw bytes through the native registry. `name` is the input file name
/// (used for extension sniffing and provenance). Sidecar formats (ENVI
/// Standard, AVIRIS LAN, FGI HDF5+XML, MATLAB Indian Pines, NetCDF MFRSR
/// with QC sidecar) error here with `Error::UnsupportedSidecar`; pass them
/// through [`open_with_sidecars`] instead.
#[pyfunction]
#[pyo3(
    name = "open_bytes",
    signature = (name, bytes, rows=None, cols=None, pixels=None, single_record=false),
    text_signature = "(name, bytes, *, rows=None, cols=None, pixels=None, single_record=False)"
)]
fn py_open_bytes(
    py: Python<'_>,
    name: PathBuf,
    bytes: &[u8],
    rows: Option<(usize, Option<usize>)>,
    cols: Option<(usize, Option<usize>)>,
    pixels: Option<Vec<(usize, usize)>>,
    single_record: bool,
) -> PyResult<PyObject> {
    let options = build_options(rows, cols, pixels, single_record)?;
    let records = open_bytes_with_options(&name, bytes, &options).map_err(map_err)?;
    let value = to_json(records)?;
    Ok(pythonize(py, &value).map_err(map_err)?.into())
}

/// Read raw bytes plus a mapping of relative sidecar names to byte
/// payloads. Use this entry point for ENVI Standard cubes, ENVI SLI,
/// AVIRIS/ERDAS LAN, FGI HDF5+XML, MATLAB Indian Pines or NetCDF MFRSR with
/// QC sidecars without touching the filesystem.
#[pyfunction]
#[pyo3(
    name = "open_with_sidecars",
    signature = (name, bytes, sidecars, rows=None, cols=None, pixels=None, single_record=false),
    text_signature = "(name, bytes, sidecars, *, rows=None, cols=None, pixels=None, single_record=False)"
)]
#[allow(clippy::too_many_arguments)]
fn py_open_with_sidecars(
    py: Python<'_>,
    name: PathBuf,
    bytes: &[u8],
    sidecars: HashMap<String, Vec<u8>>,
    rows: Option<(usize, Option<usize>)>,
    cols: Option<(usize, Option<usize>)>,
    pixels: Option<Vec<(usize, usize)>>,
    single_record: bool,
) -> PyResult<PyObject> {
    let options = build_options(rows, cols, pixels, single_record)?;
    let mut map = InMemorySidecars::new();
    for (key, value) in sidecars {
        map.insert(PathBuf::from(key), value);
    }
    let resolver: Arc<dyn SidecarResolver> = Arc::new(map);
    let records =
        open_with_sidecars_and_options(&name, bytes, resolver, &options).map_err(map_err)?;
    let value = to_json(records)?;
    Ok(pythonize(py, &value).map_err(map_err)?.into())
}

/// Walk a directory or file and return one entry per visited file.
///
/// Each entry is a dict with at minimum `path` and `status` ∈ {`parsed`,
/// `error`, `unsupported`}.
#[pyfunction]
#[pyo3(
    name = "walk_path",
    signature = (
        path,
        *,
        max_depth=None,
        include_hidden=false,
        follow_symlinks=false,
        include_unsupported=false,
        rows=None,
        cols=None,
        pixels=None,
        single_record=false,
    ),
    text_signature = "(path, *, max_depth=None, include_hidden=False, follow_symlinks=False, include_unsupported=False, rows=None, cols=None, pixels=None, single_record=False)"
)]
#[allow(clippy::too_many_arguments)]
fn py_walk_path(
    py: Python<'_>,
    path: PathBuf,
    max_depth: Option<usize>,
    include_hidden: bool,
    follow_symlinks: bool,
    include_unsupported: bool,
    rows: Option<(usize, Option<usize>)>,
    cols: Option<(usize, Option<usize>)>,
    pixels: Option<Vec<(usize, usize)>>,
    single_record: bool,
) -> PyResult<PyObject> {
    let read_options = build_options(rows, cols, pixels, single_record)?;
    let options = WalkOptions {
        max_depth,
        skip_hidden: !include_hidden,
        follow_symlinks,
        skip_unsupported: !include_unsupported,
        read_options,
    };
    let entries = walk_path(&path, &options).map_err(map_err)?;

    let payload: Vec<serde_json::Value> = entries
        .into_iter()
        .map(|entry| match entry.outcome {
            WalkOutcome::Parsed { format, records } => serde_json::json!({
                "path": entry.path,
                "status": "parsed",
                "format": format,
                "records": records,
            }),
            WalkOutcome::Error {
                candidate_format,
                message,
            } => serde_json::json!({
                "path": entry.path,
                "status": "error",
                "candidate_format": candidate_format,
                "message": message,
            }),
            WalkOutcome::Unsupported => serde_json::json!({
                "path": entry.path,
                "status": "unsupported",
            }),
        })
        .collect();
    let value = serde_json::Value::Array(payload);
    Ok(pythonize(py, &value).map_err(map_err)?.into())
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(py_probe_path, m)?)?;
    m.add_function(wrap_pyfunction!(py_open_path, m)?)?;
    m.add_function(wrap_pyfunction!(py_open_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(py_open_with_sidecars, m)?)?;
    m.add_function(wrap_pyfunction!(py_walk_path, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    // Constants surfacing the underlying library facilities.
    let formats = PyDict::new_bound(m.py());
    formats.set_item("native", true)?;
    m.add("BACKEND", formats)?;
    Ok(())
}
