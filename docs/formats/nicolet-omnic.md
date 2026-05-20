# Thermo Nicolet OMNIC

Status: experimental.

The OMNIC reader covers two reverse-engineered legacy layouts:

- `.SPA` single-spectrum files;
- `.SPG` grouped spectra, emitted as one `SpectralRecord` per sub-spectrum.
- `.srs` TGA/GC and rapid-scan series files, emitted as one 2D
  `SpectralRecord` with `dims = ["y", "x"]`.

The reader extracts the spectral header (`nx`, axis unit, signal unit, first and
last x values), the float32 intensity block and the OMNIC title/timestamp fields
when present. Wavenumber axes are emitted in native descending `cm-1` order. For
TGA/GC `.srs` files, the y/time axis is currently preserved as metadata
(`series_y_len`, `series_y_first_min`, `series_y_last_min`, `series_y_step_min`)
until the core schema grows a first-class secondary axis.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Notes |
|---|---:|---|---|---|
| `samples/nicolet_omnic/2-BaSO4_0.SPA` | 1 | wavenumber, `cm-1`, 11098 points | `absorbance` | Real SpectroChemPy fixture |
| `samples/nicolet_omnic/11-Z25-CP_0.SPA` | 1 | wavenumber, `cm-1` | `absorbance` | Additional SPA regression fixture |
| `samples/nicolet_omnic/not_opus.spa` | 1 | wavenumber, `cm-1`, 5549 points | `absorbance` | Regression fixture for `.spa` vs Bruker OPUS disambiguation |
| `samples/nicolet_omnic/wodger.spg` | 2 | wavenumber, `cm-1`, 5549 points | `absorbance` | SpectroChemPy documentation fixture |
| `samples/nicolet_omnic/CO_at_Mo_Al2O3.SPG` | 19 | wavenumber, `cm-1` | `absorbance` | Additional SPG group fixture |
| `samples/nicolet_omnic/nh4y-activation.spg` | 55 | wavenumber, `cm-1` | `absorbance` | Additional SPG group fixture |
| `samples/nicolet_omnic/GC_Demo.srs` | 1 | wavenumber, `cm-1`, 1738 x-points, 788 y rows | `transmittance` | TGA/GC series fixture; 2D `["y", "x"]` matrix tested |
| `samples/nicolet_omnic/TGAIR.srs` | 1 | wavenumber, `cm-1`, 1868 x-points, 335 y rows | `absorbance` | TGA/GC hard-case fixture; 2D `["y", "x"]` matrix tested |
| `samples_local/nicolet_omnic/spectrochempy_TGA_demo.srs` | 1 | wavenumber, `cm-1`, 3630 x-points, 485 y rows | `absorbance` | Local-only larger `tg_gc` series |
| `samples_local/nicolet_omnic/spectrochempy_rapid_scan.srs` | 1 | generated index, 4160 x-points, 643 y rows | `detector_signal`, `V`, interferogram | Local-only `rapid_scan_raw` series |
| `samples_local/nicolet_omnic/spectrochempy_rapid_scan_reprocessed.srs` | 1 | wavenumber, `cm-1`, 3734 x-points, 643 y rows | `absorbance` | Local-only `rapid_scan_reprocessed` series |

## Dispatch Boundaries

`.srs` support is intentionally limited to the series layout identified by the
three `02 00 00 00 18 00 00 00 00 00` signature anchors. This covers the
committed TGA/GC files and the local SpectroChemPy rapid-scan pair. Files with
OMNIC series magic but a different anchor pattern are classified as unsupported
series variants and refused explicitly. `.srsx` remains pending because no
redistributable fixture is available.

`series_variant` is normalized to `tg_gc`, `rapid_scan_raw` or
`rapid_scan_reprocessed` for the fixtures currently understood. Raw rapid-scan
interferograms use a generated index x axis until the model grows a richer
interferogram-domain axis.

The implementation follows the same public reverse-engineering model used by
SpectroChemPy: key `02` points to the spectral header, key `03` points to the
float32 intensity block and key `6B` carries group spectrum titles and OMNIC
timestamps. TGA/GC `.srs` files use fixed offsets relative to the three series
anchors for data header, background header and spectral matrix start.
