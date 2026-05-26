# ERDAS LAN / AVIRIS (Indian Pines)

> **Status:** Experimental · **Vendor:** ERDAS Imagine / AVIRIS (NASA JPL) · **Extensions:** `.lan` (+ `.spc`, `.GIS` sidecars)

ERDAS LAN is a banded raster container. This reader is scoped to the classic
AVIRIS 92AV3C "Indian Pines" hyperspectral cube — a widely used benchmark — and
is meant for validating hyperspectral dispatch and pixel-spectrum extraction,
not as a general ERDAS Imagine reader.

## Instruments & software

The supported file is the AVIRIS 92AV3C cube acquired over the Indian Pines test
site and distributed in ERDAS LAN form. The accompanying `.spc` file is **not**
Galactic SPC: it is an AVIRIS band-calibration text file listing the 220 band
centres. The optional `92AV3GT.GIS` file is the per-pixel ground-truth land-cover
label map.

## File structure

- `HEAD74` magic followed by a 128-byte ERDAS header (band count at offset 8,
  rows at 16, cols at 20, read as little-endian u32).
- 145 rows × 145 columns × 220 bands of unsigned 16-bit little-endian samples in
  BIL (band-interleaved-by-line) order.
- `<stem>.spc` sidecar — one band-centre wavelength per line (first whitespace
  token), skipping `FILE` and dashed separator lines. The native band order has
  local wavelength inversions, so the emitted axis order is non-monotonic.
- `92AV3GT.GIS` sidecar (optional) — a second `HEAD74` raster of the same
  145×145 footprint carrying the class label per pixel.

## What nirs4all-io extracts

- **Default layout** — one `SpectralRecord` per pixel (21,025 records), each with
  a single `raw_counts` signal, a `nm` wavelength axis (unit `dn`), and metadata
  `sample_id = pixel_y{row}_x{col}`, `x_index`, `y_index`, `spatial_x`,
  `spatial_y` (= column / row), `spatial_unit = "pixel"`, `rows`, `cols`,
  `bands`, `interleave = "bil"`. When the `.GIS` sidecar is present, each record
  gains `targets.land_cover_class`.
- **Pixel selection** — a half-open `CubeWindow` ROI (`--rows START:END
  --cols START:END`) or an ordered sparse `CubeMask` (`--pixel ROW,COL` /
  `--pixels-file`, duplicates preserved). The window mode works alongside the
  single-record cube; the mask mode does not.
- **Single-record mode** — `ReadOptions::single_record()` / `--single-record`
  emits one N-dimensional record (`dims = ["row", "col", "x"]`, `shape =
  [rows, cols, bands]`) with `row`/`col` index coordinates; the ground-truth
  labels are kept as a 2-D `metadata.land_cover_class_grid` (they do not fit the
  scalar-per-record `targets` shape — use the per-pixel layout for
  pixel-as-sample classification).
- **Provenance** — the `.lan` primary, the `.spc` axis sidecar and (when present)
  the `.GIS` ground-truth, each with SHA-256. The first record carries the
  `erdas_lan_aviris_experimental` status warning (kept off the other 21,024
  pixel records to avoid ~200 KiB of duplicated provenance; the
  `erdas-lan-aviris` format name carries the same signal), plus
  `erdas_lan_spc_axis_non_monotonic_native_order` when the `.spc` axis is out of
  order.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| AVIRIS 92AV3C 145×145×220 ERDAS LAN | Experimental | Sole supported layout; strict refusal for any other dimensions. |
| Generic ERDAS Imagine / LAN | Planned | No general header parsing, data types or interleaves yet. |
| NEON / Specim / HySpex / Headwall / AVIRIS-NG cubes | Planned | HDF5 and raw cube layouts not covered. |

## Limitations & known gaps

- Dimensions other than 145×145×220 are refused with a descriptive error.
- The ground-truth filename is the literal `92AV3GT.GIS` regardless of the LAN
  stem; a user-renamed ground-truth file would not be picked up (an intentional
  narrowing to the canonical Indian Pines layout).
- No reference comparison against Spectral Python or rasterio yet.

## Reference readers

The Indian Pines cube is commonly read with Spectral Python (`spectral`),
`rasterio` and SciPy. A subprocess conformance comparison is not yet wired in.

## Samples & validation

Fixtures live under `samples/hyperspectral_cubes/`: `92AV3C.lan` (21,025 pixel
records, 220-point `nm` axis), `92AV3C.spc` (axis calibration), and `92AV3GT.GIS`
(per-pixel `land_cover_class`). The semantic test validates a `rows=10:12`,
`cols=20:22` ROI against the full 21,025-record expansion and a sparse mask
`[(0,0), (72,36), (144,144), (10,20)]` for caller-ordered selection, plus the
refusal paths for empty and out-of-bounds masks. ERDAS LAN is sidecar-bearing:
`open_path` reads the `.lan` plus both sidecars from disk, `open_with_sidecars`
serves them from memory, and `open_bytes` returns `Error::UnsupportedSidecar`
because the `.spc` axis is mandatory.
