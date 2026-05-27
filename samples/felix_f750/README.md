# Felix Instruments F-750 (handheld VIS-NIR)

CSV export from the Felix Instruments / CID Bio-Science **F-750 Produce Quality
Meter**, a handheld VIS-NIR spectrometer (ZEISS MMS1 silicon photodiode array,
~310-1100 nm, 3 nm sampling) used in the field for non-destructive fruit
dry-matter / internal-quality models. The F-750 desktop **DataViewer** software
exports measurements as a wide CSV: leading identifier / metadata / target
columns followed by numeric wavelength columns, one spectrum per row. Like the
VIAVI MicroNIR and Si-Ware NeoSpectra wide exports, this layout is decoded by the
generic `csv_like` reader rather than a dedicated F-750 reader.

## Samples

| File | Size | Source | License | Notes |
|---|---|---|---|---|
| `mango_dmc_f750_slice.csv` | ~76 KB | 26-record deterministic slice (header + every 450th data row) of `NAnderson2020MendeleyMangoNIRData.csv` from the Mango DMC dataset — Anderson, Walsh & Subedi (2020), [Mendeley Data v1, doi:10.17632/46htwnp833.1](https://data.mendeley.com/datasets/46htwnp833/1) (mirror: [github.com/spectral-datasets/mango-dmc](https://github.com/spectral-datasets/mango-dmc)) | **CC-BY-4.0** | Real F-750 mango mesocarp absorbance spectra, 285-1200 nm (306-point `nm` axis, 3 nm step). 9 leading columns (`Set, Season, Region, Date, Type, Cultivar, Pop, Temp, DM`) then wavelength columns. `DM` = dry-matter content (%) target. Read by the wide `csv_like` reader, one record per row. |

## Parser hints

- Plain comma-delimited wide table: the first 9 columns are
  identifiers / metadata / targets, the remaining 306 columns are numeric
  wavelength headers (`285`, `288`, ..., `1200`). One spectrum per data row.
- The signal is **absorbance** (`log(1/R)`) over a `nm` wavelength axis. The
  F-750 sensor range is ~310-1100 nm; the published export pads the table to
  285-1200 nm, so values at the spectral extremes can be `0`.
- Numeric non-spectral columns (`DM`, `Pop`, numeric `Season`) are promoted to
  `targets`; text columns (`Set`, `Region`, `Date`, `Type`, `Cultivar`, `Temp`)
  to `metadata`. `DM` (dry-matter content, %) is the modelling target. Some rows
  carry a textual `Season` (e.g. `Ext 4`) which then falls back to `metadata`.
- DataViewer also offers Raw-Spectra (reflectance), Interpolated-Spectra
  (2nd-derivative absorbance) and Measurements export modes, plus a native
  on-device store; only the wide absorbance CSV is fixture-backed so far.
