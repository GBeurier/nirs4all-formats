# Usage guide

Every interface decodes through the same Rust registry, so the concepts below
are identical across the CLI, Python, R and WebAssembly — only the syntax
differs. See [Getting started](getting_started.md) for installation-free
snippets and the [data model](DATA_MODEL.md) for the record contract.

## 1. Probe — which reader, and why?

Probing sniffs the first bytes and returns the ordered candidate readers without
a full parse.

```bash
nirs4all-formats probe path/to/file        # JSON: format, reader, confidence, reason
```

```python
import nirs4all_formats as nio
nio.probe_path("path/to/file")        # list of candidates, best first
```

## 2. Read records

```bash
nirs4all-formats read-json path/to/file    # normalized SpectralRecord[] as JSON
```

```python
# Raw dicts, exactly as the core emits them:
records = nio.open_records("path/to/file")

# Lossless object model (recommended): SpectralRecordSet
rs = nio.open_recordset("path/to/file")
rs.signal_names()                     # what's inside
```

```r
library(nirs4allformats)
records <- nirs4allformats_open_records("path/to/file")
dataset <- nirs4allformats_open_dataset("path/to/file", signal = NULL)
```

## 3. Project to numpy / pandas / sklearn / torch

Projections flatten a chosen signal into a feature matrix. They are explicit and
may be lossy; records that disagree on the spectral axis raise a strict error
(resample with `nirs4all` first) rather than silently aligning.

```python
rs = nio.open_recordset("spectrum.sed")

X, axis = rs.to_numpy(signal="reflectance")   # (X[n_samples, n_features], axis)
df      = rs.to_pandas()                       # wide: metadata + x_<axis> columns
long    = rs.to_pandas_long()                  # one row per (record, signal, point)
bunch   = rs.to_sklearn(signal="reflectance", target="protein")
ds      = rs.to_torch(signal="reflectance")    # torch TensorDataset (float32)
sds     = rs.to_spectrodataset(name="myset")   # nirs4all SpectroDataset
```

Multi-dimensional signals (cubes, maps, time series) also project to
`xarray.DataArray` via `array.to_xarray()`. See the
[Python binding](bindings/python.md) for the full surface; R offers
`as.matrix`, `as.data.frame` and `nirs4allformats_as_tibble`.

## 4. Sidecar formats (companion files)

Some formats need companion files — ENVI `.img`/`.hdr`, AVIRIS `.lan`/`.spc`/`.GIS`,
FGI XML+HDF5, MATLAB Indian Pines `_gt.mat`, ARM MFRSR NetCDF + QC YAML. When you
read from a path these are resolved automatically. When you read from bytes you
must supply them:

```bash
nirs4all-formats read-json --sidecar cube.hdr=path/cube.hdr path/cube.img
```

```python
sidecars = {"cube.hdr": hdr_bytes}
nio.open_with_sidecars("cube.img", img_bytes, sidecars)
```

`open_bytes` refuses sidecar-bearing formats explicitly so callers know to route
through `open_with_sidecars`.

## 5. Image cubes — pick pixels, not the whole scene

Cube readers (ENVI Standard, AVIRIS/ERDAS LAN) accept a rectangular ROI or an
ordered sparse pixel mask, so you never have to materialise a full scene.

```bash
nirs4all-formats read-json --rows 10:20 --cols 30:40 path/cube.hdr   # ROI window
nirs4all-formats read-json --pixel 10,20 --pixel 11,21 path/cube.hdr # sparse pixels
nirs4all-formats read-json --pixels-file pixels.txt   path/cube.hdr  # one ROW,COL per line
```

```python
nio.open_records("cube.hdr", rows=(10, 20), cols=(30, 40))      # ROI
nio.open_records("cube.hdr", pixels=[(10, 20), (11, 21)])       # sparse
```

`open_recordset(..., single_record=True)` keeps the spatial grid as one
N-dimensional record (`dims = ["row", "col", "x"]`); projecting it flattens
`row`/`col` back into samples for modelling.

## 6. In-memory bytes

Every single-file reader decodes straight from a byte slice — no filesystem,
which is also how the WebAssembly build works.

```python
nio.open_bytes("spectrum.jdx", payload)        # payload: bytes
```

```js
import init, { openBytes } from "nirs4all-formats-wasm";
await init();
openBytes("spectrum.jdx", new Uint8Array(buffer));
```

## 7. Scan a directory

The walker recurses a folder, probes each file and labels it
`parsed` / `error` / `unsupported`.

```bash
nirs4all-formats scan path/to/dir --max-depth 2 --include-unsupported --json
```

```python
nio.walk_path("path/to/dir", include_unsupported=True)
```

```r
nirs4allformats_walk_path("path/to/dir", include_unsupported = TRUE)
```
