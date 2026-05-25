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

`open_recordset(path)` returns a `SpectralRecordSet`, a faithful mirror of the
Rust `SpectralRecord`: every signal, its N-dimensional `shape`/`dims`, the
spectral `axis`, per-dimension `coords`, full `metadata` and `provenance`.
Nothing is reshaped, aligned or dropped. The dataclasses are `SpectralRecordSet`,
`SpectralRecord`, `SpectralArray`, `SpectralAxis`, `SourceFile`, `Provenance`.
`SpectralArray.values` is reshaped to `shape` (C-order); `SpectralArray.to_xarray()`
returns a labelled `xarray.DataArray` when xarray is installed.

## Projections (explicit, possibly lossy)

Methods on `SpectralRecordSet` flatten the chosen feature dimension into columns
and every other dimension into samples:

- `to_numpy(signal=None, feature_dim="x")`: `(X[n_samples, n_features], axis)`;
- `to_pandas(signal=None)`: wide DataFrame — metadata + reserved
  `nirs4all_io.*` provenance columns + `x_<axis>` columns;
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
