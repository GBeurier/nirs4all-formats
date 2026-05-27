# nirs4all-formats (Python)

Python bindings for [`nirs4all-formats`](https://github.com/GBeurier/nirs4all-formats),
the Rust-first low-level reader for NIRS and spectroscopy file formats. It reads
~58 format families, auto-detecting each file by content, and projects the
canonical records into numpy / pandas / polars / sklearn / torch / xarray or a
`nirs4all` `SpectroDataset`.

Parsers live entirely in the Rust core — this package is a thin, lossless
surface over it.

## Install

```bash
pip install nirs4all-formats                                  # Python 3.10+
pip install "nirs4all-formats[numpy,pandas,sklearn,torch]"    # projection extras
```

`to_polars()` needs `polars`, `to_xarray()` needs `xarray`, and
`to_spectrodataset()` needs `nirs4all`.

## Quick start

```python
import nirs4all_formats as nio

# Probe: which reader will handle this file, and why?
nio.probe_path("spectrum.jdx")

# Lossless object model — every signal, axis, coord, metadata and provenance
rs = nio.open_recordset("spectrum.sed")
rs.signal_names()

# Modelling-ready projections (explicit; may be lossy)
X, axis = rs.to_numpy(signal="reflectance")   # (X[n_samples, n_features], axis)
df      = rs.to_pandas()                        # wide: metadata + x_<axis> columns
bunch   = rs.to_sklearn(signal="reflectance", target="protein")
```

## API at a glance

**Raw access** (records exactly as the Rust core emits them, as dicts):
`open_records`, `open_bytes`, `open_with_sidecars`, `probe_path`, `walk_path`.

**Object model:** `open_recordset(path) -> SpectralRecordSet` with dataclasses
`SpectralRecord`, `SpectralArray`, `SpectralAxis`, `SourceFile`, `Provenance`.

**Projections on `SpectralRecordSet`:** `to_numpy`, `to_pandas`,
`to_pandas_long`, `to_polars`, `to_sklearn`, `to_torch`, `to_spectrodataset`;
`SpectralArray.to_xarray()` for N-dimensional signals (cubes, maps, series).

Image-cube readers accept pixel selection: `rows=`/`cols=` (rectangular ROI),
`pixels=[(r, c), …]` (sparse), or `single_record=True` to keep the spatial grid.

A native PyO3 extension (`nirs4all_formats._native`) is used when present; otherwise
the bridge falls back to the `nirs4all-formats` CLI (`NIRS4ALL_FORMATS_CLI` can point to a
prebuilt binary).

## Documentation

Full reference: <https://github.com/GBeurier/nirs4all-formats/blob/main/docs/bindings/python.md>.
The usage guide, supported-format catalogue and data model live in the
[project docs](https://github.com/GBeurier/nirs4all-formats/tree/main/docs).

## License

MIT.
