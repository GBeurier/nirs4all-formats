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

## Missing

- generic ERDAS Imagine/LAN metadata parsing;
- other LAN dimensions, data types and interleaves;
- ROI/mask API for extracting only selected pixels from large cubes;
- reference comparison against Spectral Python or rasterio;
- NEON/Specim/HySpex/Headwall/AVIRIS-NG HDF5 or raw cube layouts.
