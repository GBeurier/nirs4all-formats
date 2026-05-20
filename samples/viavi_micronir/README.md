# VIAVI MicroNIR (handheld)

CSV / XLSX export from MicroNIR Pro / MicroNIR 1700 / MicroNIR OnSite handheld MEMS-based spectrometer. Native `.pri` project files are out of scope for v1 (no public spec).

## Samples

| File | Size | Source | License | Notes |
|---|---|---|---|---|
| `synthetic_micronir.csv` | ~10 KB | Generated locally | CC-0 | Mock MicroNIR Pro export with instrument metadata header (`Instrument`, `Serial`, `Method`, `Date`) and one-sample-per-row spectral block. |
| `micronir_forensic_K_avg.xlsx` | 118 KB | [`Figshare 21252300`](https://doi.org/10.21942/uva.21252300) (`K_Avg_MicroNIR.xlsx`) | **CC-BY-4.0** (UvA, Kranenburg et al. 2022) | 88 ketamine forensic samples averaged on a VIAVI MicroNIR 1700 (908-1676 nm). Sheet `Sheet1`, row 1 = `axis: wave` + 125 wavelength columns at 6.2 nm step; data rows = sample ID + absorbance. Real MicroNIR 1700 acquisition. |
| `micronir_forensic_T_avg.xlsx` | 88 KB | Same Figshare record (`T_Avg_MicroNIR.xlsx`) | CC-BY-4.0 | Same MicroNIR 1700 layout for the "T" sample set (heroin/cocaine/etc., 71 samples). Useful as a second-class fixture so loaders are not over-fit to the K dataset. |

## Parser hints

- Synthetic fixture: header rows are `key,value` pairs (typically up to 4–5 metadata lines), then a wavelength header row, then sample rows. Wavelengths are nm.
- Real MicroNIR 1700 export (Figshare fixtures): single sheet `Sheet1`. First cell `axis: wave`, then ~125 wavelength columns from ~908 nm to ~1676 nm at 6.2 nm step. Subsequent rows: sample ID in column A + reflectance/absorbance per wavelength column.
- Native `.pri` project files remain out of scope for v1 (no public spec, no fixture).
- Reference reader for the wavelength axis: VIAVI MicroNIR documentation cites 125 pixels with non-uniform spacing — interpolate to a regular grid only when the downstream model requires it.
