# VIAVI MicroNIR

> **Status:** Supported (scoped) · **Vendor:** VIAVI Solutions (formerly JDSU) · **Extensions:** native `.sam` sample file; `.csv`, `.xlsx` (exports); native `.pri` project (blocked)

The VIAVI MicroNIR is a compact handheld NIR spectrometer. nirs4all-formats reads
its native `.sam` sample file directly, emitting one `SpectralRecord` per
acquisition with calibrated absorbance and raw single-beam signals. Its native
`.pri` project file remains a customer-only binary, so for that path
nirs4all-formats reads the CSV / XLSX spectral-matrix exports its software
produces instead — CSV through the generic
[spectral-matrix reader](row-spectral-table.md) and XLSX through the
[Excel reader](excel.md).

## Instruments & software

Produced by MicroNIR Pro / MicroNIR OnSite software for instruments such as the
MicroNIR 1700. Both committed real exports are from a University of Amsterdam
forensic study (MicroNIR 1700 drug screening), alongside a synthetic CSV matrix.

## Native `.sam` sample file

The `.sam` is a .NET-serialised container holding a single acquisition. It is
sniffed at `Confidence::Definite` by its length-prefixed `\x04MNIR` magic — this
outranks and resolves the old [Galactic SPC](galactic-spc.md) false positive that
used to misroute `.sam` files and fail. The reader emits **one** `SpectralRecord`
with **two** signals:

- **`absorbance`** — 125 calibrated channels, axis in `nm`. The file stores only
  the first/last wavelength endpoints (~908.1–1676.2 nm), so the per-channel axis
  is **derived by linear interpolation** between those endpoints.
- **`raw_single_beam`** — 128 raw detector pixels on an `Index` axis (unit
  `pixel`, `SignalType::SingleBeam`).

Rich acquisition metadata is promoted as flat fields: `sample_name`,
`instrument_serial`, `instrument_model` (`1700`), `operator`,
`integration_time_ms`, `scan_count`, `acquired_at`, `channel_count` (125),
`raw_pixel_count` (128), `format_version` (`3.1.0.0`) and the stored
`wavelength_first_nm` / `wavelength_last_nm` endpoints. Provenance carries the
`viavi_micronir_reverse_engineered_header` warning, source file and SHA-256.

## File structure

- **CSV** — a one-spectrum-per-row matrix: an optional preamble, then a header
  row whose numeric columns are the wavelength axis, preceded by a sample-id
  column. The delimiter is auto-detected.
- **XLSX** — a worksheet whose first cell can carry an axis descriptor such as
  `axis: wavelength (nm) / data: absorbance (a.u.)`; numeric wavelength columns
  follow, with sample identifiers in the first column. Multi-sheet workbooks are
  handled by the Excel reader.

## What nirs4all-formats extracts

- **Signal** — one `absorbance` signal per row/sample, axis in `nm`.
- **Axis descriptor** — the XLSX first-cell descriptor sets the axis unit/kind
  and is preserved under `metadata.axis_descriptor`.
- **Metadata** — the first-column sample identifier is promoted to
  `metadata.sample_id`.
- **Provenance** — source file + SHA-256, reader name and version.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Native `.sam` sample file | Supported | Native reader; `\x04MNIR` sniff, absorbance + raw single-beam signals. |
| MicroNIR CSV matrix export | Supported | Read via the spectral-matrix reader. |
| MicroNIR `.xlsx` export | Supported | Read via the Excel reader; real MicroNIR 1700 sets committed. |
| Native `.pri` project | Blocked | Customer-only binary project format. |

## Limitations & known gaps

- The native `.sam` `absorbance` axis is a **linear interpolation of the stored
  first/last endpoints** — the vendor file stores only those endpoints, not a
  per-pixel wavelength vector.
- Native `.pri` reverse engineering is out of scope; the exports already cover
  the spectral content.
- Vendor metadata beyond the exported worksheet labels is not surfaced.
- No paired reference-reader comparison exists for the native project format.

## Reference readers

The exports are equally readable with `pandas` (CSV) and `openpyxl` / R `readxl`
(XLSX); nirs4all-formats adds axis detection, signal typing and provenance. No open
reader exists for the native `.pri` project.

## Samples & validation

Fixtures under `samples/viavi_micronir/` are golden-backed / probe-locked:
`synthetic_micronir.sam` (1 record, 5 absorbance channels + 6 raw pixels; a
byte-accurate fixture for the native reader golden), `synthetic_micronir.csv`
(20 records, 200-point `nm` axis), and the real UvA forensic sets
`micronir_forensic_K_avg.xlsx` (88 records, 125 points, ketamine) and
`micronir_forensic_T_avg.xlsx` (71 records, 125 points, THC). Real native `.sam`
vendor files are local-only under `samples_local/viavi_micronir/` (private,
licence TBD).
