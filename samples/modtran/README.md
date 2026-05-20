# MODTRAN5 albedo `.dat`

Not really an instrument format — auxiliary scientific text used in radiative-transfer pipelines.

## Samples

| File | Source | License |
|---|---|---|
| `synthetic_albedo.dat` | Generated locally | CC-0 | Mock MODTRAN5 albedo file with `WAVELENGTH_um  ALBEDO` columns. |

## Parser hints

- ASCII, 2-column, whitespace-separated.
- Comment lines start with `#`.
- Wavelengths in micrometres.
- Trivially loadable with `numpy.loadtxt` once the comment lines are skipped.
