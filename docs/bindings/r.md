# R Binding

The R package (`nirs4allio`) is a thin wrapper over the Rust core. Parser logic
must stay in Rust: the binding only dispatches calls and converts the result
into R-native shapes. It exposes two layers — raw records and a flat dataset —
plus directory walking and in-memory decoding.

## Installation

```sh
R CMD INSTALL bindings/r/nirs4allio
```

When Cargo is on `PATH` at install time the package compiles a native extendr
static library and calls Rust directly; otherwise it installs and falls back to
the `nirs4all-io` CLI (see *Transport*). The optional `tibble` package enables
`nirs4allio_as_tibble()`.

## Raw access

Records exactly as the Rust core emits them, as nested R lists:

- `nirs4allio_open_records(path)`: normalized records (one named list per
  record), the lossless mirror of the Rust `SpectralRecord`;
- `nirs4allio_open_bytes(name, bytes)`: decode an in-memory `raw` buffer
  (native backend only; sidecar formats are rejected);
- `nirs4allio_open_with_sidecars(name, bytes, sidecars)`: decode bytes plus a
  named list of companion `raw` vectors (native backend only);
- `nirs4allio_probe_path(path)`: ordered candidate readers without a full parse;
- `nirs4allio_walk_path(path, ...)`: recursive per-file outcomes.

Each record from `nirs4allio_open_records()` is a named list with `signals`
(named channels, each with `values`, `shape`, `dims`, optional `coords`,
`signal_type`, `unit`, `role`, `source` and an `axis` of `values`/`unit`/`kind`/
`order`), `signal_type`, `targets`, `metadata`, `provenance` (reader name/version,
per-source SHA-256, format, schema version, warnings) and `quality_flags`.
Nothing is reshaped, aligned or dropped.

## Flat dataset

`nirs4allio_open_dataset(path, signal = NULL)` projects one signal per record
into a rectangular, R-friendly object of class `nirs4allio_dataset`. All records
must share the same spectral axis (an error is raised otherwise), so it targets
a homogeneous set of spectra. The object is a named list with:

| Field | Description |
|-------|-------------|
| `x` | Numeric matrix of spectra, `n_samples` x `n_wavelengths`. |
| `wavelengths` | Numeric vector of axis coordinates. |
| `targets` | `data.frame` of reference values (one column per target key). |
| `sample_ids` | Character vector of per-row identifiers. |
| `metadata` | List of per-record metadata lists, preserved verbatim. |
| `signal_type` | Signal type of the selected channel. |
| `axis_unit` | Unit string of the spectral axis (e.g. `"nm"`). |
| `formats` | Source format per row. |

### Signal selection

When `signal` is `NULL` the channel is chosen per record in this order:

1. the first signal whose `signal_type` equals the record-level `signal_type`;
2. otherwise the first present of `"reflectance"`, `"absorbance"`,
   `"transmittance"`, `"signal"`;
3. otherwise the alphabetically first signal name.

Passing an explicit `signal` name selects that channel and errors if a record
lacks it.

### Targets and metadata

Reference values under each record's `targets` are gathered into the `targets`
`data.frame` (missing values become `NA`). The full per-record metadata lists
are preserved verbatim in `metadata`. Each row's `sample_id` is taken from
`metadata$sample_id` when present; otherwise it is derived from the source-file
basename and 0-based row index (`"<basename>:<i>"`), falling back to
`"record:<i>"` when no source path is known.

### Projections

S3 methods and a tibble helper turn an `nirs4allio_dataset` into common R
shapes:

- `as.matrix(dataset)`: the `n_samples` x `n_wavelengths` spectral matrix;
- `as.data.frame(dataset)`: a wide frame — `sample_id`, then target columns,
  then one `x_<wavelength>` column per axis value;
- `nirs4allio_as_tibble(dataset)`: the same wide table as a tibble (requires the
  `tibble` package).

## Walking a directory

`nirs4allio_walk_path(path, max_depth = NULL, include_hidden = FALSE,
follow_symlinks = FALSE, include_unsupported = FALSE)` recursively visits a
directory (or a single file) and returns a list of per-file outcomes. Each entry
carries a `status` (`"parsed"`, `"error"`, or `"unsupported"`) and, when
detected, a `format`. Only sniffing happens here; no file is fully decoded.

## Bytes and sidecars (native only)

`nirs4allio_open_bytes(name, bytes)` decodes a `raw` buffer through the registry
without touching the filesystem; `name` (with its extension) drives sniffing and
provenance. Formats that split a measurement across companion files — ENVI
Standard cubes (`.img` + `.hdr`), ERDAS LAN, and similar — use
`nirs4allio_open_with_sidecars(name, bytes, sidecars)`, where `sidecars` is a
named list of `raw` vectors keyed by paths relative to the primary file
(e.g. `"cube.hdr"`). Both paths require the native extendr library and raise an
error when it is absent; they have no CLI fallback.

```r
read_raw <- function(p) readBin(p, "raw", n = file.info(p)$size)
records <- nirs4allio_open_with_sidecars(
  "cube.img",
  read_raw("cube.img"),
  list("cube.hdr" = read_raw("cube.hdr"))
)
```

## Transport

- the native extendr static library (built by `R CMD INSTALL` with Cargo on
  `PATH`) is used when present;
- otherwise the bridge calls the `nirs4all-io` CLI: `NIRS4ALL_IO_CLI` can point
  to a prebuilt binary (whitespace-split into command plus arguments), a
  `nirs4all-io` binary on `PATH` is used if found, and in a source checkout it
  falls back to `cargo run -p nirs4all-io-cli`.

Use `nirs4allio_native_available()` to tell which path is active.
`nirs4allio_open_bytes()` and `nirs4allio_open_with_sidecars()` are available
only on the native path.

## Example

```r
library(nirs4allio)

ds <- nirs4allio_open_dataset("samples/csv_tsv/synthetic_nirs.csv")
dim(as.matrix(ds))         # samples x wavelengths
df <- as.data.frame(ds)    # sample_id + targets + x_<wavelength>
tb <- nirs4allio_as_tibble(ds)

# Lower-level: lossless records and detection.
records <- nirs4allio_open_records("samples/csv_tsv/synthetic_nirs.csv")
records[[1]]$provenance$format
nirs4allio_probe_path("samples/csv_tsv/synthetic_nirs.csv")[[1]]$format
```
