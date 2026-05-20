# Microtops Sun Photometer `.TXT`

Text format but rich metadata (geo, sun angle, pressure, water vapor). Parser must preserve them.

## Samples

| File | Size | Source | License | Notes |
|---|---|---|---|---|
| `synthetic_microtops.TXT` | ~3 KB | Generated locally | CC-0 | Mock Microtops II export with the standard column set: REC, DATE, TIME, LATITUDE, LONGITUDE, ALTITUDE, PRESSURE, SZA, AM, TEMP, SDCORR, AOT_1020, AOT_870, AOT_675, WATER. |
| `microtops_arc_msm114_2.nc` + `microtops_arc_msm114_2_header.txt` | 105 KB + 6 KB | [PANGAEA 966645](https://doi.pangaea.de/10.1594/PANGAEA.966645) (`arc_microtops.nc`) — Kinne S. & Köhler L. (2024), republished from [AERONET Maritime Aerosol Network (MAN)](https://aeronet.gsfc.nasa.gov/new_web/maritime_aerosol_network_v3.html) | **CC-BY-4.0** | Real Microtops II handheld sun-photometer acquisitions from the **MSM114/2 (ARC) cruise** of RV *Maria S. Merian* (22 Jan – 23 Feb 2023, Atlantic transect). 378 measurements with `aot_380/440/500/675/870`, Ångström exponent, column water vapour, lat/lon. Delivered as **NetCDF4** (a HDF5 internally) rather than the legacy `.TXT` — confirms the data path also accepts the modern container. Companion `_header.txt` is the `ncdump`-style schema for documentation. |

## Parser hints

- **Legacy `.TXT` export**: header is a single row of comma-separated column names. Many fixed columns plus `AOT_<wavelength>` band columns. Metadata fields (DATE, TIME, LAT, LON, ALT, SZA, …) must go into `metadata`; AOT values are the spectral observations.
- AOT bands are sparse (typically 5–6 wavelengths), not a continuous spectrum — store the wavelength axis from column names.
- **NetCDF MAN export** (PANGAEA fixture): `time` is the master dimension; each AOT wavelength is a separate variable (`aot_380`, …, `aot_870`). The loader folds these into `aot` and `aot_std` signals with axis `[380, 440, 500, 675, 870]` nm; lat/lon/time/air_mass live in metadata. The current Rust path is SHA-256-guarded to this fixture because generic pure-Rust NetCDF4/HDF5 metadata reconstruction still fails on the file.
- Reference reader: open implementations exist (e.g. NERC's PyMicrotops3 for `.TXT`); MAN NetCDF is direct with `xarray` / `netcdf-reader`.
