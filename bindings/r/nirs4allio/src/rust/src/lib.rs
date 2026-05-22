//! extendr-api bridge for `nirs4all-io`.
//!
//! All three native functions return JSON strings so that the R side can
//! lean on `jsonlite::fromJSON` rather than translating SEXP trees by hand.

use std::path::PathBuf;
use std::sync::Arc;

use extendr_api::prelude::*;
use nirs4all_io::{
    open_bytes_with_options, open_path_with_options, open_with_sidecars_and_options, probe_path,
    walk_path, CubeMask, CubeWindow, InMemorySidecars, ReadOptions, SidecarResolver, WalkOptions,
    WalkOutcome,
};

fn to_string<T: serde::Serialize>(value: &T) -> std::result::Result<String, Error> {
    serde_json::to_string(value).map_err(|err| Error::Other(err.to_string()))
}

fn map_io_err<E: std::fmt::Display>(err: E) -> Error {
    Error::Other(err.to_string())
}

/// Probe a file and return the JSON candidates.
/// @export
#[extendr]
fn nirs4allio_native_probe(path: &str) -> Result<String> {
    let probes = probe_path(path).map_err(map_io_err)?;
    to_string(&probes)
}

/// Read a file and return JSON-encoded records.
/// Optional cube selection: pass `rows`/`cols` as length-2 integer vectors
/// (use `NA` as the unbounded end) or `pixels` as a two-column integer matrix
/// (`row`, `col`).
/// @export
#[extendr]
fn nirs4allio_native_read(
    path: &str,
    rows: Nullable<Integers>,
    cols: Nullable<Integers>,
    pixels: Nullable<RMatrix<i32>>,
) -> Result<String> {
    let rows_opt = window_from_integers(rows, "rows")?;
    let cols_opt = window_from_integers(cols, "cols")?;
    let pixels_opt = pixels_from_matrix(pixels)?;
    let options = build_options(rows_opt, cols_opt, pixels_opt)?;
    let records = open_path_with_options(path, &options).map_err(map_io_err)?;
    to_string(&records)
}

/// Decode raw bytes through the native registry and return JSON-encoded
/// records. The file name drives extension-based sniffing and provenance.
/// @export
#[extendr]
fn nirs4allio_native_read_bytes(
    name: &str,
    bytes: &[u8],
    rows: Nullable<Integers>,
    cols: Nullable<Integers>,
    pixels: Nullable<RMatrix<i32>>,
) -> Result<String> {
    let rows_opt = window_from_integers(rows, "rows")?;
    let cols_opt = window_from_integers(cols, "cols")?;
    let pixels_opt = pixels_from_matrix(pixels)?;
    let options = build_options(rows_opt, cols_opt, pixels_opt)?;
    let records = open_bytes_with_options(name, bytes, &options).map_err(map_io_err)?;
    to_string(&records)
}

/// Decode raw bytes plus a list of sidecar payloads through the native
/// registry. `sidecars` must be a named list whose values are raw vectors
/// (`raw()`). Names are interpreted as relative paths next to the primary
/// file (e.g. `"foo.hdr"` next to an ENVI Standard cube).
/// @export
#[extendr]
fn nirs4allio_native_read_with_sidecars(
    name: &str,
    bytes: &[u8],
    sidecars: List,
    rows: Nullable<Integers>,
    cols: Nullable<Integers>,
    pixels: Nullable<RMatrix<i32>>,
) -> Result<String> {
    let rows_opt = window_from_integers(rows, "rows")?;
    let cols_opt = window_from_integers(cols, "cols")?;
    let pixels_opt = pixels_from_matrix(pixels)?;
    let options = build_options(rows_opt, cols_opt, pixels_opt)?;
    let mut resolver = InMemorySidecars::new();
    for (key, value) in sidecars.iter() {
        let raw = value
            .as_raw_slice()
            .ok_or_else(|| Error::Other(format!("sidecar '{key}' must be a raw vector")))?;
        resolver.insert(PathBuf::from(key.to_string()), raw.to_vec());
    }
    let arc: Arc<dyn SidecarResolver> = Arc::new(resolver);
    let records =
        open_with_sidecars_and_options(name, bytes, arc, &options).map_err(map_io_err)?;
    to_string(&records)
}

