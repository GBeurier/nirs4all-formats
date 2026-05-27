# Parquet NIRS Tables

> **Status:** Supported · **Vendor:** Apache / generic · **Extensions:** `.parquet` · **Feature flag:** `fmt-parquet`

Native Rust reader for canonical tabular NIRS datasets stored as Apache Parquet,
the columnar format used for efficient internal distribution of wide spectral
tables. A spectral Parquet table has one row per sample and numeric
wavelength-named columns.

## Instruments & software

Vendor-neutral; written by Arrow / `pyarrow`, `fastparquet`,
`pandas.to_parquet` and the `nirs4all` `ParquetLoader`. Useful as a compact
distribution format for NIRS datasets.

## File structure

Detected by the `PAR1` magic plus the `.parquet` extension and opened through
Arrow (with Zstd support). A table is accepted as spectral when:

- its spectral columns are **named by numeric wavelength values** and typed
  `float32` or `float64`;
- there are **at least 8** such columns;
- the resulting axis is **strictly ascending** (these two checks reject generic
  Parquet files).

Non-spectral numeric columns become targets, and a `sample_id` / `sample` / `id`
UTF-8 column becomes the identifier.

## What nirs4all-formats extracts

- **Signals** — one `SpectralRecord` per table row, each with a single
  `absorbance` signal (type `Absorbance`).
- **Axis** — values from the numeric column names; unit `nm`, kind `Wavelength`.
- **Targets** — numeric non-spectral columns (`float32`/`float64`/`int32`/`int64`,
  e.g. `protein`) become `targets`; nulls are preserved as `null`.
- **Metadata** — the `sample_id`/`sample`/`id` string column maps to
  `metadata.sample_id`; `row_index` and a `parquet` summary (spectral/target
  column counts) are recorded.
- **Provenance** — source file + SHA-256, reader name and version.

The same decode path serves both `read_path` (filesystem) and `read_bytes` /
`open_bytes` (in-memory), so the reader is sidecar-free and works without a
resolver.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Numeric-wavelength spectral table (float32/float64) | Supported | ≥ 8 ascending wavelength columns. |
| Zstd-compressed Parquet | Supported | Default committed fixture is Zstd. |
| Numeric target + `sample_id` columns | Supported | Targets and identifier joined per row. |
| In-memory decode (`open_bytes`) | Supported | Same code path as filesystem reads. |
| Non-spectral Parquet (e.g. `alltypes_plain.parquet`) | Detected / refused | Refused as "not a NIRS spectral table". |

## Limitations & known gaps

- Schema metadata is not yet read for explicit units or signal type, so the
  signal is always typed `Absorbance` on a `nm` wavelength axis.
- Compression variants beyond the committed Zstd fixture are added as needed.
- Projection-based reading for very wide tables is not implemented; the reader
  materialises all spectral columns per batch.

## Reference readers

`pyarrow.parquet`, `fastparquet`, `pandas.read_parquet` and the `nirs4all`
`ParquetLoader` read the same tables. nirs4all-formats adds the spectral-schema
validation, axis construction, target/metadata separation and provenance.

## Samples & validation

Fixtures live under `samples/parquet/`, covered by golden summaries in
`crates/nirs4all-formats/tests/goldens/` (`parquet_*`):
`synthetic_nirs.parquet` yields 50 records over 200 wavelength columns with a
`protein` target, and `alltypes_plain.parquet` (the Apache sample) is refused as
non-spectral. The probe reports format `parquet-container` at
`Confidence::Likely`.
