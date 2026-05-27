# Sun Photometer Text Exports

> **Status:** Supported (scoped) · **Vendor:** Solar Light / YES Inc. · **Extensions:** `.OUT`, `.TXT`, `.csv`, `.lev10`, `.lev15`, `.lev20` (text); `.nc` (NetCDF, behind `fmt-hdf5`)

Sun photometers measure spectral aerosol optical thickness and irradiance at a
handful of fixed filter wavelengths. These are not core NIR lab spectra, but
they appear in the sample corpus and exercise the same normalization contract:
channel wavelengths become the spectral axis and each observation row becomes
one `SpectralRecord`. nirs4all-formats reads the common ASCII exports (MFR, Microtops,
AERONET MAN); the ARM MFRSR and Microtops MAN NetCDF paths live in the NetCDF
reader and require the `fmt-hdf5` feature.

## Instruments & software

Solar Light Microtops II and MFR-7 rotating-shadowband radiometers, the DOE ARM
MFRSR datastream, and AERONET Maritime Aerosol Network (MAN) cruise exports.

## File structure

The ASCII reader recognises three text layouts by content:

- **MFR-7 `.OUT`** — a `MFR-7 Sun Photometer` title and site line, a `Record …`
  header, then fixed-width rows with `Channel_<nm>` columns.
- **Microtops `.TXT`/CSV** — a comma-separated header containing `AOT_1020` /
  `AOT_870`, then one row per observation with `AOT_<nm>` columns.
- **AERONET MAN ASCII** — a `Maritime Aerosol Network` preamble (version/level,
  campaign, data policy, PI line) and a `Date(dd:mm:yyyy)` header with
  `AOD_<nm>nm` (and optional `STD_<nm>nm`) columns.

The NetCDF path (ARM MFRSR b1 and Microtops MAN) is decoded through the
`fmt-hdf5` NetCDF reader.

## What nirs4all-formats extracts

- **MFR `.OUT`** — one record per row, signal `channels` (raw counts) at the
  filter wavelengths (e.g. 415, 500, 614, 673, 870, 940 nm). Record number,
  time and air mass are preserved per record; site/lat/lon/alt from the header.
- **Microtops `.TXT`/CSV** — one record per row, signal `aot` typed
  `aerosol_optical_thickness` at the AOT filter wavelengths (e.g. 1020, 870,
  675 nm). Location, pressure, solar geometry and water-column fields are
  preserved as metadata.
- **AERONET MAN ASCII** — one record per row, signal `aot`
  (`aerosol_optical_thickness`) over the valid 380–870 nm channels; a paired
  `aot_std` signal (typed `uncertainty`) when STD columns are present. Missing
  `-999` channels are omitted from the axis; campaign, level, aggregation, PI
  fields and row metadata are preserved. Records carry the
  `microtops_man_ascii_experimental` warning.
- **NetCDF** (via `fmt-hdf5`) — ARM MFRSR exposes 7-filter
  hemispheric/diffuse/direct irradiance plus voltage and direct/diffuse ratio
  signals, with ARM datastream metadata, filter centroids/FWHM, solar geometry
  and per-signal QC; Microtops MAN NetCDF exposes `aot` and `aot_std`.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| MFR-7 `.OUT` text | Supported | Synthetic fixture validates the parser; a redistributable real `.OUT` is still wanted. |
| Microtops `.TXT`/CSV text | Partial | Synthetic CSV covered; no redistributable legacy `.TXT` field export found. |
| AERONET MAN ASCII (`.lev10/.lev15/.lev20`) | Supported (local-only) | All-points, daily and series aggregations validated on local Okeanos samples; not redistributable (MAN data policy). |
| Microtops MAN NetCDF | Supported | Real PANGAEA fixture; uses a contiguous-layout fallback decoder (see below). |
| ARM MFRSR NetCDF | Experimental (local-only) | One local b1 fixture; broader ARM/xarray conformance pending. |

## Limitations & known gaps

- No atmospheric correction or unit conversion is applied.
- The Microtops MAN NetCDF fixture is discovered as a Microtops `aot_<nm>`
  schema. When the high-level `hdf5-reader` 0.5 API fails to resolve this
  layout's shared attribute heap, the reader falls back to a generic
  contiguous-layout decoder (`DataLayout::Contiguous` blocks via fractal-heap
  link records and `get_or_parse_header(addr)`), emitting
  `microtops_man_netcdf_contiguous_layout_fallback` and, for byte-scanned
  global string attributes, `microtops_man_netcdf_global_attributes_byte_scan`.
  This fallback should disappear once `hdf5-reader` resolves NetCDF4 shared
  attribute heaps cleanly.
- AERONET MAN ASCII and ARM MFRSR NetCDF support are validated on local samples
  only and are not redistributed.
- Redistributable MFR-7 `.OUT` and legacy Microtops `.TXT` field exports, and a
  generic NetCDF path, are still needed.

## Reference readers

Ad-hoc parsers, SPECCHIO and `xarray` / ARM ACT (for the NetCDF datastreams) are
the reference candidates.

## Samples & validation

`samples/mfr/synthetic_mfr.OUT` (50 records) and
`samples/microtops/synthetic_microtops.TXT` (20 records) are golden-backed in
`crates/nirs4all-formats/tests/goldens/`, alongside the real
`samples/microtops/microtops_arc_msm114_2.nc` MAN NetCDF (378 records, PANGAEA
MSM114/2, CC-BY-4.0). Local-only fixtures cover the ARM MFRSR b1 NetCDF (4,320
records × 7 filters, with a QC YAML sidecar attached as a `qc_sidecar` source)
and the AERONET MAN ASCII Okeanos exports. Microtops/MAN `aot` arrays use the
`aerosol_optical_thickness` signal type; `aot_std` uses `uncertainty`. The
probes report `mfr-sun-photometer`, `microtops-sun-photometer` and
`microtops-man-ascii` at `Confidence::Definite`.
