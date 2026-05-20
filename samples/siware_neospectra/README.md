# Si-Ware NeoSpectra (handheld MEMS)

CSV / XLSX export from the Si-Ware NeoSpectra (Scanner / Micro) handheld MEMS Fourier-transform NIR spectrometer (~1300-2600 nm). Often paired with site/soil metadata blocks. Public real samples come from the Open Soil Spectral Library (Woodwell Climate / KSSL) and from the UvA forensic-NIR dataset on Figshare.

## Samples

| File | Size | Source | License | Notes |
|---|---|---|---|---|
| `synthetic_neospectra.csv` | ~10 KB | Generated locally | CC-0 | Mock single-measurement export with `#`-commented metadata block (site, soil moisture, GPS) and 2-column (wavelength_nm, absorbance) data. |
| `neospectra_ossl_column_names.csv` | 6.5 KB | [`Zenodo 13122321 — OSSL NeoSpectra v1.2`](https://zenodo.org/records/13122321) (`Neospectra_database_column_names.csv`) | **CC-BY-4.0** (Open Soil Spectral Library / Woodwell Climate) | Schema of the OSSL NeoSpectra Woodwell+KSSL database: column name → type → example → description. Documents every column of the wide CSV below. |
| `neospectra_ossl_50samples_slice.csv` | 117 KB | First 50 rows of `Neospectra_WoodwellKSSL_soil+site_NIR.csv` from the same Zenodo record (full file = 169 MB / 1976 rows) | CC-BY-4.0 | Real 50-row slice of the OSSL NeoSpectra database: 50 mineral-soil samples × 614 columns (KSSL metadata + 9 scanners × replicates + averaged NIR spectrum). Demonstrates the "wide CSV with metadata block + spectra block" layout actually shipped by Woodwell. |
| `neospectra_forensic_K_avg.xlsx` | 138 KB | [`Figshare 21252300`](https://doi.org/10.21942/uva.21252300) (`K_Avg_NeoSpectra.xlsx`) | CC-BY-4.0 (UvA, Kranenburg et al. 2022) | 88 ketamine-containing forensic samples averaged on the NeoSpectra Scanner (1299-2606 nm). Header row = `axis: wave` + 160 wavelength columns; rows = sample IDs + reflectance. Confirms the "first-row wavelengths" Excel export pattern used by NeoSpectra outside the OSSL schema. |

## Parser hints

- Single-measurement CSV (synthetic fixture): `#`-prefixed comment lines carrying instrument and site/soil metadata, then 2-column `wavelength_nm,absorbance` data.
- For multi-sample exports, NeoSpectra writes one CSV per measurement — the loader should glob and aggregate when given a directory.
- OSSL wide CSV (real fixture): first ~85 columns are KSSL/Woodwell metadata (UUID, layer/horizon, depth, site coordinates, reference chemistry, scanner serial, etc.), then **9 blocks of 60 NIR wavelengths** (one per Si-Ware scanner, repeats) followed by an averaged scan. Column names in the spectral block are numeric (nm). The companion `_column_names.csv` is the authoritative schema.
- NeoSpectra Excel export (forensic fixture): single sheet `Sheet1`. First row = `axis: wave` then wavelengths; each subsequent row = sample ID + reflectance values. Wavelengths are floats (1299.36951 …).
