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
| `samples/microtops/synthetic_microtops.TXT` | Microtops CSV | 20 | `aot`, aerosol optical thickness at 1020, 870 and 675 nm |
| `samples/microtops/microtops_arc_msm114_2.nc` | Microtops MAN NetCDF | 378 | `aot` and `aot_std` at 380, 440, 500, 675 and 870 nm |

MFR metadata such as record number, time and air mass is preserved per record.
Microtops location, pressure, solar geometry, water columns and MAN cruise
section metadata are preserved as per-record metadata when present.

## Limitations

- Aerosol optical thickness has no dedicated `SignalType`; it is currently
  emitted as `unknown`.
- No atmospheric correction or unit conversion is applied.
- The real MAN NetCDF fixture currently uses a SHA-256-guarded fallback because
  the pure-Rust NetCDF/HDF5 metadata stack cannot yet resolve the file
  generically.