/// Walk a directory or file and return JSON-encoded outcomes per visited file.
/// @export
#[extendr]
fn nirs4allio_native_walk(
    path: &str,
    max_depth: Nullable<i32>,
    include_hidden: bool,
    follow_symlinks: bool,
    include_unsupported: bool,
) -> Result<String> {
    let max_depth = match max_depth {
        Nullable::NotNull(depth) if depth >= 0 => Some(depth as usize),
        Nullable::NotNull(_) => return Err(Error::Other("max_depth must be >= 0".into())),
        Nullable::Null => None,
    };
    let options = WalkOptions {
        max_depth,
        skip_hidden: !include_hidden,
        follow_symlinks,
        skip_unsupported: !include_unsupported,
        read_options: ReadOptions::default(),
    };
    let entries = walk_path(path, &options).map_err(map_io_err)?;
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
    to_string(&payload)
}

fn window_from_integers(
    value: Nullable<Integers>,
    label: &str,
) -> Result<Option<(usize, Option<usize>)>> {
    let Nullable::NotNull(values) = value else {
        return Ok(None);
    };
    if values.len() != 2 {
        return Err(Error::Other(format!("{label} must have length 2")));
    }
    let start = values[0].inner();
    let end = values[1];
    if start < 0 {
        return Err(Error::Other(format!("{label} start must be >= 0")));
    }
    let end_opt = if end.is_na() {
        None
    } else {
        let e = end.inner();
        if e < 0 {
            return Err(Error::Other(format!("{label} end must be >= 0")));
        }
        Some(e as usize)
    };
    Ok(Some((start as usize, end_opt)))
}

fn pixels_from_matrix(matrix: Nullable<RMatrix<i32>>) -> Result<Option<Vec<(usize, usize)>>> {
    let Nullable::NotNull(matrix) = matrix else {
        return Ok(None);
    };
    if matrix.ncols() != 2 {
        return Err(Error::Other(
            "pixels must be a two-column integer matrix (row, col)".into(),
        ));
    }
    let rows = matrix.nrows();
    let data = matrix.data();
    let mut out = Vec::with_capacity(rows);
    for row in 0..rows {
        let r = data[row];
        let c = data[row + rows];
        if r < 0 || c < 0 {
            return Err(Error::Other(format!(
                "pixels must be non-negative, got ({r}, {c})"
            )));
        }
        out.push((r as usize, c as usize));
    }
    Ok(Some(out))
}

fn build_options(
    rows: Option<(usize, Option<usize>)>,
    cols: Option<(usize, Option<usize>)>,
    pixels: Option<Vec<(usize, usize)>>,
) -> Result<ReadOptions> {
    let has_window = rows.is_some() || cols.is_some();
    let has_mask = pixels.is_some();
    if has_window && has_mask {
        return Err(Error::Other(
            "rows/cols cannot be combined with pixels".into(),
        ));
    }
    if let Some(pixels) = pixels {
        return Ok(ReadOptions::default().with_cube_mask(CubeMask::new(pixels)));
    }
    if has_window {
        let (row_start, row_end) = rows.unwrap_or((0, None));
        let (col_start, col_end) = cols.unwrap_or((0, None));
        return Ok(ReadOptions::default().with_cube_window(CubeWindow::new(
            row_start, row_end, col_start, col_end,
        )));
    }
    Ok(ReadOptions::default())
}

extendr_module! {
    mod nirs4allio_r;
    fn nirs4allio_native_probe;
    fn nirs4allio_native_read;
    fn nirs4allio_native_read_bytes;
    fn nirs4allio_native_read_with_sidecars;
    fn nirs4allio_native_walk;
}
