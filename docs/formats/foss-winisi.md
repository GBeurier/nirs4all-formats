# Foss / WinISI Exports

Status: export path done, native binary blocked.

Foss NIRSystems / WinISI native project files (`.NIR`, `.DA`, `.cal`, `.eqa`)
remain vendor-closed and no reliable open reference reader or redistributable
binary fixture is available. The supported path is therefore the exported text
or CSV spectral matrix.

## Implemented

- `Wavelengths:` block text exports through `spectral_matrix`;
- wide CSV exports with metadata/target columns followed by numeric wavelength
  headers through `csv_like`;
- preservation of `ID` as `metadata.sample_id`;
- preservation of numeric properties such as `Moisture`, `Protein` and `Year`
  as targets.

## Supported Fixtures

| Fixture | Records | Axis | Notes |
|---|---:|---|---|
| `samples/foss_winisi/synthetic_winisi_export.txt` | 50 | wavelength, `nm`, 200 points | Synthetic WinISI-style matrix export with `protein` target. |
| `samples/foss_winisi/foss_xds_barleyground_sensAIfood.csv` | 7 | wavelength, `nm`, 1050 points | Real Foss XDS / NIRSystems CSV export, 400-2498 nm. |
| `samples/foss_winisi/foss_xds_wheat2_sensAIfood.csv` | 2 | wavelength, `nm`, 1050 points | Real Foss XDS CSV export, same wide layout. |

## Missing

- native `.NIR`, `.DA`, `.cal` and `.eqa` reverse engineering;
- calibration/equation payload extraction;
- comparison against a vendor or community native binary reader, because none
  is currently available.

