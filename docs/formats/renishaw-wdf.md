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
- `MAP ` analysis-block inventory plus bounded `dataRange` extraction for the
  observed PSET tail layout, without treating derived PSET payloads as spectra;
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
block UID, byte length, SHA-256, PSET length, a short printable-string preview
and `data_range_*` fields when the observed PSET tail can be decoded as
float32 values. When the decoded `dataRange` length exactly matches the stored
spectrum count, each emitted record also receives `map_analysis_values`
entries keyed by block UID and label. Undefined
`measurement_type=0` containers are still refused.

## Supported Fixtures

| Fixture | Records | Axis | Notes |
|---|---:|---|---|
| `samples/raman_renishaw/renishaw_test_spectrum.wdf` | 1 | wavelength, `nm`, 36 points | RosettaSciIO single-point spectrum |
| `samples/raman_renishaw/renishaw_test_linescan.wdf` | 5 | wavelength, `nm`, 40 points | Diagonal `xyline`; exposes X/Y and distance metadata |
| `samples/raman_renishaw/renishaw_test_map.wdf` | 9 | wavelength, `nm`, 40 points | 3 x 3 regular map with WMAP-derived X/Y indices |
| `samples/raman_renishaw/renishaw_test_map2.wdf` | 400 | wavelength, `nm`, 40 points | 20 x 20 map plus two `MAP ` PSET `dataRange` analysis maps |
| `samples/raman_renishaw/renishaw_test_streamline.wdf` | 2,205 | wavelength, `nm`, 1,015 points | StreamLine map, emitted as one record per stored spectrum |
| `samples/raman_renishaw/renishaw_test_focustrack.wdf` | 3 | wavelength, `nm`, 1,015 points | FocusTrack Z metadata |
| `samples/raman_renishaw/renishaw_test_focustrack_invariant.wdf` | 10 | wavelength, `nm`, 1,015 points | FocusTrack invariant navigation variant |
| `samples/raman_renishaw/renishaw_test_exptime10_acc1.wdf` | 1 | wavelength, `nm`, 1,015 points | Exposure/accumulation metadata regression |
| `samples/raman_renishaw/renishaw_test_timeseries.wdf` | 3 | wavelength, `nm`, 1,015 points | Elapsed-time metadata regression |
| `samples/raman_renishaw/renishaw_test_zscan.wdf` | 40 | wavelength, `nm`, 1,015 points | Z-depth navigation regression |
| `samples/raman_renishaw/interrupted_acquisition.wdf` | 12 | wavenumber, `cm-1`, 1010 points | Reads stored count, preserves X/Y map positions and warns about truncated capacity |
| `samples/raman_renishaw/wire_sp.wdf` | 1 | wavenumber, `cm-1`, 1015 points | SpectroChemPy real-world single spectrum |
| `samples/raman_renishaw/wire_depth.wdf` | 40 | wavenumber, `cm-1`, 1,015 points | SpectroChemPy depth profile with elapsed time and two `MAP ` PSET `dataRange` analysis maps |
| `samples/raman_renishaw/wire_line.wdf` | 235 | wavenumber, `cm-1`, 1,015 points | Real-world `xyline` path with distance and X/Y normalization |
| `samples/raman_renishaw/wire_Streamline.wdf` | 2,205 | wavenumber, `cm-1`, 1,015 points | Real-world StreamLine map |

`renishaw_test_undefined.wdf` and `wire_undefined.wdf` are committed as
negative fixtures. Both fail with a clear undefined-measurement message because
their WDF header reports `measurement_type=0`.

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
windows. The observed layout stores float32 `dataRange` values in the payload
tail after `8 + pset_declared_len + 8` bytes. The reader decodes that layout
only when the value stream is finite and 4-byte aligned. It records the values
as per-record metadata, not as spectral signals, because they are derived
analysis output rather than primary acquisition data. Other `MAP ` layouts
remain inventory-only until covered by fixtures or a reference implementation.

## Reference Readers

Layout cross-checks:

- `rsciio.renishaw` 0.13.0 for broad WDF coverage and ORGN/WMAP conventions;
- `renishawWiRE` 0.1.16 / `py-wdf-reader` for the single-spectrum and ORGN path;
- SpectroChemPy `read_wire` for chunk and enum cross-checking.
