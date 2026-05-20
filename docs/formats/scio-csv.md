# Consumer Physics SCiO CSV

Status: experimental.

The SCiO CSV reader covers committed developer-app exports from
`kebasaa/SCIO-read`. It is registered before the generic CSV readers so SCiO
column groups are not mistaken for target-only tables.

## Supported Fixtures

| Fixture | Records | Axis | Signals | Notes |
|---|---:|---|---|---|
| `samples/scio/scio_app_scan.csv` | 1 | wavelength, `nm`, 740-1070 | `spectrum` | Wide `band740` ... `band1070` layout. |
| `samples/scio/scio_scans_from_tech_support.csv` | 145 | wavelength, `nm`, 740-1070 | `spectrum`, `wr_raw`, `sample_raw` | Preamble metadata plus grouped `spectrum_*`, `wr_raw_*`, `sample_raw_*` columns. |
| `samples/scio/scio_calibration_plate_Polypen.csv` | 1 | wavelength, `nm`, 324-790 | `reflectance` | Axis-first calibration table handled by `spectral_table`. |

All three committed SCiO CSV fixtures are golden-backed. The calibration plate
uses the generic row-table path by design because it is an axis-first spectral
table rather than a SCiO grouped export.

The developer export preserves scan/sample/device metadata per record. `Protein`
and `Fat` columns are mapped to targets; temperature and acquisition labels stay
in metadata.

## Limitations

- SCiO native mobile-app/project containers are not decoded.
- The plain `band*` export does not declare whether values are absorbance,
  reflectance or another processed spectrum, so its signal type remains
  `unknown`.
- CSV parsing currently assumes comma-separated exports without quoted embedded
  commas.
