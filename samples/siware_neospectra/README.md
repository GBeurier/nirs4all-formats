# Si-Ware NeoSpectra (handheld MEMS)

CSV export from the NeoSpectra Scanner handheld MEMS Fourier-transform NIR spectrometer. Often paired with site/soil metadata blocks.

## Samples

| File | Source | License |
|---|---|---|
| `synthetic_neospectra.csv` | Generated locally | CC-0 | Mock NeoSpectra Scanner export with `#`-commented metadata block (site, soil moisture, GPS) and 2-column (wavelength_nm, absorbance) data. |

## Parser hints

- Header: `#`-prefixed comment lines carrying instrument and site/soil metadata.
- Data block: 2-column `wavelength_nm,absorbance`.
- For multi-sample exports, NeoSpectra writes one CSV per measurement — the loader should glob and aggregate when given a directory.
