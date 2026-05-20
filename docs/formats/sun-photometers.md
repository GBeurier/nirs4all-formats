# Sun Photometer Text Exports

Status: experimental.

This reader covers small channel-based sun photometer exports. These are not
core NIR lab spectra, but they appear in the sample corpus and exercise the
same normalization contract: channel wavelengths become the spectral axis and
each observation row becomes one `SpectralRecord`.

## Supported Fixtures

| Fixture | Format | Records | Signal |
|---|---|---:|---|
| `samples/mfr/synthetic_mfr.OUT` | MFR-7 fixed-width text | 50 | `channels`, raw counts at 415, 500, 614, 673, 870 and 940 nm |
| `samples_local/mfr/arm_mfrsr_sgp_E11_20210329.nc` | ARM MFRSR b1 NetCDF, local-only | 4,320 | 7-filter hemispheric/diffuse/direct irradiance, alltime voltage and direct/diffuse ratio signals |
| `samples/microtops/synthetic_microtops.TXT` | Microtops CSV | 20 | `aot`, aerosol optical thickness at 1020, 870 and 675 nm |
| `samples/microtops/microtops_arc_msm114_2.nc` | Microtops MAN NetCDF | 378 | `aot` and `aot_std` at 380, 440, 500, 675 and 870 nm |
| `samples_local/microtops/aeronet_man_Okeanos_19_2_*.lev20` | AERONET MAN ASCII, local-only | 25 all-points / 5 daily | `aot`, plus `aot_std` for daily/series exports, at valid 380-870 nm channels |

MFR metadata such as record number, time and air mass is preserved per record.
The local ARM MFRSR NetCDF reader preserves ARM datastream metadata, filter
centroid wavelengths/FWHM, time and solar geometry, and per-signal QC bit rows.
Microtops location, pressure, solar geometry, water columns and MAN cruise
section metadata are preserved as per-record metadata when present. The local
AERONET MAN ASCII reader also preserves campaign, level, aggregation, PI fields
and row metadata, while omitting missing `-999` AOD channels from the spectral
axis.

## Limitations

- Aerosol optical thickness has no dedicated `SignalType`; it is currently
  emitted as `unknown`.
- No atmospheric correction or unit conversion is applied.
- The real MAN NetCDF fixture currently uses a SHA-256-guarded fallback because
  the pure-Rust NetCDF/HDF5 metadata stack cannot yet resolve the file
  generically.
- AERONET MAN `.lev10/.lev15/.lev20` ASCII support is validated on local
  Okeanos samples only; the files are not redistributed because of AERONET MAN
  data-policy constraints.
- ARM MFRSR NetCDF support is validated on one local b1 fixture only; broader
  ARM ACT/xarray conformance and more datastream variants are still pending.
