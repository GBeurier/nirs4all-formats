# USGS SPECPR / Spectral Library Text

Status: experimental partial.

The production target is historical USGS SPECPR plus the ASCII exports commonly
used as an interchange path. The current Rust support is intentionally text
first:

- USGS splib06 `.asc` files with wavelength, reflectance and standard deviation
  columns are parsed by `row-spectral-table`; standard deviation is typed as
  `uncertainty`;
- ECOSTRESS / ASTER `.spectrum.txt` exports with metadata-described X/Y
  columns are parsed by `row-spectral-table`;
- legacy single-column `AREF` dumps are parsed by `usgs-aref-single-column`.

The binary SPECPR container is still not decoded.

## Supported Fixtures

| Fixture | Reader | Records | Axis | Notes |
|---|---|---:|---|---|
| `samples/specpr/asphalt_gds366.27407.asc` | `row-spectral-table` | 1 | wavelength, `um`, 2151 points | Reflectance + standard deviation columns |
| `samples/envi_sli/ecostress_b.spectrum.txt` | `row-spectral-table` | 1 | wavelength, `um`, 2151 points | ECOSTRESS text reflectance |
| `samples/envi_sli/ecostress_a.spectrum.txt` | `row-spectral-table` | 1 | wavelength, `um`, 561 points | ECOSTRESS text reflectance |
| `samples/envi_sli/aster_granite.spectrum.txt` | `row-spectral-table` | 1 | wavelength, `um`, 2844 points | ASTER/JHU text reflectance |
| `samples/envi_sli/usgs_liba_AREF.txt` | `usgs-aref-single-column` | 1 | generated `index`, 24 points | Reflectance only; provenance warning marks missing wavelength axis |

## Missing Behavior

- Decode binary SPECPR records directly.
- Attach true wavelength vectors to one-column `AREF` dumps when a sidecar or
  library-level axis is available.
- Add reference comparisons against the USGS conversion tools or Spectral
  Python for representative Library A/B/C/D exports.
