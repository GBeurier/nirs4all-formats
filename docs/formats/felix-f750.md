# Felix Instruments F-750

> **Status:** Supported (scoped) · **Vendor:** Felix Instruments / CID Bio-Science · **Extensions:** `.csv`

The Felix Instruments (CID Bio-Science) **F-750 Produce Quality Meter** is a
handheld VIS-NIR spectrometer (ZEISS MMS1 silicon photodiode array, ~310-1100 nm,
3 nm sampling) used for non-destructive fruit dry-matter and internal-quality
models. Its desktop **DataViewer** software exports a wide CSV, so — like
[VIAVI MicroNIR](viavi-micronir.md) and [Si-Ware NeoSpectra](siware-neospectra.md)
— there is **no dedicated F-750 reader**: the export is decoded by the generic
[`csv_like`](text-readers-001.md) wide-table reader.

## Instruments & software

F-750 Produce Quality Meter handheld; F-750 DataViewer desktop export software.

## File structure

DataViewer "Export" produces a comma-delimited wide table: leading
identifier / metadata / target columns, then numeric wavelength columns (`nm`),
one spectrum per row. The canonical public example is the Mango DMC dataset
(Anderson et al. 2020), whose columns are `Set, Season, Region, Date, Type,
Cultivar, Pop, Temp, DM` followed by 285-1200 nm in 3 nm steps.

## What nirs4all-formats extracts

- **Signals** — `absorbance` over a `nm` wavelength axis, one record per spectrum
  row (the published export is `log(1/R)` absorbance).
- **Targets** — numeric non-spectral columns (`DM` dry-matter %, `Pop`, numeric
  `Season`) are preserved as `targets`.
- **Metadata** — text columns (`Set`, `Region`, `Date`, `Type`, `Cultivar`,
  `Temp`) and `row_index` are preserved as `metadata`.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| DataViewer wide absorbance CSV | Supported | Via `csv_like`; one record per row, `nm` axis, `DM` target. |
| Raw-Spectra (reflectance) CSV | Planned | Same wide layout; reflectance signal type not yet asserted by a fixture. |
| Interpolated-Spectra (2nd-derivative) CSV | Planned | Derivative export; no fixture yet. |
| Native on-device / SD store | Blocked | No fixture; measurements are held on-device before DataViewer export. |

## Limitations & known gaps

- Only the wide absorbance CSV export is fixture-backed; the reflectance and
  derivative export modes and the native on-device format are not yet validated.
- The generic reader types the signal as `absorbance` from the wide-header
  layout; it does not read the DataViewer "export mode" to distinguish
  reflectance vs absorbance vs derivative.

## Reference readers

Generic CSV tooling (`pandas`, `read.table`); no dedicated reference reader.

## Samples & validation

`samples/felix_f750/mango_dmc_f750_slice.csv` is golden-backed in
`crates/nirs4all-formats/tests/goldens/` (`csv_felix_f750_mango_slice`, 26 records,
306-point `nm` axis, 285-1200 nm). It is a CC-BY-4.0 slice of the Mango DMC
dataset (Anderson, Walsh & Subedi 2020, Mendeley `10.17632/46htwnp833.1`).
