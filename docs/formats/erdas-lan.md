# ERDAS LAN / AVIRIS

Status: experimental, sample-backed subset.

The ERDAS `.lan` reader is currently limited to the classic AVIRIS 92AV3C
Indian Pines cube committed under `samples/hyperspectral_cubes/`. It is useful
for validating hyperspectral dispatch and pixel-spectrum extraction, not as a
general ERDAS Imagine reader.

## Format

The supported layout is:

- `HEAD74` magic;
- 128-byte ERDAS header;
- 145 rows x 145 columns x 220 bands;
- unsigned 16-bit little-endian BIL payload;
- `.spc` sidecar with 220 band-center wavelengths;
- optional `92AV3GT.GIS` 145 x 145 ground-truth label map.

The `.spc` sidecar is not Galactic SPC. It is an AVIRIS band-calibration text
file. The native band order has a few local wavelength inversions, so the
emitted `SpectralAxis.order` is non-monotonic and records carry warning
`erdas_lan_spc_axis_non_monotonic_native_order`.

## Implemented

- probe/read for `92AV3C.lan`;
- one `SpectralRecord` per pixel, i.e. 21,025 records;
- optional half-open row/column windows through `open_path_with_options()` and
  the CLI `read-json --rows START:END --cols START:END`;
- optional sparse pixel mask through `ReadOptions::with_cube_mask(CubeMask::new(...))`
  and the CLI `read-json --pixel ROW,COL ...` / `--pixels-file PATH`, preserving
  the order of pixels supplied by the caller (duplicates allowed);
- `raw_counts` signal with wavelength axis in `nm` and unit `dn`;
- `sample_id`, `x_index`, `y_index`, `spatial_x`, `spatial_y` metadata;
- `land_cover_class` target from `92AV3GT.GIS` when present;
- strict refusal for ERDAS LAN dimensions other than 145 x 145 x 220.

## Supported Fixtures

| Fixture | Records | Axis | Notes |
|---|---:|---|---|
| `samples/hyperspectral_cubes/92AV3C.lan` | 21,025 | wavelength, `nm`, 220 points | AVIRIS Indian Pines ERDAS LAN cube. |
| `samples/hyperspectral_cubes/92AV3C.spc` | sidecar | wavelength calibration | First column is used as the spectral axis. |
| `samples/hyperspectral_cubes/92AV3GT.GIS` | sidecar | class labels | Per-pixel ground truth exposed as `targets.land_cover_class`. |

The semantic test also validates a `rows=10:12`, `cols=20:22` ROI against the
same pixels from the full 21,025-record expansion, plus a sparse mask reading
`[(0,0), (72,36), (144,144), (10,20)]` to confirm caller-ordered selection and
the refusal paths for empty and out-of-bounds masks.

## Sidecar contract (M1, 2026-05-22)

ERDAS LAN is a sidecar-bearing format: every record carries the
`<stem>.spc` axis sidecar and, when present, the `92AV3GT.GIS`
ground-truth file. Three entry points cover decoding:

- `open_path(path)` reads the `.lan` plus both sidecars from disk.
- `open_with_sidecars(name, bytes, Arc<dyn SidecarResolver>)` decodes
  the cube from in-memory bytes; the resolver serves the `.spc` axis
  and the optional `.GIS` ground-truth.
- `open_bytes(name, bytes)` returns `Error::UnsupportedSidecar` because
  the axis sidecar is mandatory.

The ground-truth filename is the literal `92AV3GT.GIS`, regardless of
the LAN file's stem. A user supplying their own LAN file paired with
e.g. `MYCUBE_gt.GIS` would not get the ground-truth lookup — that's an
intentional narrowing to the canonical AVIRIS Indian Pines layout.

## Metadata surface

Every emitted pixel record carries:

- `sample_id` = `pixel_y{row}_x{col}`;
- `x_index`, `y_index`, `spatial_x`, `spatial_y` (= column / row in
  pixel coordinates);
- `spatial_unit = "pixel"`;
- `rows`, `cols`, `bands`, `interleave = "bil"`;
- `targets["land_cover_class"] = <class>` when the `.GIS` ground-truth
  sidecar is present.

Provenance warnings: every record carries the
`erdas_lan_aviris_experimental` warning (the layout is still scoped to
the 145x145x220 AVIRIS 92AV3C fixture) plus
`erdas_lan_spc_axis_non_monotonic_native_order` when the `.spc` axis is
out of order.

## Missing

- generic ERDAS Imagine/LAN metadata parsing;
- other LAN dimensions, data types and interleaves;
- reference comparison against Spectral Python or rasterio;
- NEON/Specim/HySpex/Headwall/AVIRIS-NG HDF5 or raw cube layouts.
