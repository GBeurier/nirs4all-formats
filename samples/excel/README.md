# Excel `.xlsx` / `.xls`

`.xlsx` is ZIP/XML (OOXML), `.xls` is OLE/CFB (legacy). Already in `nirs4all`.

## Samples

| File | Source | License |
|---|---|---|
| `synthetic_nirs.xlsx` | Generated locally | CC-0 | Same content as the CSV fixture (50 samples × 200 wavelengths) written to a single `spectra` sheet via `pandas.to_excel`. |
| `synthetic_multisheet_nirs.xlsx` | Generated locally | CC-0 | Compact workbook with `spectra`, `metadata` and `references` sheets joined by `sample_id`. |

## Parser hints

- Reference readers: `openpyxl` (.xlsx), `xlrd` (.xls, legacy), `pandas.read_excel`.
- Many lab transfers use multi-sheet workbooks with: `metadata` (one row per sample), `spectra` (matrix), `references` (target properties). The native reader now auto-detects these canonical sheet names and joins them by `sample_id`.
- Defensive: Excel mangles wavelength values that look like dates (e.g. "1/8" rows). The loader should round-trip numbers as numbers.
