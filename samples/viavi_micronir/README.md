# VIAVI MicroNIR (handheld)

CSV export from MicroNIR Pro / MicroNIR OnSite handheld MEMS-based spectrometer. Native `.pri` project files are out of scope for v1 (no public spec).

## Samples

| File | Source | License |
|---|---|---|
| `synthetic_micronir.csv` | Generated locally | CC-0 | Mock MicroNIR Pro export with instrument metadata header (`Instrument`, `Serial`, `Method`, `Date`) and one-sample-per-row spectral block. |

## Parser hints

- Header rows are `key,value` pairs (typically up to 4–5 metadata lines), then a wavelength header row, then sample rows.
- Wavelengths in the header row are typically nm.
- Reference reader: none open-source; treat as a structured CSV.
