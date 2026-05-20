# ENVI Spectral Library `.sli` + `.hdr` (and ENVI Cube `.img`/`.hdr`)

ASCII `.hdr` paired with binary `.sli` (library) or `.img`/`.dat` (image cube). Well-documented format from L3Harris/Exelis ENVI.

## Samples

### Spectral library (`.sli` + `.hdr`)

| File | Size | Source | License | Notes |
|---|---|---|---|---|
| `synthetic_lib.sli` + `synthetic_lib.hdr` | 40 KB | Generated locally | CC-0 | Synthetic 50-sample × 200-wavelength library in standard ENVI BSQ float32, complete `.hdr` metadata (`spectra names`, `wavelength`). Useful as a known-good shape fixture. |
| `usgs_splib06a_aviris95_envi.sli` + `.hdr` | 2.4 MB + 64 KB | [`capstone-coal/pycoal@master/pycoal/tests/s06av95a_envi.sli`](https://github.com/capstone-coal/pycoal/tree/master/pycoal/tests) (GPL-2) | USGS data: U.S. Government public domain; pycoal wrapper: GPL-2 | Real **USGS Digital Spectral Library splib06a** convolved to the AVIRIS 1995 sensor grid (224 bands). 477 spectra × 224 floats covering minerals, soils, coatings, liquids, organics, artificial materials and vegetation. Lets the BSQ float32 + multi-spectra path be validated against a true USGS-distributed library. |
| `usgs_splib07_aviris95_envi.sli` + `.hdr` | 2.7 MB + 144 KB | [`capstone-coal/pycoal@master/pycoal/tests/s07_AV95_envi.sli`](https://github.com/capstone-coal/pycoal/tree/master/pycoal/tests) (GPL-2) | USGS V7 data: U.S. Government public domain; pycoal wrapper: GPL-2 | Real **USGS Spectral Library Version 7** convolved to AVIRIS 1995 grid. Same shape contract as splib06 but with the v7 superset of spectra (>2300 entries) — useful to confirm the loader handles arbitrary `samples` counts and the `spectra names` array even when it grows past 1000 entries. |

### ENVI image cube mini-fixture

| File | Size | Source | License |
|---|---|---|---|
| `cubescope-mini-cube.hdr` + `cubescope-mini-cube.img` | 555 B + 144 KB | [`yongyin-leon/CubeScope-demo`](https://github.com/yongyin-leon/CubeScope-demo/blob/main/site/fixtures) | MIT | Tiny ENVI cube for cube-aware code paths (refusal/extraction). |

### Reference ASCII spectra (USGS / ECOSTRESS / ASTER)

Not ENVI binaries, but reference spectra in plain ASCII used widely with ENVI. From [`spectralpython/spectral`](https://github.com/spectralpython/spectral/tree/master/spectral/tests/data) and [`susanmeerdink/ASTER-Spectral-Library`](https://github.com/susanmeerdink/ASTER-Spectral-Library).

| File | Source | Notes |
|---|---|---|
| `usgs_liba_AREF.txt` | USGS splib06a / spectralpython | ASD-band ASCII export from USGS Library A. |
| `ecostress_a.spectrum.txt`, `ecostress_b.spectrum.txt` | ECOSTRESS (spectralpython mirror) | ECOSTRESS spectral library text output. |
| `aster_granite.spectrum.txt` | ASTER / JHU Becknic | Granite reflectance reference. |
| `92AV3C.spc` | spectralpython | AVIRIS hyperspectral cube (legacy `.spc` ENVI variant). Not Galactic SPC. |

## Parser hints

- `.hdr` is ASCII with `key = value` (with `{ … }` lists). Mandatory keys: `samples`, `lines`, `bands`, `data type`, `interleave` (`bsq`/`bil`/`bip`), `byte order`.
- `.sli` payload is samples × bands float32 (BSQ when `bands=1`).
- Reference readers:
  - Python: [`spectral`](https://github.com/spectralpython/spectral) (Spectral Python), `pysptools`
  - R: `RStoolbox::readSLI()`
- Image cubes are explicitly **out-of-scope for v1**. Detect by `file type = ENVI Standard` (vs. `ENVI Spectral Library`) and refuse with a pointer to `spectral`/`rasterio`.
