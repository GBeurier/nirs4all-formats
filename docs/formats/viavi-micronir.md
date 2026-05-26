# VIAVI MicroNIR

> **Status:** Supported (scoped) · **Vendor:** VIAVI Solutions (formerly JDSU) · **Extensions:** `.csv`, `.xlsx` (exports); native `.pri` project (blocked)

The VIAVI MicroNIR is a compact handheld NIR spectrometer. Its native `.pri`
project file is a customer-only binary, so nirs4all-io reads the CSV / XLSX
spectral-matrix exports its software produces — CSV through the generic
[spectral-matrix reader](row-spectral-table.md) and XLSX through the
[Excel reader](excel.md).

## Instruments & software

Produced by MicroNIR Pro / MicroNIR OnSite software for instruments such as the
MicroNIR 1700. Both committed real exports are from a University of Amsterdam
forensic study (MicroNIR 1700 drug screening), alongside a synthetic CSV matrix.

## File structure

- **CSV** — a one-spectrum-per-row matrix: an optional preamble, then a header
  row whose numeric columns are the wavelength axis, preceded by a sample-id
  column. The delimiter is auto-detected.
- **XLSX** — a worksheet whose first cell can carry an axis descriptor such as
  `axis: wavelength (nm) / data: absorbance (a.u.)`; numeric wavelength columns
  follow, with sample identifiers in the first column. Multi-sheet workbooks are
  handled by the Excel reader.

## What nirs4all-io extracts

- **Signal** — one `absorbance` signal per row/sample, axis in `nm`.
- **Axis descriptor** — the XLSX first-cell descriptor sets the axis unit/kind
  and is preserved under `metadata.axis_descriptor`.
- **Metadata** — the first-column sample identifier is promoted to
  `metadata.sample_id`.
- **Provenance** — source file + SHA-256, reader name and version.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| MicroNIR CSV matrix export | Supported | Read via the spectral-matrix reader. |
| MicroNIR `.xlsx` export | Supported | Read via the Excel reader; real MicroNIR 1700 sets committed. |
| Native `.pri` project | Blocked | Customer-only binary project format. |

## Limitations & known gaps

- Native `.pri` reverse engineering is out of scope; the exports already cover
  the spectral content.
- Vendor metadata beyond the exported worksheet labels is not surfaced.
- No paired reference-reader comparison exists for the native project format.

## Reference readers

The exports are equally readable with `pandas` (CSV) and `openpyxl` / R `readxl`
(XLSX); nirs4all-io adds axis detection, signal typing and provenance. No open
reader exists for the native `.pri` project.

## Samples & validation

Fixtures under `samples/viavi_micronir/` are golden-backed / probe-locked:
`synthetic_micronir.csv` (20 records, 200-point `nm` axis), and the real UvA
forensic sets `micronir_forensic_K_avg.xlsx` (88 records, 125 points, ketamine)
and `micronir_forensic_T_avg.xlsx` (71 records, 125 points, THC).
