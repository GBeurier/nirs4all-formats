# Renishaw WDF

> **Status:** Supported (scoped) · **Vendor:** Renishaw · **Extensions:** `.wdf`

WDF is the native chunked container written by Renishaw's WiRE software for inVia
confocal Raman spectrometers. It stores single spectra, maps, line scans, depth
profiles, time series and StreamLine acquisitions in one file. nirs4all-formats reads
the spectral payload subset and emits one record per stored spectrum. The format
is Raman, adjacent to the core NIRS point-spectrum scope; it is included for
spectroscopy interchange and disambiguation.

## Instruments & software

Produced by Renishaw WiRE on inVia (including Qontor and Apollo) Raman
microscopes. Spatial maps, line scans, depth/Z scans, time series and StreamLine
fast-mapping acquisitions all share the WDF container.

## File structure

A WDF file is a sequence of named blocks. Every block starts with a 16-byte
header (`name[4]`, `block_uid: u32le`, `block_size: u64le`), and the leading
`WDF1` block is a fixed 512-byte file header. The reader sniffs the `WDF1` magic
(never the extension) and reads:

- `WDF1` fixed header — point count, capacity, stored spectrum count,
  accumulation count, scan and measurement type, spectral unit, laser
  wavenumber, application name/version, user and title;
- `DATA` — float32 ordinate payload (one spectrum after another);
- `XLST` — float32 spectral axis with leading data-type and unit codes;
- `YLST` — ordinate unit metadata when present;
- `ORGN` — navigation axes (spatial X/Y/Z, FocusTrack Z, acquisition time,
  exposure time, multiwell coordinates);
- `WMAP` — 48-byte map descriptor (map type, XYZ offsets/scales/sizes,
  linefocus size);
- `WHTL` — white-light image container metadata (not the pixel payload);
- `MAP ` — derived analysis-block inventory with bounded `dataRange` extraction.

## What nirs4all-formats extracts

- **Signals** — one `SpectralRecord` per stored spectrum, each with a single
  `raw_counts` signal typed `RawCounts` (counts unit when declared by `YLST`).
- **Axis** — values and unit from `XLST`; unit code `1` maps to wavenumber
  (`cm-1`), codes `2`/`3` to wavelength (`nm`), others to an `Index` axis.
- **Metadata** — container fields plus per-record navigation copied from `ORGN`:
  `spatial_x/y/z`, `focus_track_z`, `time_filetime_100ns`,
  `elapsed_time_seconds` and exposure/multiwell fields. `WMAP` adds `map_width`,
  `map_height`, `map_x_index`, `map_y_index` and, for `xyline` maps,
  `spatial_distance`. `WHTL` adds a `white_light_image` object (JPEG MIME type,
  byte length, SHA-256, dimensions, JFIF density, EXIF fields). `MAP ` blocks add
  `map_analysis_blocks`, and `map_analysis_values` per record when a decoded
  `dataRange` length matches the spectrum count.
- **Provenance & warnings** — every record carries
  `renishaw_wdf_reverse_engineered_chunks`; interrupted acquisitions and
  unnormalised map types are flagged as warnings.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Single spectrum | Supported | One record. |
| Regular map / line scan (`xyline`) / depth (Z) scan | Supported | WMAP-derived X/Y indices; line scans expose `spatial_distance`. |
| Time series | Supported | Elapsed time derived from FILETIME origin axis. |
| FocusTrack acquisitions | Supported | FocusTrack Z copied to metadata. |
| StreamLine fast mapping | Supported | Column-major map; one record per stored spectrum. |
| Interrupted acquisition | Supported | Reads the stored count, preserves positions, warns about truncated capacity. |
| `MAP ` PSET analysis maps | Supported (scoped) | Observed `dataRange` tail layout decoded as per-record metadata, not spectral signals; other layouts stay inventory-only. |
| `measurement_type=0` containers | Detected / refused | Refused with an explicit undefined-measurement error. |

## Limitations & known gaps

- The reader covers the spectral payload subset; `WHTL` images are described but
  pixels are not decoded into records.
- Derived `MAP ` analysis values are surfaced as metadata only, because they are
  WiRE-computed output rather than primary acquisition data; only the observed
  `f32le`-tail PSET layout is decoded.
- Authoritative units/algorithms for derived blocks, broader `MAP ` layouts and
  per-model inVia Qontor/Apollo fixtures are still wanted.
- Full-array conformance is pending (see below).

## Reference readers

Layout and conventions cross-checked against `rsciio.renishaw` 0.13.0 (broad WDF
coverage, ORGN/WMAP conventions), `renishawWiRE` 0.1.16 / `py-wdf-reader` (the
single-spectrum and ORGN path) and SpectroChemPy `read_wire` (chunk and enum
cross-checking).

## Samples & validation

Fifteen spectral fixtures under `samples/raman_renishaw/` cover single, map,
line, depth/Z, FocusTrack, time-series, StreamLine and interrupted acquisitions;
`renishaw_test_map2.wdf` (400 records) and `wire_depth.wdf` (40 records, with two
`MAP ` PSET `dataRange` analysis maps) are golden-backed. `renishaw_test_undefined.wdf`
and `wire_undefined.wdf` are negative fixtures that fail with the undefined
`measurement_type=0` message. The probe reports `renishaw-wdf` at
`Confidence::Definite`. Full-array external conformance remains future work.
