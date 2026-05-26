# Spectral Matrix Exports

> **Status:** Supported · **Vendor:** Generic / Foss / Metrohm / VIAVI exports · **Extensions:** `.csv`, `.txt`

A "wide matrix" text layout where **each row is one complete spectrum** and the
axis is declared either by numeric column headers or by a dedicated
`Wavelengths:` block, optionally after a short metadata preamble. This reader
(`spectral_matrix`) handles the matrix forms that instrument software and ML
pipelines write for whole datasets.

It sits between two siblings: the [delimited table](text-readers-001.md) reader
handles the simplest case where the first line is already a numeric-header table,
and the [row-oriented spectral table](row-spectral-table.md) reader handles the
transposed point-per-row layout.

## Instruments & software

Vendor-neutral, useful for ML and bulk dataset interchange. Committed fixtures
come from Foss / WinISI text exports (including Foss XDS `barleyground`/`wheat2`
sensAIfood sets), Metrohm Vision Air CSV, VIAVI MicroNIR CSV, the sensAIfood
AuroraNIR handheld export and a Si-Ware NeoSpectra OSSL soil slice.

## File structure

The reader recognises two matrix forms:

- **`Wavelengths:` block** — a literal `Wavelengths:` line followed by a numeric
  axis line, then a header row of `p`-prefixed spectral columns (`p0`, `p1`, …)
  whose count matches the axis length, then one spectrum per row.
- **Numeric-header matrix with preamble** — leading metadata/comment lines are
  collected as key/value pairs, then a header row of at least 10 numeric
  wavelength columns (strictly ascending, first value ≥ 100) defines the axis,
  followed by one spectrum per row.

The delimiter (comma, semicolon, tab or whitespace) is detected per line.

## What nirs4all-io extracts

- **Signals** — one `SpectralRecord` per sample row, each with a single
  `absorbance` signal (type `Absorbance`).
- **Axis** — values from the `Wavelengths:` block or numeric headers; unit `nm`,
  kind `Wavelength`.
- **Targets** — non-spectral numeric columns (e.g. `protein`, `moisture`, `fat`,
  `Moisture`/`Protein`/`Year`) become `targets`.
- **Metadata** — identifier columns map to `metadata.sample_id`; the first column
  is treated as the sample id when its header is empty; preamble key/value pairs
  are preserved under `metadata.vendor`; a `row_index` is recorded.
- **Provenance** — source file + SHA-256, reader name and version.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| `Wavelengths:` block + `p`-prefixed headers (Foss/WinISI text) | Supported | Axis line drives the `nm` axis. |
| Numeric-header matrix after metadata preamble | Supported | ≥10 ascending numeric headers, first ≥ 100. |
| Metrohm Vision Air / VIAVI MicroNIR CSV | Supported | Vendor metadata preamble preserved. |
| Si-Ware NeoSpectra OSSL slice | Supported | Soil reference properties become targets. |
| Target-only report (no spectral axis) | Detected / refused | Fails with `no spectral matrix header found`; routed away from spectra. |

## Limitations & known gaps

- Target-only reports are intentionally not loaded as spectra: the committed FOSS
  DS3 and Perten report fixtures carry properties but no spectral axis, so they
  are refused until the core model gains a non-spectral report representation.
- The single emitted signal is always typed `Absorbance`; per-column signal-type
  inference is not attempted in this generic matrix path.
- Vendor preamble pairs are preserved verbatim under `metadata.vendor` rather than
  promoted to typed fields.

## Reference readers

`pandas.read_csv` and R `read.table` read the same matrices. nirs4all-io adds
the `Wavelengths:`-block and preamble handling, axis detection, target/metadata
separation and provenance.

## Samples & validation

Fixtures live under `samples/foss_winisi/`, `samples/metrohm/`,
`samples/viavi_micronir/`, `samples/csv_tsv/` and `samples/siware_neospectra/`,
covered by golden summaries in `crates/nirs4all-io/tests/goldens/`
(`spectral_matrix_*`). Representative outputs: 50 records for the synthetic
WinISI export (`Wavelengths:` block, `nm`, `protein` target) and 50 records for
Metrohm Vision Air (numeric `;` headers, `protein`/`moisture`/`fat` targets). The
probe reports format `spectral-matrix` at `Confidence::Likely`.
