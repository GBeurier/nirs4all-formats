# Microtops Sun Photometer `.TXT`

Text format but rich metadata (geo, sun angle, pressure, water vapor). Parser must preserve them.

## Samples

| File | Source | License |
|---|---|---|
| `synthetic_microtops.TXT` | Generated locally | CC-0 | Mock Microtops II export with the standard column set: REC, DATE, TIME, LATITUDE, LONGITUDE, ALTITUDE, PRESSURE, SZA, AM, TEMP, SDCORR, AOT_1020, AOT_870, AOT_675, WATER. |

## Parser hints

- Header is a single row of comma-separated column names. Many fixed columns plus AOT_<wavelength> band columns.
- Metadata fields (DATE, TIME, LAT, LON, ALT, SZA, …) must go into `metadata`; AOT values are the spectral observations.
- AOT bands are sparse (typically 5–6 wavelengths), not a continuous spectrum — store the wavelength axis from column names.
- Reference reader: open implementations exist (e.g. NERC's PyMicrotops3) but none ship bundled samples; treat as a structured CSV.
