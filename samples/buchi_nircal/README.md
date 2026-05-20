# BUCHI NIRFlex / NIRMaster (NIRCal) `.nir`

Proprietary binary that bundles spectra + metadata + reference properties (protein, moisture, …). Distinct from FOSS `.NIR` (see `foss_winisi/`) — the magic byte must distinguish them.

## Samples

| File | Size | Source | License |
|---|---|---|---|
| `muestras-tejido-foliar_transfer.nir` | 880 KB | [`l-ramirez-lopez/prospectr@master/tests/testthat/testdata/muestras-tejido-foliar_transfer.nir`](https://github.com/l-ramirez-lopez/prospectr/blob/master/tests/testthat/testdata/muestras-tejido-foliar_transfer.nir) | MIT (prospectr is CRAN package) | Plant-foliar tissue NIR calibration transfer file. The reference fixture used by `prospectr::read_nircal()` tests. |

## Parser hints

- Reference reader: R [`prospectr::read_nircal()`](https://l-ramirez-lopez.github.io/prospectr/reference/read_nircal.html). **No Python port exists**. v1 strategy: either port the parser or shell out to R via `rpy2`.
- Magic differs from FOSS NIRSystems `.NIR` — never dispatch on extension alone.
- The format embeds:
  - Spectra (matrix)
  - Wavelength axis
  - Reference properties (protein, moisture, fat, ash, …) → these are training labels and must be exposed in `targets`, not metadata. The committed fixture has property names but zero numeric values, which `prospectr` treats as missing.
  - Free-text description / comments
- See `get_nircal_*()` helpers in `prospectr` for the field layout.
