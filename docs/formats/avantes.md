# Avantes AvaSoft

Experimental native Rust readers for Avantes AvaSoft ASCII and binary
fixtures.

## Scope Implemented

ASCII exports:

- AvaSoft wave tables (`.ttt`, `.trt`, `.tit`, `.tat`) with `Wave;...`
  columns;
- two-column irradiance exports (`.IRR`) such as `irr_820_1941.IRR`.
- text exports from AvaSoft 8 such as `avasoft8.txt`; these are ASCII fixtures
  and do not close the binary AvaSoft 8 suffix gaps.
- `Dark`, `Ref`, `Reference` and `Sample` ASCII columns are typed as
  `raw_counts`; processed columns retain reflectance, transmittance or
  irradiance signal types.

Legacy AvaSoft binaries:

- AvaSoft 7-style single-mode files whose header is stored as little-endian
  `float32` values;
- tested extensions: `.TRM`, `.ROH`, `.DRK`, `.REF`;
- `.ABS` is routed through the same layout and formula path but is not covered
  by a committed fixture yet.

AvaSoft 8 binaries:

- `AVS82`/`AVS84` containers;
- tested extensions: `.Raw8`, `.IRR8`;
- one subfile per current fixture;
- wavelength coordinates, scope, dark and reference arrays are decoded.

## Legacy Layout

The committed AvaSoft 7 binary fixtures use:

```text
0    version_id f32 = 70
4    spec_id[9] as 9 float32 ASCII code points
40   user_name[64] as 64 float32 ASCII code points
296  wavelength coefficients a0..a4
316  first_pixel
320  last_pixel
324  measure_mode
400  data
```

The wavelength axis is:

```text
wavelength = a0 + a1*p + a2*p^2 + a3*p^3 + a4*p^4
```

where `p` is the pixel index from `first_pixel` to `last_pixel`.

For `.TRM` and `.ABS`, payload triples are interleaved per point:

```text
scope, white, dark
```

The current formulas are:

- transmittance percent: `(scope - dark) / (white - dark) * 100`;
- absorbance: `-log10((scope - dark) / (white - dark))`.

Raw `.ROH`, `.DRK` and `.REF` payloads store one `float32` vector followed by
three trailing acquisition values.

## AvaSoft 8 Layout

The implemented subset follows the current fixtures:

```text
0      magic char[5] = AVS82 or AVS84
5      number of spectra
sub+0  subfile length u32
sub+4  sequence u8
sub+5  measurement mode u8
sub+6  bitness u8
sub+7  sd marker u8
sub+8  spec_id[10]
sub+18 user_name[64]
sub+83 start_pixel u16
sub+85 stop_pixel u16
sub+87 integration_time f32
sub+91 integration_delay u32
sub+95 averages u32
sub+128 SPC date i32
sub+192 comment[130]
sub+322 xcoord[n], scope[n], dark[n], reference[n] as float32
```

The reader advances to the next subfile with `sub_start + length + 10`, matching
the merge-group trailer observed in the fixtures.

`IRR8` calibration is not applied yet. The primary signal is exposed as
`irradiance` for discoverability, but provenance contains
`avantes_irr8_irradiance_calibration_not_applied`.

## Record Mapping

- one `SpectralRecord` per file or AvaSoft 8 subfile;
- axis: wavelength in `nm`;
- metadata: `metadata.avantes` with version/magic, spectrometer id, user name,
  pixels, acquisition parameters, decoded AvaSoft 8 SPC date/time and comments
  where present;
- legacy `.TRM`: `transmittance`, `sample`, `white_reference`,
  `dark_reference`;
- legacy `.ROH/.DRK/.REF`: `scope`, `dark_reference` or `white_reference`;
- AvaSoft 8 `.Raw8`: `scope`, `sample`, `dark_reference`, `white_reference`;
- AvaSoft 8 `.IRR8`: `irradiance`, `sample`, `dark_reference`,
  `white_reference`.

## Fixtures and Reference Checks

Current binary fixtures:

| File | Points | Axis | Primary control |
|---|---:|---|---|
| `avantes2.TRM` | 1442 | `275.271759 -> 1100.133307 nm` | transmittance `11.840215 -> -127.179425` |
| `avantes_trans.TRM` | 1623 | `179.100616 -> 1100.347880 nm` | transmittance `30.313837 -> 54.054054` |
| `avantes_reflect.ROH` | 1442 | `275.271759 -> 1100.133307 nm` | scope `805.000000 -> 774.299988` |
| `1305084U1.DRK` | 1442 | `275.271759 -> 1100.133307 nm` | dark `785.900024 -> 782.700012` |
| `1305084U1.REF` | 1442 | `275.271759 -> 1100.133307 nm` | white `856.000000 -> 802.200012` |
| `1904090M1_0003.Raw8` | 1019 | `300.013855 -> 899.874878 nm` | `scope` equals raw `sample`; `AVS84`, measure mode `0` |
| `eg.IRR8` | 1620 | `144.942429 -> 1100.441406 nm` | `irradiance` equals raw `sample`; `AVS84`, measure mode `4`, calibration warning |

Each committed binary suffix above is also locked by probe tests so extension
routing remains aligned with the binary header/magic checks.
AvaSoft 8 `spc_date` values are unpacked into top-level
`acquisition_start_date` and `acquisition_start_time` metadata while the raw
integer and decoded components remain under `metadata.avantes`.

Current ASCII fixtures:

| File | Points | Axis | Primary control |
|---|---:|---|---|
| `avantes_export.ttt` | 401 | `300.000000 -> 700.000000 nm` | transmittance `3.148700 -> 31.491200` |
| `avantes_export2.trt` | 1442 | `275.270000 -> 1100.130000 nm` | sample counts `805.000000 -> 774.300000` |
| `avantes_export_long.ttt` | 1442 | `275.270000 -> 1100.130000 nm` | `Dark`, `Ref` and `Sample` raw-count columns plus `Transmittance` |
| `irr_820_1941.IRR` | 1922 | `173.000000 -> 1133.500000 nm` | two-column irradiance parser |
| `avasoft8.txt` | 401 | `300.000000 -> 700.000000 nm` | AvaSoft 8 text export with raw dark/reference/sample and reflectance |

Reference behavior was compared against the documented `lightr` parser layout
and its known formulas. `lightr` is GPL, so it remains a conformance reference
only; no runtime dependency is linked into the MIT Rust core.

## Next Work

- Add committed `.ABS`, `.RFL8`, `.TRM8` and multi-subfile AVS8 fixtures.
- Decode and normalize the remaining AVS8 acquisition fields.
- Implement calibrated `IRR8` once the required irradiance calibration terms are
  understood.
- Add subprocess-based conformance reports against `lightr` in the
  reverse-engineering lab when the local R native dependency chain is available.
