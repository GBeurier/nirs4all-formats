# Excel Spectral Tables

> **Status:** Supported (scoped) · **Vendor:** Generic / lab & handheld exports · **Extensions:** `.xlsx`, `.xlsm`

Spreadsheet workbooks that store a NIRS dataset as a spectral table — one row per
sample, numeric wavelength headers across the columns. nirs4all-formats reads the
OOXML workbook family (`.xlsx` and `.xlsm`) through the pure-Rust `calamine`
crate; legacy OLE `.xls` is not yet supported.

## Instruments & software

Vendor-neutral; common as an export from lab software and handheld NIR apps.
Committed fixtures include synthetic single-sheet and multi-sheet workbooks, a
macro-compatible `.xlsm`, and real forensic handheld exports from SCiO, NIRone,
Si-Ware NeoSpectra and VIAVI MicroNIR.

## File structure

A workbook is detected by the ZIP/OOXML magic (`PK\x03\x04`) plus the `.xlsx` /
`.xlsm` extension. The reader prefers a worksheet named `spectra` and otherwise
falls back to the first worksheet. The selected sheet has:

- one header row;
- numeric wavelength headers for the spectral columns;
- optional identifier columns (`sample`, `sample_id`, `id`);
- optional numeric target columns (e.g. `protein`);
- an optional first-cell axis descriptor such as
  `axis: wavelength (nm) / data: absorbance (a.u.)`, in which case the first
  column holds sample IDs.

Optional sibling worksheets named `metadata`/`meta`/`samples` and
`references`/`reference`/`targets` are joined to the spectral rows by
`sample_id`.

## What nirs4all-formats extracts

- **Signals** — one `SpectralRecord` per non-empty data row. The signal name and
  type are inferred from the axis descriptor's `data:` label (e.g. `absorbance`,
  `reflectance`, `raw`); absent a descriptor the signal defaults to `absorbance`.
- **Axis** — values from the numeric headers. The descriptor selects the axis:
  `wavenumber`/`cm-1` gives kind `Wavenumber` (`cm-1`), otherwise kind
  `Wavelength` (`nm`). The signal unit is taken from the descriptor's parentheses
  when present.
- **Targets** — numeric non-spectral columns become `targets`; the `references`
  sheet contributes additional numeric targets joined by `sample_id`.
- **Metadata** — identifier columns map to `metadata.sample_id`; other text
  columns are kept; `sheet`, `row_index` and the `axis_descriptor` are recorded;
  the `metadata` sheet contributes extra fields joined by `sample_id`.
- **Provenance** — source file + SHA-256, reader name and version.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| `.xlsx` numeric-header spectral table | Supported | Single `spectra`/first sheet. |
| `.xlsm` macro-compatible workbook | Supported | Read as OOXML; macros ignored. |
| Multi-sheet workbook with `metadata` / `references` | Supported | Auxiliary sheets joined by `sample_id`. |
| Axis descriptor in first cell | Supported | Drives axis kind/unit and signal label/unit. |
| Metadata-only workbook (methods/settings, no matrix) | Detected / refused | Fails with `no numeric spectral headers found`. |
| Legacy `.xls` (OLE) | Blocked | Not handled by the OOXML path; samples still wanted. |

## Limitations & known gaps

- Legacy `.xls` OLE workbooks, caller-selected non-canonical sheet names, and
  workbooks where Excel has coerced wavelengths into dates are not handled. The
  reader is intentionally limited to numeric spectral headers so malformed lab
  transfers fail clearly instead of silently producing a shifted axis.
- Metadata-only companion workbooks (e.g. sensAIfood AuroraNIR and Foss XDS
  method/settings sheets) are explicit refusals because they carry no spectral
  matrix.
- A workbook that mixes several spectral sheets is read as the single chosen
  sheet only; arbitrary multi-table layouts are out of scope.

## Reference readers

`calamine` (the underlying Rust engine), `openpyxl`, `pandas.read_excel` and R
`readxl` read the same workbooks. nirs4all-formats adds sheet selection, axis-descriptor
parsing, auxiliary-sheet joins, signal typing and provenance.

## Samples & validation

Fixtures live under `samples/excel/`, `samples/siware_neospectra/` and
`samples/viavi_micronir/`, covered by golden summaries in
`crates/nirs4all-formats/tests/goldens/` (`excel_*`). Representative outputs:
`synthetic_nirs.xlsx` and `synthetic_nirs_macro_compatible.xlsm` each yield 50
records over a 200-point `nm` axis with an `absorbance` signal and a `protein`
target; `scio_forensic_P_avg.xlsx` yields 71 `raw` records over 331 points. Probe
tests lock the synthetic `.xlsx`/`.xlsm` workbooks plus the real SCiO, NIRone,
NeoSpectra and MicroNIR workbooks, and lock the AuroraNIR / Foss XDS metadata-only
workbooks as refusals.
