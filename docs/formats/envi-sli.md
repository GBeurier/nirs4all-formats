# ENVI Spectral Library & Standard Cubes

> **Status:** Supported (scoped) · **Vendor:** L3Harris / ENVI (formerly Exelis / RSI) · **Extensions:** `.sli` + `.hdr` (spectral library); `.img` / `.dat` + `.hdr` (image cube)

ENVI is the de facto interchange format for hyperspectral remote sensing. A
plain-text `.hdr` header describes a binary payload, which is either a
one-band-per-row spectral library (`.sli`) or a multi-band image cube
(`.img` / `.dat`). nirs4all-formats reads both: spectral libraries become one record
per stored spectrum, and image cubes become one record per pixel (or a single
N-dimensional cube record on request).

## Instruments & software

Written by ENVI and compatible GIS/remote-sensing tools. ENVI spectral
libraries ship with reference collections such as the USGS spectral libraries
(splib06/splib07), while ENVI Standard cubes back airborne and lab hyperspectral
imagers. Committed fixtures include a synthetic little-endian float32 library, a
mini ENVI Standard cube, and the USGS AVIRIS-resampled splib06a / splib07
libraries.

## File structure

- **Header (`.hdr`)** — ASCII key/value lines beginning with `ENVI`, parsed into
  `samples`, `lines`, `bands`, `interleave`, `data type`, `byte order`, optional
  `header offset`, `wavelength = { ... }`, `wavelength units`, `spectra names`
  and (for cubes) `map info`. Brace-wrapped lists may span multiple lines.
- **Binary payload** — resolved from `data file = ...` when present, otherwise
  from the sibling file (`.sli` / `.SLI` for libraries; `.img` / `.IMG` /
  `.dat` / `.DAT` for cubes). Decoded through the shared numeric helper covering
  ENVI `data type` 1, 2, 3, 4, 5, 12, 13, 14, 15 (u8 / i16 / i32 / f32 / f64 /
  u16 / u32 / i64 / u64) in either byte order.
- **Layout constraints** — spectral libraries require `bands == 1` and BSQ
  interleave; ENVI Standard cubes support BSQ, BIL and BIP and resolve the
  interleave into C-order on read.

## What nirs4all-formats extracts

- **Signals** — one `spectrum` signal per record. The signal type is `Unknown`
  (ENVI headers carry no semantic unit field).
- **Axis** — values from `wavelength = { ... }`; the unit/kind follow
  `wavelength units` (`nm`, `um`, `cm-1`); a missing axis falls back to a
  generated index with the `envi_sli_missing_wavelength_axis_generated_index`
  warning.
- **Library records** — one record per `spectra names` entry, with
  `metadata.sample_id` and an `envi` object (dimensions, data type, interleave,
  byte order, wavelength units, sensor type).
- **Cube records** — one record per pixel, `sample_id = pixel_y{row}_x{col}`,
  `pixel_x` / `pixel_y`, and, when `map info` is present, `spatial_x` /
  `spatial_y`, normalized `spatial_unit`, projection, reference pixel/map
  coordinates, pixel sizes, zone, hemisphere and datum.
- **Pixel selection** — cubes accept a half-open `CubeWindow` ROI
  (`--rows START:END --cols START:END`) or an ordered sparse `CubeMask`
  (`--pixel ROW,COL` / `--pixels-file`); original pixel coordinates are
  preserved.
- **Single-record mode** — `ReadOptions::single_record()` / `--single-record`
  emits one N-dimensional record (`dims = ["row", "col", "x"]`) with `row`/`col`
  index coordinates and map-level georeferencing in metadata (rejects a sparse
  mask).
- **Provenance** — header sidecar (role `header`) and binary payload (role
  `binary`), each with SHA-256.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| ENVI Spectral Library `.sli` + `.hdr` | Supported | One-band BSQ float/int payloads, either byte order; `spectra names` become records. |
| ENVI Standard cube `.img` / `.dat` + `.hdr` | Supported (scoped) | BSQ/BIL/BIP; per-pixel records, ROI window, sparse mask, or single N-D record. |
| USGS splib06 / splib07 (AVIRIS-resampled) | Supported | Read through both `.hdr` and direct `.sli` entry paths. |
| Legacy `.slb` spectral library | Planned | No fixture yet; low NIRS impact. |
| Specim / HySpex / Headwall / NEON cubes | Planned | Production-scale cube fixtures still wanted. |

## Limitations & known gaps

- `data ignore value` sentinels are preserved as numeric values rather than
  converted to missing values; a masking policy is pending in the shared model.
- Non-zero `header offset` and big-endian payloads are accepted by the decoder
  but not yet exercised by a committed fixture.
- The single-record cube mode rejects sparse masks; use the per-pixel layout for
  arbitrary pixel selection.

## Reference readers

The format is documented by ENVI and supported by Spectral Python (`spectral`),
R `RStoolbox::readSLI()`, `pysptools` and `rasterio`. The nirs4all-formats reader is
clean-room. Conformance output from Spectral Python is planned once the optional
dependency is present in the reverse-engineering environment.

## Samples & validation

Fixtures live under `samples/envi_sli/` (synthetic library `.hdr`/`.sli`, mini
cube `.hdr`/`.img`, USGS splib06a/splib07) and the AVIRIS cube under
`samples/hyperspectral_cubes/`; all are golden-backed. The synthetic library
yields 50 records of 200 points over 1100–2500 nm, first record `y[0] = 0.0367427170`,
last record `y[199] = 0.0608757548`. The mini cube is 48×48 pixels × 32 bands
(first pixel first/last `100`/`3223`; last pixel `152`/`3275`), with UTM `map info`
normalized to `spatial_unit = "m"`. The USGS libraries expose 1365 (splib06a) and
3139 (splib07) records on a 224-point `um` axis (`0.38315..2.5082`). ENVI is a
sidecar-bearing format: `open_path` reads the pair from disk, `open_with_sidecars`
decodes in-memory bytes through a `SidecarResolver`, and `open_bytes` returns
`Error::UnsupportedSidecar`.
