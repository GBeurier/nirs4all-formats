# User Outputs

This document describes the outputs that `nirs4all-formats` must provide to end
users. The library reads heterogeneous spectroscopy formats, so it must expose
both a rich model and simple projections for data science tools.

## Principles

- The Rust model remains the source of truth.
- Python, R, WASM, and future bindings do not reimplement parsers; they expose
  the same data in idiomatic shapes.
- Canonical outputs must preserve signals, axes, dimensions, metadata,
  provenance, and warnings.
- Tabular or ML projections can be convenient but are potentially destructive;
  they must be explicit.
- A format containing multiple signals, blocks, pixels, time series, or processed
  versions must not be implicitly reduced to a single matrix.

## Level 1: non-destructive canonical model

These outputs have priority because they preserve the information extracted from
the source file.

| Output | Interfaces | Expected content | Priority |
|---|---|---|---|
| `SpectralRecordSet` | Python, R, WASM, future C ABI | Set of homogeneous or heterogeneous records from a file or batch. | P0 |
| `Vec<SpectralRecord>` | Rust | Native representation already used by the core. | P0 |
| `SpectralRecord` | All | A sample, pixel, acquisition, block, or logical unit. | P0 |
| `SpectralArray` | All | A named signal with `values`, `axis`, `shape`, `dims`, `coords`, `unit`, `role`, and `source`. | P0 |
| `metadata` | All | Instrument, acquisition, sample, vendor, sidecar, and domain metadata. | P0 |
| `provenance` | All | Source file, detected format, reader, hash, version, warnings, and known limits. | P0 |
| `quality_flags` | All | Explicit indicators for conversion, missing data, suspicious axes, or partial support. | P1 |

Expected contract: this layer does not resample, merge signals, or choose a
signal on the user's behalf. It must be able to represent a 1-D spectrum, a
`[row, col, x]` cube, a `[time, x]` series, or multiple signals such as
`raw_counts`, `white_reference`, `dark_reference`, `absorbance`, and
`reflectance`.

## Level 2: simple data science outputs

These outputs serve users who want to load data quickly into a notebook, an R
script, or an ML pipeline.

| Output | Interfaces | Shape | Usage |
|---|---|---|---|
| `X, axis` | Python, R, Rust | `n_samples x n_features` matrix + spectral axis. | Preprocessing, regression, classification. |
| `wide DataFrame` | pandas, polars, R `data.frame`/tibble | One row per sample/pixel; metadata columns + spectral columns. | Domain use, Excel-like workflows, classical ML. |
| `long DataFrame` | pandas, R tidyverse | `record_id`, `signal`, `x`, `value`, metadata columns. | Multi-signal data, visualization, heterogeneous axes. |
| `targets` | Python, R, Rust | Separate table or dedicated columns. | Lab values, classes, chemical properties. |
| `sklearn.Bunch` | Python | `data`, `target`, `feature_names`, `metadata`. | scikit-learn integration. |
| `torch.TensorDataset` | Python | `float32` tensors for `X` and optional target. | Deep learning. |
| `SpectroDataset` | Python / nirs4all | Dataset compatible with the `nirs4all` ecosystem. | Downstream modeling. |

Important rule: if records do not share the same spectral axis, the wide or `X`
projection must fail with a clear diagnostic. Resampling must remain an explicit
step in `nirs4all` or in user code.

## Level 3: outputs for complex data

Some project formats are not naturally tabular. The following outputs must avoid
losing their structure.

| Output | Relevant cases | Recommended shape |
|---|---|---|
| `xarray.DataArray` | Hyperspectral cubes, Raman maps, time-spectrum series. | Named dims and coords. |
| `xarray.Dataset` | Multiple aligned or partially aligned signals. | Variables per signal. |
| `ND array + dims + coords` | Rust, WASM, C ABI, bindings without xarray. | Typed buffer + shape + coordinates. |
| `pixel table` | ENVI, ERDAS, Specim, AVIRIS, maps. | `row`, `col`, spatial metadata, selected signal. |
| `multi-signal dataset` | Raw/dark/white/processed in the same file. | One signal per source, explicit roles. |
| `record inventory` | JCAMP `LINK`, OPUS, SPC multi-subfile, HDF5 projects. | List of available blocks/signals before projection. |

A user interface should allow the signal to be selected explicitly, for example
`signal="absorbance"`, `signal="reflectance"`, or `signal="raw_counts"`.

## File Exports

The CLI and bindings must be able to write stable output formats for workflows
outside code.

| Export | Priority | Content | Usage |
|---|---|---|---|
| JSON lossless | P0 | Complete `SpectralRecord[]`, metadata, provenance, warnings. | Stable CLI/bindings/tests transport. |
| CSV wide | P0 | Simple table with metadata and spectral columns. | Excel, domain tools, quick import. |
| CSV long | P1 | One row per spectral point and per signal. | Multi-signal data, non-aligned axes, visualization. |
| Parquet | P1 | Wide or long table with stable schema. | Large volumes, data lakes, Python/R/Polars. |
| Arrow IPC | P2 | Serialized in-memory table. | Fast Python/R/JS exchange. |
| HDF5 or Zarr | P2 | N-D data, chunks, coords, metadata. | Large cubes and series. |
| PNG quicklook | P3 | Overlaid curves, heatmap, or cube preview. | Quick quality control. |
| Diagnostics JSON | P1 | Dimensions, axes, signals, warnings, NaN, saturation, hashes. | Automated audit and user support. |

## Recommended Bundle

For a complete CLI export, use a structured directory:

```text
dataset.n4io/
  manifest.json
  records.json
  spectra_wide.csv
  spectra_long.csv
  spectra.parquet
  metadata.json
  targets.csv
  diagnostics.json
  quicklook.png
```

`manifest.json` must describe the `nirs4all-formats` version, source file,
detected format, commands used, exported signal, pixel/ROI selection options,
and checksums.

## Interface Surfaces

| Interface | Outputs to provide |
|---|---|
| Rust | `open_path() -> Vec<SpectralRecord>`, `RecordSet`, controlled projections, JSON/CSV/Parquet exports. |
| Python | Raw dict, `SpectralRecordSet` dataclasses, `numpy`, `pandas`, `polars`, `xarray`, `sklearn`, `torch`, `SpectroDataset`. |
| R | Raw list, `nirs4allformats_dataset`, `matrix`, `data.frame`, optional tibble, target extraction. |
| WASM / JS | JSON lossless, typed arrays, separable metadata/provenance. |
| CLI | `probe`, `read-json`, `convert`, `scan --json`, bundle exports. |

## Product Priorities

1. Stabilize the P0 trio: JSON lossless, wide DataFrame/CSV, and `X, axis`.
2. Provide a clear diagnostic when a projection is impossible or lossy.
3. Add Parquet for large volumes and modern pipelines.
4. Expose N-D structures through `xarray`/coords without flattening them by default.
5. Add a bundle export to simplify exchanges with non-developer users.
