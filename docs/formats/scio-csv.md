# Consumer Physics SCiO CSV

> **Status:** Supported · **Vendor:** Consumer Physics · **Extensions:** `.csv`

CSV exports from the Consumer Physics SCiO handheld NIR sensor and its developer
app. nirs4all-io recognises the SCiO-specific `band`-prefixed and grouped
channel layouts and is registered ahead of the generic CSV readers so SCiO
column groups are not mistaken for target-only tables.

## Instruments & software

Produced by the SCiO mobile/developer app; committed fixtures come from the
`kebasaa/SCIO-read` project. The sensor's spectral range is roughly 740–1070 nm.

## File structure

Two SCiO layouts are detected (both `.csv`):

- **Band export** — a header with at least 32 `band<wavelength>` columns
  (`band740` … `band1070`). Each row is one spectrum.
- **Developer export** — recognised by the `num_wavelengths` and
  `wavelengths_start` markers. A header row carries `sample_id` plus three
  channel groups — `spectrum_*`, `wr_raw_*` and `sample_raw_*` — optionally
  followed by an `int,…` type row, then one record per row. Lines before the
  header are collected as preamble metadata.

The axis is read from the numeric suffix of each band/channel column name.

## What nirs4all-io extracts

- **Signals** — one `SpectralRecord` per row. The band export emits a single
  `spectrum` signal (type `Unknown`); the developer export emits three signals:
  `spectrum` (`Reflectance`), `wr_raw` (`RawCounts`, unit `counts`) and
  `sample_raw` (`RawCounts`, unit `counts`).
- **Axis** — wavelength in `nm`, kind `Wavelength`, from the column-name
  suffixes (≈740–1070 nm).
- **Targets** — `Protein` and `Fat` columns map to `targets`.
- **Metadata** — preamble key/values and per-row scan/sample/device fields
  (including temperature and acquisition labels) are kept; `row_index` and a
  layout marker are recorded.
- **Provenance** — source file + SHA-256, reader name and version.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Band export (`band740`…`band1070`) | Supported | Single `spectrum` signal, type `Unknown`. |
| Developer export (grouped channels) | Supported | `spectrum`/`wr_raw`/`sample_raw`; `Protein`/`Fat` targets. |
| Calibration plate (axis-first) | Supported (via row table) | Decoded by the [row-oriented spectral table](row-spectral-table.md) reader, not this one. |

## Limitations & known gaps

- SCiO native mobile-app / project containers are not decoded.
- The plain `band*` export does not declare whether values are absorbance,
  reflectance or another processed spectrum, so its signal type stays `Unknown`.
- CSV parsing assumes comma-separated exports without quoted embedded commas.
- The axis-first SCiO calibration plate is handled by the generic row-table
  reader by design, because it is an axis-first table rather than a SCiO grouped
  export.

## Reference readers

The `kebasaa/SCIO-read` project parses the same developer/app exports;
nirs4all-io adds channel grouping, signal typing, target/metadata separation and
provenance.

## Samples & validation

Fixtures live under `samples/scio/`, covered by golden summaries in
`crates/nirs4all-io/tests/goldens/` (`scio_*`):
`scio_app_scan.csv` yields 1 `spectrum` record over a 740–1070 nm axis;
`scio_scans_from_tech_support.csv` yields 145 records with `spectrum`/`wr_raw`/
`sample_raw` signals; `scio_calibration_plate_Polypen.csv` (324–790 nm
`reflectance`) is routed to the row-table reader. The probe reports format
`scio-csv` at `Confidence::Definite`.
