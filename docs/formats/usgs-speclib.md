# USGS SPECPR / Spectral Library Text

> **Status:** Supported (scoped) · **Vendor:** USGS / JHU / ECOSTRESS · **Extensions:** `SPECPR`, `.asc`, `.txt`, `.spectrum.txt`

USGS SPECPR is the historical binary container behind the USGS Spectral Library
(splib06/07) and related ECOSTRESS/ASTER/JHU reflectance libraries. In practice
these libraries are most often exchanged as ASCII text, so nirs4all-formats supports
the text interchange paths today; the binary SPECPR container is not yet
decoded.

## Instruments & software

USGS field/laboratory spectrometer measurements (e.g. ASD-based splib
acquisitions) and the JHU/ECOSTRESS reflectance libraries, distributed by the
USGS speclib tools and the ECOSTRESS/ASTER spectral library projects.

## File structure

Three text layouts are handled, routed to the appropriate reader:

- **SPECPR `.asc` / `*.spectrum.txt`** — axis-first tables with a wavelength
  column followed by reflectance (and, for splib, standard deviation). These are
  parsed by the [row-spectral-table](row-spectral-table.md) reader.
- **Single-column `AREF` dump** — a one-line title (`Record=… AREF`) followed by
  bare reflectance values with no embedded wavelengths. This is handled by the
  dedicated `usgs-aref-single-column` reader, which detects the title line and
  reads the column.

## What nirs4all-formats extracts

- **Axis-first text** (via row-spectral-table) — wavelength axis in `um`, a
  `reflectance` signal and, for splib `.asc`, a standard-deviation column typed
  as `uncertainty`. ECOSTRESS/ASTER metadata-described columns are preserved.
- **AREF single-column** (dedicated reader) — a `reflectance` signal over a
  generated `index` axis (no wavelengths in the file), with the title and
  optional record number under metadata, an `axis_note` of "no wavelength axis
  in file", and the warning `usgs_aref_axis_generated_index`.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| USGS splib06/07 `.asc` (wavelength + reflectance + std-dev) | Supported | Via row-spectral-table; std-dev typed `uncertainty`. |
| ECOSTRESS / ASTER / JHU `*.spectrum.txt` | Supported | Via row-spectral-table; metadata-described X/Y columns. |
| Single-column `AREF` dump | Supported (scoped) | Generated index axis; provenance warning marks the missing wavelength axis. |
| Binary SPECPR container | Blocked | Not decoded; binary records still need a sample/spec path (low v1 priority). |

## Limitations & known gaps

- The binary SPECPR container is not decoded; only the ASCII interchange paths
  are supported.
- One-column `AREF` dumps cannot recover a true wavelength vector — no sidecar
  or library-level axis is attached, so the axis is a synthetic index.
- Reference comparisons against the USGS conversion tools or Spectral Python for
  representative Library A/B/C/D exports are not yet wired in.

## Reference readers

USGS speclib conversion tools and Spectral Python (`spectral`) are the reference
candidates; comparisons are not yet automated.

## Samples & validation

Fixtures are golden-backed in `crates/nirs4all-formats/tests/goldens/`:
`samples/specpr/asphalt_gds366.27407.asc` (splib06, 2151 points, reflectance +
std-dev), the ECOSTRESS/ASTER text exports under `samples/envi_sli/`
(`ecostress_a.spectrum.txt`, `ecostress_b.spectrum.txt`,
`aster_granite.spectrum.txt`) and `samples/envi_sli/usgs_liba_AREF.txt`
(24-point single-column AREF dump on a generated index axis). The AREF probe
reports format `usgs-aref-single-column` at `Confidence::Likely`.
