# Renishaw WDF

Status: experimental.

The Renishaw WDF reader covers the spectral payload subset of WiRE `.wdf`
files:

- `WDF1` chunk header and block table validation;
- `DATA` float32 ordinate payload;
- `XLST` float32 spectral axis payload;
- `YLST` unit metadata when present;
- `ORGN` navigation axes for spatial X/Y/Z, FocusTrack Z and acquisition time;
- `WMAP` map dimensions, map type, offsets and scales;
- `WHTL` white-light image metadata without embedding the image payload;
- `MAP ` analysis-block inventory without treating derived PSET payloads as
  spectra;
- fixed-header metadata such as point count, scan type, measurement type,
  accumulation count, application version, laser wavenumber, user and title.

Maps, line scans, depth profiles, time series, StreamLine acquisitions and
interrupted acquisitions are emitted as one `SpectralRecord` per stored
spectrum. `ORGN` values are copied to normalized per-record metadata such as
`spatial_x`, `spatial_y`, `spatial_z`, `focus_track_z`,
`time_filetime_100ns` and `elapsed_time_seconds`. `WMAP` adds `map_width`,
`map_height`, `map_x_index`, `map_y_index` and line-scan
`spatial_distance` when the map type is `xyline`. `WHTL` adds a
`white_light_image` metadata object with JPEG MIME type, byte length, SHA-256,
image dimensions, precision/component count, JFIF density and basic EXIF
make/description fields. `MAP ` blocks add `map_analysis_blocks` entries with
block UID, byte length, SHA-256, PSET length and a short printable-string
preview so derived analysis payloads are discoverable without silently
promoting them to spectra. Undefined
`measurement_type=0` containers are still refused.

## Supported Fixtures

| Fixture | Records | Axis | Notes |
|---|---:|---|---|
| `samples/raman_renishaw/renishaw_test_spectrum.wdf` | 1 | wavelength, `nm`, 36 points | RosettaSciIO single-point spectrum |
| `samples/raman_renishaw/renishaw_test_linescan.wdf` | 5 | wavelength, `nm`, 40 points | Diagonal `xyline`; exposes X/Y and distance metadata |
| `samples/raman_renishaw/interrupted_acquisition.wdf` | 12 | wavenumber, `cm-1`, 1010 points | Reads stored count, preserves X/Y map positions and warns about truncated capacity |
| `samples/raman_renishaw/wire_sp.wdf` | 1 | wavenumber, `cm-1`, 1015 points | SpectroChemPy real-world single spectrum |

The full committed WDF fixture set is covered by count-level tests except
`renishaw_test_undefined.wdf` and `wire_undefined.wdf`, which fail with a clear
undefined-measurement message.

## Binary Notes

All observed blocks start with:

```text
name[4]
block_uid: u32le
block_size: u64le
payload...
```

The first `WDF1` block is 512 bytes. The current reader uses these fixed
header offsets:

| Offset | Field |
|---:|---|
| `0x003c` | point count per spectrum |
| `0x0040` | capacity |
| `0x0048` | stored spectrum count |
| `0x0050` | accumulation count |
| `0x0054`, `0x0058` | Y and X sizes |
| `0x0080`, `0x0084` | scan type and measurement type |
| `0x0098`, `0x009c` | spectral unit code and laser wavenumber |

`XLST` payload starts with `data_type: u32le`, `unit: u32le`, followed by
`point_count` float32 axis values. `DATA` stores float32 ordinate values.

`ORGN` payload starts with an axis count. Each axis entry stores
`data_type: u32le`, `unit: u32le`, a 16-byte annotation and one 64-bit value
per capacity slot. Completed acquisitions have `capacity == count`; interrupted
acquisitions require advancing by `capacity` while only emitting the first
`count` values.

`WMAP` is a 48-byte payload containing a map type, XYZ offsets, XYZ scales, XYZ
sizes and `linefocus_size`. Current normalization covers observed map type
values `0` (unspecified regular grid), `2` (column-major StreamLine) and `128`
(`xyline`).

`WHTL` payloads in committed fixtures are JPEG blobs. The reader records their
container metadata only; it does not store or decode pixels into
`SpectralRecord`.

`MAP ` payloads in committed fixtures start with `PSET` and describe WiRE
derived analysis maps, such as intensity-at-point and signal-to-baseline
windows. The reader records an inventory only. Decoding `dataRange` into a
derived non-spectral signal remains a separate decision because the payload is
analysis output, not primary spectral acquisition data.

## Reference Readers

Layout cross-checks:

- `rsciio.renishaw` 0.13.0 for broad WDF coverage and ORGN/WMAP conventions;
- `renishawWiRE` 0.1.16 / `py-wdf-reader` for the single-spectrum and ORGN path;
- SpectroChemPy `read_wire` for chunk and enum cross-checking.
