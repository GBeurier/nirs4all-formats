# nirs4allformats

R binding for [`nirs4all-formats`](../../..), the Rust-first low-level reader library
for NIRS and spectroscopy file formats. It auto-detects each file by content,
decodes it through a single Rust registry, and surfaces the canonical,
provenance-tracked records with R-native ergonomics: nested lists, a spectral
matrix, a wide data frame, or a tibble.

`nirs4all-formats` does no modelling itself; it produces the records that the
`nirs4all` modelling library consumes.

## Parsers live only in Rust

This package is a thin binding. It **never reimplements any format parsing** in
R. Detection and decoding happen entirely in the Rust core; the R layer only
dispatches calls and reshapes the result into R objects. Adding support for a
new format means writing a new Rust reader, not new R code.

## Installation

```sh
R CMD INSTALL bindings/r/nirs4allformats
```

When [Cargo](https://www.rust-lang.org/tools/install) (Rust's package manager)
is on `PATH` at install time, the package compiles a native extendr static
library from `src/rust/` and calls the Rust core directly. Without Cargo, the
package still installs and falls back to invoking the `nirs4all-formats` command-line
tool (see *Transport* below).

The optional `tibble` package enables `nirs4allformats_as_tibble()`.

## Transport: native vs CLI

| Path | When it is used | Capabilities |
|------|-----------------|--------------|
| **Native (extendr)** | Built at install time with Cargo on `PATH` | All functions, including in-memory `nirs4allformats_open_bytes()` and `nirs4allformats_open_with_sidecars()` |
| **CLI fallback** | Native library absent | File-path reads, `nirs4allformats_probe_path()`, `nirs4allformats_walk_path()` |

Use `nirs4allformats_native_available()` to check which path is active.

CLI resolution order:

1. `NIRS4ALL_FORMATS_CLI` environment variable, if set (whitespace-split into a
   command plus arguments);
2. a `nirs4all-formats` binary found on `PATH`;
3. in a source checkout, `cargo run -p nirs4all-formats-cli`.

The in-memory byte paths are **native-only** and raise an error when the native
library is not loaded.

## Worked example

```r
library(nirs4allformats)

# Load a file into a flat, rectangular dataset.
ds <- nirs4allformats_open_dataset("samples/csv_tsv/synthetic_nirs.csv")

# The spectral matrix: rows = samples, columns = wavelengths.
m <- as.matrix(ds)
dim(m)              # e.g. 50 x 200
ds$wavelengths[1:5] # axis coordinates
ds$axis_unit        # e.g. "nm"
ds$signal_type      # selected signal type

# A wide data frame: sample_id, target columns, then x_<wavelength> columns.
df <- as.data.frame(ds)
names(df)[1:4]

# Same wide table as a tibble (requires the 'tibble' package).
tb <- nirs4allformats_as_tibble(ds)

# Select a specific signal channel by name (otherwise auto-selected).
ds_abs <- nirs4allformats_open_dataset("spectrum.dx", signal = "absorbance")
```

### Lower-level access

```r
# Lossless records exactly as the Rust core emits them.
records <- nirs4allformats_open_records("samples/csv_tsv/synthetic_nirs.csv")
records[[1]]$provenance$format
names(records[[1]]$signals)

# Which readers recognize a file (no full parse).
nirs4allformats_probe_path("samples/csv_tsv/synthetic_nirs.csv")[[1]]$format

# Recursively scan a directory for per-file detection outcomes.
entries <- nirs4allformats_walk_path("samples/asd")
entries[[1]]$status   # "parsed", "error", or "unsupported"
entries[[1]]$format

# In-memory decoding (native backend only).
if (nirs4allformats_native_available()) {
  raw_bytes <- readBin("spectrum.dx", "raw", n = file.info("spectrum.dx")$size)
  recs <- nirs4allformats_open_bytes("spectrum.dx", raw_bytes)
}
```

## The `nirs4allformats_dataset` object

`nirs4allformats_open_dataset()` returns an object of class `nirs4allformats_dataset`, a
named list with:

| Field | Description |
|-------|-------------|
| `x` | Numeric matrix of spectra, `n_samples` x `n_wavelengths`. |
| `wavelengths` | Numeric vector of axis coordinates. |
| `targets` | `data.frame` of reference values (one column per target key). |
| `sample_ids` | Character vector of per-row identifiers. |
| `metadata` | List of per-record metadata lists (preserved verbatim). |
| `signal_type` | Signal type of the selected channel. |
| `axis_unit` | Unit string of the spectral axis (e.g. `"nm"`). |
| `formats` | Source format per row. |

All records must share the same spectral axis, so this object is intended for a
homogeneous set of spectra. For heterogeneous or N-dimensional data, work from
`nirs4allformats_open_records()` directly.

When `signal = NULL`, the channel is auto-selected per record: first the signal
whose `signal_type` matches the record's `signal_type`; otherwise the first
present of `reflectance`, `absorbance`, `transmittance`, `signal`; otherwise the
alphabetically first signal. Sample IDs come from `metadata$sample_id` when
present, else from the source-file basename and 0-based row index.

## Exported API

| Function | Purpose |
|----------|---------|
| `nirs4allformats_open_records(path)` | Lossless records as nested R lists. |
| `nirs4allformats_open_dataset(path, signal = NULL)` | Flat `nirs4allformats_dataset`. |
| `as.matrix(dataset)` | Spectral matrix from a dataset. |
| `as.data.frame(dataset)` | Wide data frame from a dataset. |
| `nirs4allformats_as_tibble(dataset)` | Dataset as a tibble. |
| `nirs4allformats_probe_path(path)` | Ordered candidate readers for a file. |
| `nirs4allformats_walk_path(path, ...)` | Recursive per-file scan outcomes. |
| `nirs4allformats_open_bytes(name, bytes)` | Decode in-memory bytes (native only). |
| `nirs4allformats_open_with_sidecars(name, bytes, sidecars)` | Decode bytes plus companion files (native only). |
| `nirs4allformats_native_available()` | Whether the native backend is loaded. |
| `nirs4allformats_version()` | Binding version string. |

See `?nirs4allformats` and the per-function help pages (e.g. `?nirs4allformats_open_dataset`)
for full details.

## License

MIT. See [`LICENSE`](LICENSE).
