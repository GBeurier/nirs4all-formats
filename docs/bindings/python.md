# Python Binding

Python bindings are thin wrappers over the Rust core. Parser logic must stay in
Rust. There are two layers: raw access and the lossless object model.

## Raw access

Records exactly as the Rust core emits them, as plain dicts:

- `open_records(path)`: normalized records as Python dictionaries;
- `open_bytes(name, payload)`: decode in-memory bytes (sidecar formats raise
  `UnsupportedSidecar`);
- `open_with_sidecars(name, payload, sidecars)`: decode bytes plus a
  `{name: bytes}` map of companion files;
- `probe_path(path)`: ordered candidate readers without a full parse;
- `walk_path(path, ...)`: recursive per-file outcomes.

## Lossless object model

`open_recordset(path, single_record=False)` returns a `SpectralRecordSet`, a
faithful mirror of the
Rust `SpectralRecord`: every signal, its N-dimensional `shape`/`dims`, the
spectral `axis`, per-dimension `coords`, full `metadata` and `provenance`.
Nothing is reshaped, aligned or dropped. The dataclasses are `SpectralRecordSet`,
`SpectralRecord`, `SpectralArray`, `SpectralAxis`, `SourceFile`, `Provenance`.
`SpectralArray.values` is reshaped to `shape` (C-order); `SpectralArray.to_xarray()`
returns a labelled `xarray.DataArray` when xarray is installed.

`single_record=True` asks the cube readers (ENVI Standard, AVIRIS/ERDAS LAN)
to emit one N-dimensional record (`dims = ["row", "col", "x"]`, with `row`/`col`
coordinates) instead of one record per pixel — preserving the spatial grid.
Projecting such a record (`to_numpy`, `to_sklearn`, …) flattens `row`/`col`
back into samples, so you still get `X[n_pixels, n_bands]` for modelling.

## Projections (explicit, possibly lossy)

Methods on `SpectralRecordSet` flatten the chosen feature dimension into columns
and every other dimension into samples:

- `to_numpy(signal=None, feature_dim="x")`: `(X[n_samples, n_features], axis)`;
- `to_pandas(signal=None)`: wide DataFrame — metadata + reserved
  `nirs4all_io.*` provenance columns + `x_<axis>` columns;
- `to_polars(signal=None)`: same wide table as `to_pandas`, as a polars
  DataFrame (the backend nirs4all's `SpectroDataset.metadata()` uses);
- `to_pandas_long()`: loss-minimising long frame, one row per
  `(record, signal, point)`;
- `to_sklearn(signal=None, target=None)`: scikit-learn `Bunch`;
- `to_torch(signal=None, target=None)`: a `torch.utils.data.TensorDataset`
  (float32);
- `to_spectrodataset(name=..., signals=None, target=None)`: a nirs4all
  `SpectroDataset` where each signal becomes a source; provenance and quality
  flags travel as reserved `nirs4all_io.*` metadata columns (including JSON
  blobs) so model reports can trace file origin.

Projection contract: records that disagree on the feature axis raise a strict
error with a projection report (resample with nirs4all before projecting). A
record missing a selected signal contributes a NaN-filled row.

## Transport

- native PyO3 extension (`_native`) built by maturin is used when present;
- otherwise the bridge calls `nirs4all-io read-json`; `NIRS4ALL_IO_CLI` can
  point to a prebuilt binary, and in a source checkout it falls back to
  `cargo run -p nirs4all-io-cli`.

## Examples

Load, inspect, then project to a modelling matrix:

```python
import nirs4all_io as nio

rs = nio.open_recordset("spectrum.sed")
print(rs.signal_names(), len(rs))

X, axis = rs.to_numpy(signal="reflectance")   # (X[n_samples, n_features], axis)
df      = rs.to_pandas()                        # wide frame for inspection/export
```

Read a hyperspectral cube without materialising the whole scene:

```python
# Rectangular ROI window (half-open) or an ordered sparse pixel list
roi    = nio.open_records("cube.hdr", rows=(10, 20), cols=(30, 40))
sparse = nio.open_records("cube.hdr", pixels=[(10, 20), (11, 21)])

# Keep the spatial grid as one N-dimensional record, then project
grid   = nio.open_recordset("cube.hdr", single_record=True)
arr    = grid[0].signals["reflectance"]
cube   = arr.to_xarray()                        # dims ("row", "col", "x")
X, ax  = grid.to_numpy(signal="reflectance")    # row/col flattened into samples
```

Decode in memory (e.g. an upload), routing sidecar formats explicitly:

```python
records = nio.open_bytes("spectrum.jdx", payload)               # bytes
cube    = nio.open_with_sidecars("cube.img", img_bytes,
                                 {"cube.hdr": hdr_bytes})
```
