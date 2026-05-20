# Spectral Evolution / PSR SED

Status: experimental.

The `.sed` reader targets Spectral Evolution / PSR ASCII exports. It recognizes
files by the `.sed` extension plus `Version:` and `Instrument:` header keys,
then parses the `Data:` table.

## Scope Implemented

Implemented:

- key/value header preservation under `vendor`;
- wavelength axis in `nm`;
- one normalized signal per data column after `Wvl`;
- reflectance columns typed as `reflectance`;
- DN/reference/target columns typed as `raw_counts`;
- signal units inferred from observed column labels: `DN` for normalized DN
  columns, `%` for `Reflect. %`, and `1` for `Reflect. [1.0]`;
- explicit warning and quality flag when the file contains only DN channels and
  no reflectance signal.
- parseable GPS latitude/longitude/altitude, acquisition date/time, GPS time
  and satellite counts promoted to canonical top-level metadata while the raw
  header remains preserved under `vendor`;
- instrument/model/serial, measurement mode, radiometric calibration, declared
  point count, wavelength range, source signal labels and source signal units
  promoted to top-level metadata.

## Fixture Coverage

| Fixture | Variant | Coverage |
|---|---|---|
| `1566060_09506_working.sed` | PSR+3500 DN + reflectance | 2151-point axis, raw DN reference/target plus reflectance |
| `1566060_15025_not_working.sed` | broken-but-valid DN-only export | 2151-point axis, two raw DN signals, `missing_reflectance_signal` quality flag |
| `serbinsh_cvars_grape_leaf.sed` | PSR-3500 grape-leaf reflectance acquisition | 2151-point axis, firmware/header drift coverage, canonical GPS/date/time metadata |

The DN-only fixture remains readable because it contains valid spectral raw
channels. It is not promoted to reflectance: downstream users must handle the
`sed_missing_reflectance_signal` warning or compute reflectance from a validated
workflow.

## Known Gaps

- SR-3500 / SR-6500 firmware-specific headers remain under-covered.
- Signal-unit inference is limited to column labels observed in committed
  fixtures.
- The reader does not reconstruct reflectance from DN-only acquisitions.
- Automated conformance reports against `spectrolab` / `specdal` are still
  pending in the reverse-engineering lab.
