# Bruker OPUS `.dpt` (Data Point Table)

Two-column ASCII export from OPUS software (`Save → Save as → Data Point Table`). Columns: wavenumber, intensity. Trivial to parse with any text loader.

## Samples

| File | Size | Source | License | Notes |
|---|---|---|---|---|
| `RS-1.dpt` | 67 KB | [`ropensci/lightr`](https://github.com/ropensci/lightr/blob/main/inst/testdata/RS-1.dpt) | GPL-3 | A real `.dpt` export. Headerless, comma-separated, wavenumber-decreasing. |
| `synthetic.dpt` | ~5 KB | Generated locally | CC-0 | Synthetic NIR spectrum, 200 points, wavenumber-decreasing (`{cm⁻¹}, {absorbance}`). Useful as a known-good shape reference. |

## Parser hints

- Delimiter varies: comma, tab, or whitespace. Auto-detect.
- X axis is **wavenumber (cm⁻¹), decreasing** — typical FTIR convention.
- Reference readers: any text loader works. `pandas.read_csv(..., header=None)` suffices.
