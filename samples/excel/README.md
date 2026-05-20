# Excel `.xlsx` / `.xls`

`.xlsx` is ZIP/XML (OOXML), `.xls` is OLE/CFB (legacy). Already in `nirs4all`.

## Samples

| File | Size | Source | License | Notes |
|---|---|---|---|---|
| `synthetic_nirs.xlsx` | ~30 KB | Generated locally | CC-0 | Same content as the CSV fixture (50 samples × 200 wavelengths) written to a single `spectra` sheet via `pandas.to_excel`. |
| `synthetic_multisheet_nirs.xlsx` | ~40 KB | Generated locally | CC-0 | Compact workbook with `spectra`, `metadata` and `references` sheets joined by `sample_id`. |
| `scio_forensic_P_avg.xlsx` | 212 KB | [`Figshare 21252300`](https://doi.org/10.21942/uva.21252300) (`P_Avg_Scio.xlsx`) | **CC-BY-4.0** (UvA, Kranenburg et al. 2022) | Real Consumer Physics **SCiO** (740-1070 nm) averaged-per-sample export of 71 phenacetin/cocaine forensic rows — single sheet, `axis: wave` row + wavelength columns + per-sample rows. Smallest real-vendor Excel fixture currently shipping; exercises the "wide-spectra-with-axis-row" Excel layout that miniaturised handheld vendors export. |
| `nirone_forensic_T_avg.xlsx` | 132 KB | Same Figshare record (`T_Avg_NIRONE.xlsx`) | CC-BY-4.0 | Real Spectral Engines **NIRone** (~1.55-1.95 µm) export, same axis-row layout. Covers the upper end of the handheld NIR wavelength range. |

## Parser hints

- Reference readers: `openpyxl` (.xlsx/.xlsm), `xlrd` (.xls, legacy), `pandas.read_excel`.
- Many lab transfers use multi-sheet workbooks with: `metadata` (one row per sample), `spectra` (matrix), `references` (target properties). The native reader now auto-detects these canonical sheet names and joins them by `sample_id`.
- Handheld-vendor exports (`scio_forensic_*.xlsx`, `nirone_forensic_*.xlsx`) follow a flatter pattern: a single sheet, first row = literal `axis: wave` + numeric wavelength columns, rows below = sample ID + measurements. The header heuristic "first cell `axis: wave` ⇒ row 1 is the wavelength axis" handles them.
- Defensive: Excel mangles wavelength values that look like dates (e.g. "1/8" rows). The loader should round-trip numbers as numbers.
