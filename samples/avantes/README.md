# Avantes AvaSoft

Two binary generations (legacy 6/7 with per-mode extensions; modern 8 with `*8` suffixes) plus ASCII exports. Each suffix encodes the measurement mode.

## Samples

All files are from [`ropensci/lightr@main/inst/testdata`](https://github.com/ropensci/lightr/tree/main/inst/testdata), distributed under **GPL-3** (ROpenSci package).

### AvaSoft 6/7 — legacy binaries

| File | Mode | Notes |
|---|---|---|
| `avantes2.TRM` | Transmittance | One-spectrum-per-mode binary, AvaSoft 6/7 |
| `avantes_trans.TRM` | Transmittance | Another `.TRM` variant |
| `avantes_reflect.ROH` | Reflectance (raw scope) | `.ROH` = raw output / scope mode |
| `1305084U1.DRK` | Dark reference | `.DRK` — paired with `.REF` for absorbance/transmittance computation |
| `1305084U1.REF` | White reference | `.REF` — paired with `.DRK` |
| `irr_820_1941.IRR` | Absolute irradiance | `.IRR` |

### AvaSoft 8 — modern binaries

| File | Mode | Notes |
|---|---|---|
| `1904090M1_0003.Raw8` | Raw scope | `.RAW8` / `.Raw8` |
| `eg.IRR8` | Irradiance | `.IRR8` |

### ASCII exports

| File | Format | Notes |
|---|---|---|
| `avantes_export.ttt` | Transmittance text export | `.ttt` |
| `avantes_export2.trt` | Sample-count text export | `.trt`; one `Sample` counts column |
| `avantes_export_long.ttt` | Multi-signal transmittance text export | `Dark`, `Ref`, `Sample` and `Transmittance` columns |
| `avasoft8.txt` | AvaSoft 8 ASCII export | Tab-separated |

## Parser hints

- Reference reader: R [`lightr`](https://github.com/ropensci/lightr) (see [`lr_parse_avantes_trm()`](https://docs.ropensci.org/lightr/reference/lr_parse_avantes_trm.html)).
- **No maintained Python reader exists** for AvaSoft 8 binaries. v1 options: (a) `rpy2`-wrap `lightr`, (b) port `lightr`'s parsers, (c) ingest ASCII exports only.
- The official spec for AvaSoft 8 is the [AvaSoft 8 manual (PDF)](https://www.avantes.com/content/uploads/2022/02/020379-AvaSoft-8-Manual.pdf).
- Apogee USB instruments share similar extensions but are **not** the same family — sniff by header signature.
