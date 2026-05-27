# Avantes AvaSoft

> **Status:** Supported (scoped) · **Vendor:** Avantes · **Extensions:** ASCII `.ttt`, `.trt`, `.tit`, `.tat`, `.IRR`, `.txt`; legacy binary `.TRM`, `.ABS`, `.ROH`, `.DRK`, `.REF`; AvaSoft 8 binary `.Raw8`, `.IRR8` (and other `*8` suffixes)

AvaSoft is the acquisition software for Avantes fibre-optic spectrometers. It
writes both ASCII exports (recommended for interchange) and per-acquisition
binary files in two generations: the legacy AvaSoft 6/7 single-mode layout and
the AvaSoft 8 `AVS82`/`AVS84` container. nirs4all-formats reads the ASCII exports
fully and decodes the binary layouts covered by committed fixtures.

## Instruments & software

Produced by AvaSoft across the AvaSpec range. ASCII wave tables come from
AvaSoft 6/7 (`Wave;...` columns) and AvaSoft 8 text exports; binary files are the
native per-measurement records. Reference behaviour was compared against the
documented `lightr` parser layout and its formulas; `lightr` is GPL and remains
a conformance reference only — no runtime dependency is linked into the MIT core.

## File structure

- **ASCII wave tables** — a short metadata preamble, then a `Wave;...` header
  row, an optional units row, and semicolon-delimited columns (axis first). The
  delimiter is auto-detected. `.IRR` two-column whitespace files are parsed as a
  single irradiance trace.
- **Legacy binary (AvaSoft 6/7)** — a 400-byte header stored as little-endian
  `float32` values: `version_id` (= 70) at byte 0, spectrometer id and user name
  as float32 ASCII code points, wavelength polynomial coefficients `a0..a4` at
  byte 296, `first_pixel`/`last_pixel` at 316/320, `measure_mode` at 324, then
  the data at byte 400. The axis is `a0 + a1·p + a2·p² + a3·p³ + a4·p⁴` for pixel
  index `p`. Processed modes (`.TRM`, `.ABS`) interleave `scope, white, dark`
  triples; raw modes (`.ROH`, `.DRK`, `.REF`) store one vector plus trailing
  acquisition values.
- **AvaSoft 8 binary** — a 5-byte magic (`AVS82` / `AVS84`), a spectra count, then
  per-subfile headers carrying subfile length, measurement-mode byte,
  start/stop pixel, integration time/delay, averages, SPC date, comment, and
  four float32 payload vectors (`xcoord`, scope, dark, reference). The reader
  advances to the next subfile with `start + length + 10`.

## What nirs4all-formats extracts

- **Signals** — one `SpectralRecord` per file or AvaSoft 8 subfile, axis in `nm`.
  Processed modes derive the primary signal: transmittance/reflectance
  `(scope − dark)/(white − dark)·100`, absorbance `−log10((scope − dark)/(white −
  dark))`. Raw and reference channels are exposed alongside (`sample`,
  `white_reference`, `dark_reference`, `scope`). ASCII `Dark`/`Ref`/`Reference`/
  `Sample` columns are typed `raw_counts`; processed columns keep their
  reflectance/transmittance/irradiance types.
- **Metadata (harmonized top level)** — `measurement_mode`, `point_count`,
  `first_pixel`, `last_pixel`, `integration_time_ms`, `averages_count`,
  `integration_delay`; legacy adds `detector_temperature_c` and `version_id`;
  AvaSoft 8 adds `magic`, `acquisition_start_date`/`acquisition_start_time`
  (decoded from the packed SPC date) and, when populated, `instrument_serial`,
  `operator`, `comment`. Fixed-length C-string fields are cut at the first NUL.
- **Raw vendor block** — `metadata.avantes` preserves byte-level provenance
  (`spec_id`, `user_name`, wavelength coefficients, raw measure-mode byte,
  decoded SPC date, smooth/trigger fields).
- **Diagnostics** — single-channel legacy raw modes carry
  `avantes_legacy_single_channel:<mode>:companion_files_required` (consumers need
  the companion files to recompute processed signals). When the AvaSoft 8
  extension implies a mode that contradicts the observed `measure_mode` byte, the
  record carries `avantes_avasoft8_extension_mode_mismatch:expected=…:observed=…`.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| AvaSoft 6/7 ASCII wave tables (`.ttt`/`.trt`/`.tit`/`.tat`) | Supported | Multi-column axis-first tables; per-column signal typing. |
| Two-column irradiance ASCII (`.IRR`) | Supported | Single irradiance trace. |
| AvaSoft 8 text export (`.txt`) | Supported | Raw dark/reference/sample plus processed signal. |
| Legacy binary `.TRM` / `.ROH` / `.DRK` / `.REF` | Supported (scoped) | Processed transmittance + channels, or single raw channel. |
| Legacy binary `.ABS` | Supported (scoped) | Same layout/formula path; no committed fixture yet. |
| AvaSoft 8 binary `.Raw8` / `.IRR8` | Partial | One subfile per fixture; `IRR8` calibration not yet applied. |
| AvaSoft 8 `.RWD8`/`.ABS8`/`.TRM8`/`.RFL8`/`.RIR8`/`.RMN8`/`.RMD8`, multi-subfile | Planned | Recognised by magic but no committed fixtures yet. |

## Limitations & known gaps

- `IRR8` irradiance calibration is not applied: the primary signal is exposed as
  `irradiance` for discoverability with the
  `avantes_irr8_irradiance_calibration_not_applied` warning, and the fourth
  payload vector is exposed as `irradiance_calibration` (signal type `unknown`,
  values spanning ~1e10 down to ~1e0) rather than a misnamed `white_reference`.
- Many active AvaSoft 8 suffixes lack committed fixtures, and multi-subfile
  AVS8 containers are not yet exercised.
- The remaining AVS8 acquisition fields are preserved raw but not fully
  normalized.

## Reference readers

`lightr` (GPL) is the layout/formula reference for the binary paths, used only
in the isolated conformance lab. ASCII exports are equally readable with
`pandas` and R `read.table`; nirs4all-formats adds axis derivation, signal typing and
provenance. Subprocess conformance reports against `lightr` are planned once the
local R native chain is available.

## Samples & validation

Binary fixtures under `samples/avantes/` are golden-backed and probe-locked per
suffix: `avantes2.TRM` (1442 pts, 275.27→1100.13 nm, transmittance
11.840215→−127.179425), `avantes_trans.TRM` (1623 pts), `avantes_reflect.ROH`
(scope 805.0→774.3), `1305084U1.DRK`/`.REF`, `1904090M1_0003.Raw8` (1019 pts,
`AVS84`, mode 0) and `eg.IRR8` (1620 pts, `AVS84`, mode 4, calibration warning).
ASCII fixtures include `avantes_export.ttt`, `avantes_export2.trt`,
`avantes_export_long.ttt`, `irr_820_1941.IRR` and `avasoft8.txt`. AvaSoft 8 SPC
dates are unpacked into top-level `acquisition_start_date`/`time` metadata while
the raw integer and decoded components remain under `metadata.avantes`.
