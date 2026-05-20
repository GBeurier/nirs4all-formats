# DigitalSurf SUR / PRO

Status: experimental / partial.

The DigitalSurf reader covers the committed MountainsMap `.sur` and `.pro`
fixtures from RosettaSciIO:

- fixed 512-byte little-endian object headers;
- uncompressed `DIGITAL SURF` and zlib-stream `DSCOMPRESSED` payloads;
- `_SPECTRUM` single and multi-spectrum profiles;
- `_HYPCARD` hyperspectral maps emitted as one `SpectralRecord` per XY point;
- `_SURFACE` height maps emitted as one profile record per row, with an
  explicit warning because the axis is spatial rather than spectral.
- normalized map metadata: `map_x_index`, `map_y_index`, dimensions and
  `map_axis_order = y_slowest_x_fastest`;
- normalized surface row metadata: `spatial_y_index`, X/Y spatial units and
  `surface_axis_order = row_profiles_y_slowest_x_fastest`.

Wavelength axes stored in `mm` by MountainsMap are normalized to `nm` in the
`SpectralAxis`. The original DigitalSurf axis name and unit are preserved in
metadata fields such as `signal_axis_name` and `signal_axis_original_unit`.

## Supported Fixtures

| Fixture | Records | Axis | Notes |
|---|---:|---|---|
| `samples/digitalsurf/test_spectrum.pro` | 1 | wavelength, `nm`, 512 points | Single spectrum |
| `samples/digitalsurf/test_spectra.pro` | 65 | wavelength, `nm`, 512 points | Multi-spectrum profile with line-position metadata |
| `samples/digitalsurf/test_spectral_map.sur` | 120 | wavelength, `nm`, 310 points | 12 by 10 hyperspectral map |
| `samples/digitalsurf/test_spectral_map_compressed.sur` | 120 | wavelength, `nm`, 281 points | zlib-compressed 12 by 10 hyperspectral map |
| `samples/digitalsurf/test_surface.sur` | 128 | spatial index, `mm`, 128 points | Surface rows exported as spatial profiles |

## Binary Notes

Each object is a complete header plus payload. Multi-object files concatenate
objects; the first header declares `number_of_objects` and `p_size`.

The fixed header starts with a 12-byte signature:

```text
DIGITAL SURF
DSCOMPRESSED
```

Important decoded fields include object type, point size, X/Y dimensions,
optional W size, axis labels and units, offsets, spacings, scaling parameters,
comment size, private-zone size and compressed payload size.

Compressed payloads are not RLE. They use a small directory followed by zlib
streams:

```text
stream_count: u32le
repeat stream_count:
  raw_len_bytes: u32le
  zlib_len_bytes: u32le
repeat stream_count:
  zlib_payload
```

Raw points are signed 16-bit or 32-bit integers. For spectral/profile payloads
the decoded value is:

```text
(raw_int - z_min) * (z_spacing / z_unit_ratio) + z_offset
```

## Remaining Gaps

The committed RosettaSciIO spectrum, multi-spectrum, hyperspectral-map,
surface and compressed fixtures are fully golden-backed. The remaining partial
status is conformance and scope work: automate full-array comparison against
`rsciio.digitalsurf`, enrich object/comment metadata and decide which
MountainsMap variants outside this corpus, including branded AFM-Raman exports,
belong in the core reader.

## Reference Readers

Layout and fixture values are cross-checked against `rsciio.digitalsurf`
0.13.0. RosettaSciIO is GPL-3.0; it is used only as an external conformance
reference and is not imported or linked by the MIT runtime.
