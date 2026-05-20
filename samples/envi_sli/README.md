# ENVI Spectral Library `.sli` + `.hdr` (and ENVI Cube `.img`/`.hdr`)

ASCII `.hdr` paired with binary `.sli` (library) or `.img`/`.dat` (image cube). Well-documented format from L3Harris/Exelis ENVI.

## Samples

### Spectral library (`.sli` + `.hdr`)

| File | Size | Source | License |
|---|---|---|---|
| `synthetic_lib.sli` + `synthetic_lib.hdr` | 40 KB | Generated locally | CC-0 | Synthetic 50-sample × 200-wavelength library in standard ENVI BSQ float32, complete `.hdr` metadata (`spectra names`, `wavelength`). Useful as a known-good shape fixture. |

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
