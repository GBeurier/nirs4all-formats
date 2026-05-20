# Hyperspectral imaging cubes

Partially supported per `FORMATS.md` §4. The native readers currently expand
small ENVI Standard `.img/.dat + .hdr` cubes and the AVIRIS 92AV3C ERDAS
`.lan` cube to one point spectrum per pixel. Larger hyperspectral workflows
still need an explicit
`extract_point_spectra(cube, mask)` helper and reference checks against
`spectral` / `rasterio`.

## Samples

All from [`spectralpython/sample-data@master`](https://github.com/spectralpython/sample-data) — **NO LICENSE FILE** in the repo (README only; data is the AVIRIS 92AV3C classic dataset distributed for academic use since 1998).

| File | Size | Notes |
|---|---|---|
| `92AV3C.lan` | 8.8 MB | **AVIRIS hyperspectral cube** — Indiana Indian Pines test site, 145 × 145 pixels × 220 bands, ERDAS `.lan` (ENVI-BIL-compatible). The reference cube used by virtually every hyperspectral tutorial. |
| `92AV3C.spc` | 11 KB | Sidecar SPC-format band calibration (NOT Galactic SPC — ENVI-flavour). |
| `92AV3GT.GIS` | 21 KB | Ground-truth classification labels (16 land-cover classes). |
| `spectralpython_README.md` | <1 KB | Upstream attribution: Landgrebe, D. *Multispectral data analysis from a signal theory perspective.* Purdue 1998. |

## Parser hints

- `.lan` is BIL with a 128-byte ERDAS `HEAD74` header and is now parsed directly for the 92AV3C layout.
- Ground-truth `.GIS` is a per-pixel integer label map and is exposed as `targets.land_cover_class`.
- Future ROI/mask extraction should produce representative spectra per class without expanding very large cubes eagerly.
- The `mini-cube` ENVI fixture (`cubescope-mini-cube.hdr` + `.img`) lives in `envi_sli/` — kept there because it ships as an ENVI library-like pair.
