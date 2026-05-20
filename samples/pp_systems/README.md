# PP Systems UniSpec SC / DC

`.SPT` (single channel) and `.SPU` (dual channel) ASCII formats. Limited literature; SPECCHIO claims support.

## Samples

| File | Notes |
|---|---|
| `synthetic_unispec.SPT` | Synthetic UniSpec SC export with header metadata + `Wavelength,DN_white,DN_target,Reflectance` columns. CC-0. Covered by semantic tests and golden summary. |
| `synthetic_unispec_dc.SPU` | Synthetic UniSpec DC dual-channel export with `Wavelength,Channel_A_DN,Channel_B_DN,Reflectance` columns. CC-0. Covered by semantic tests and golden summary. |

## Parser hints

- Both formats are CSV-like with a header block (File, Date, Notes, …) followed by a 3- or 4-column data block.
- DC files contain two simultaneous channels (typically up- and down-looking radiometers).
- Reference open-source reader: none known; SPECCHIO's parser is closed-source.
- No publicly redistributable real raw `.SPT/.SPU` samples were found.
- Local Arctic LTER UniSpec-DC CSV/XLSX files are derived vegetation-index products, not raw spectra.
