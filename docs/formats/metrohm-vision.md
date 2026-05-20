# Metrohm Vision / Vision Air

Status: experimental partial.

Metrohm Vision Air CSV exports are handled as spectral matrix files: each row is
one sample, early property columns become targets, and numeric wavelength
headers become the spectral axis. Native Vision project databases remain out of
scope until a redistributable fixture or public schema is available.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Targets |
|---|---:|---|---|---|
| `samples/metrohm/synthetic_visionair.csv` | 50 | 200 wavelengths, `1100..2500 nm` | `absorbance` | `protein`, `moisture`, `fat` |

The reader preserves the export title under `metadata.vendor.title`, keeps the
sample identifier in `metadata.sample_id`, and stores the source row as
`metadata.row_index`.

## Dispatch Boundaries

The supported path is CSV/Excel-style export with numeric wavelength columns.
Target-only reports or native Vision project stores are not promoted to
`SpectralRecord` objects because they do not expose a spectral axis.

## Remaining Gaps

- real customer Vision Air CSV/Excel export with license-cleared spectra;
- native Vision project database reverse engineering;
- comparison with a Metrohm/Vision Air export imported through a trusted
  reference workflow;
- broader metadata normalization for instrument, method, product and project
  fields once real exports are available.
