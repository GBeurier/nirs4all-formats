# Spectro Inc. SiWare API (JSON / CSV)

Recent cloud-attached MEMS spectrometers stream measurements as JSON over HTTP. Schema documented by Spectro Inc; the loader should support both the JSON payload and the CSV companion export.

## Samples

| File | Source | License | Notes |
|---|---|---|---|
| `synthetic_siware_api.json` | Generated locally | CC-0 | Mock SiWare API payload: `instrument` / `measurement` (wavelengths + absorbance + metadata) / `predictions` (property values). |
| `synthetic_siware_api.csv` | Generated locally | CC-0 | Mock CSV stream with `#`-commented metadata then `wavelength_nm,absorbance`; parsed by the generic `row-spectral-table` reader. |

## Parser hints

- JSON: stream-friendly (one measurement per object). `nirs4all-io` detects `measurement.wavelengths` + `measurement.absorbance` and maps `predictions` to targets.
- For batch ingestion (a folder of JSON files), behave like a `SpectralCollection`.
- **No publicly redistributable real samples** were found; the Spectro Inc. API is gated behind customer credentials.
