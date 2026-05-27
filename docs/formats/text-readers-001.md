# Delimited Spectral Tables (one spectrum per row)

> **Status:** Supported · **Vendor:** Generic / instrument & software exports · **Extensions:** `.csv`, `.tsv`, `.txt`

Many tools export a whole dataset as a single delimited table where **each data
row is one complete spectrum** and the **column headers are numeric wavelengths**.
This reader (`csv_like`) handles that "wide" orientation. It is the complement of
the [row-oriented spectral table](row-spectral-table.md) reader, which handles the
transposed layout (one spectral point per row, axis in the first column).

## Instruments & software

This is a vendor-neutral text reader and the simplest base path for external
imports. It is the format produced by `pandas.DataFrame.to_csv`, R `write.table`,
and many lab/handheld export buttons that emit a wavelength-header matrix.
Committed fixtures are synthetic NIRS tables in comma, semicolon and tab variants.

## File structure

A single header row followed by one row per sample. The header mixes:

- **numeric wavelength headers** (e.g. `400`, `402`, …) that define the axis;
- optional identifier columns (`sample`, `sample_id`, `id`);
- optional non-spectral columns (numeric targets or text metadata).

The delimiter is auto-detected per file (comma, semicolon or tab). For `.csv`,
all three delimiters are considered; `.tsv` is tab-only; `.txt` uses the
detected delimiter with a slightly stricter threshold to avoid false positives.

## What nirs4all-formats extracts

- **Signals** — one `SpectralRecord` per data row, each with a single signal
  named `signal`, typed as `Absorbance`. Values come from the numeric-header
  columns in header order.
- **Axis** — built from the numeric column headers; unit `nm`, kind
  `Wavelength`. The native header order is preserved.
- **Targets** — non-spectral numeric columns are stored as `targets` under their
  header name.
- **Metadata** — identifier columns map to `metadata.sample_id`; other non-numeric
  columns are kept under their header name; a `row_index` is recorded.
- **Provenance** — source file + SHA-256, reader name and version.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Comma `.csv` with numeric headers | Supported | Strongest match (`Confidence::Likely`). |
| Semicolon `.csv` | Supported | Semicolon delimiter auto-detected. |
| Tab `.tsv` / `.txt` | Supported | Tab and whitespace-delimited tables. |
| Mixed spectral + target/metadata columns | Supported | Numeric extras become targets; text extras become metadata. |

## Limitations & known gaps

- Parsing is intentionally narrow: the header **must** contain numeric spectral
  columns. Tables without numeric headers fail explicitly with
  `no numeric spectral headers found` rather than guessing an axis.
- Target-only reports (properties but no spectral axis) are not loaded as
  spectra; the FOSS DS3 / Perten report fixtures are refused here by design.
- The axis-first orientation (point-per-row) belongs to the
  [row-oriented spectral table](row-spectral-table.md) reader, and one-spectrum
  matrices fronted by a `Wavelengths:` block or `p`-prefixed headers route to the
  [spectral matrix](spectral-matrix.md) reader instead.
- All non-identifier signals are typed `Absorbance`; per-column signal-type
  inference is not attempted in this generic path.

## Reference readers

`pandas.read_csv` and R `read.table` read the same exports; the `nirs4all`
`CSVLoader` consumes them in the modelling library. nirs4all-formats adds delimiter
detection, axis construction, target/metadata separation and provenance.

## Samples & validation

Fixtures live under `samples/csv_tsv/` (synthetic NIRS in comma, tab and
semicolon form) and are covered by golden summaries in
`crates/nirs4all-formats/tests/goldens/` (`csv_synthetic*`). Each fixture yields 50
records over a 200-point `nm` axis with a `protein` target and a `sample_id`
metadata column. The probe reports format `delimited-text` at
`Confidence::Likely` for a direct numeric header.
