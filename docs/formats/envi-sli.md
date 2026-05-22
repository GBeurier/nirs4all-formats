# ENVI Spectral Library / Standard Cubes

Experimental native Rust reader for ENVI Spectral Library sidecars and a
minimal ENVI Standard cube-to-point-spectra path.

## Scope Implemented

- Sniffs `.hdr` files whose header starts with `ENVI` and declares
  `file type = ENVI Spectral Library` or `file type = ENVI Standard`.
- Opens either the `.hdr` path or the paired `.sli` / `.img` binary path.
- Resolves the binary payload from `data file = ...` when present,
  otherwise from the sibling file:
  - **SLI**: fallback extensions `.sli` / `.SLI` only (the `data file`
    LDR is honoured first for explicit overrides).
  - **Standard**: fallback extensions `.img` / `.IMG` / `.dat` / `.DAT`.
- Decodes payloads via the shared `decode_numeric_payload` helper, which
  covers ENVI scalar `data type` values 1, 2, 3, 4, 5, 12, 13, 14 and 15
  (u8 / i16 / i32 / f32 / f64 / u16 / u32 / i64 / u64) in either byte
  order. The SLI path additionally requires `bands == 1` and
  `interleave == bsq`; ENVI Standard supports BSQ/BIL/BIP.
- Uses `wavelength = { ... }` plus `wavelength units` to build the spectral
  axis.
- Emits one `SpectralRecord` per `spectra names` entry, with one `spectrum`
  signal and `metadata.sample_id`.
- Emits one `SpectralRecord` per ENVI Standard pixel, with `pixel_x`,
  `pixel_y`, row-slowest/X-fastest order and optional parsed `map info`
  coordinates in metadata.
- Supports optional half-open row/column windows through
  `open_path_with_options()` and the CLI `read-json --rows START:END --cols
  START:END`, while preserving original pixel coordinates in record metadata.
- Supports optional sparse pixel masks through
  `ReadOptions::with_cube_mask(CubeMask::new(...))` and the CLI
  `read-json --pixel ROW,COL ...` / `--pixels-file PATH`. Pixels are emitted in
  caller order; duplicates are preserved so callers can describe ordered sample
  paths.

## Record Mapping

Each spectrum in a spectral library becomes one record:

- signal name: `spectrum`;
- record signal type: `unknown` unless a future fixture provides an explicit
  unit or semantic field;
- axis: wavelength in `nm` for the current fixture;
- metadata:
  - `sample_id` from `spectra names`;
  - `envi` object with dimensions, data type, interleave, byte order,
    wavelength units and sensor type.
- provenance sources:
  - header sidecar as role `header`;
  - binary payload as role `binary`.

Each ENVI Standard cube pixel also becomes one record:

- signal name: `spectrum`;
- axis: wavelengths from the ENVI `wavelength = { ... }` vector;
- metadata:
  - `sample_id` as `pixel_y{row}_x{col}`;
  - `pixel_x`, `pixel_y`;
  - `spatial_x`, `spatial_y`, normalized `spatial_unit` when `map info` is
    present;
  - `map_axis_order`, projection, reference pixel/map coordinates, pixel sizes,
    zone, hemisphere and datum when they are present in `map info`;
  - `envi` object with dimensions, interleave, data type, byte order, raw map
    info, parsed map info and coordinate system string.

## Fixtures and Reference Checks

Current fixture:

- `samples/envi_sli/synthetic_lib.hdr`
- `samples/envi_sli/synthetic_lib.sli`
- `samples/envi_sli/cubescope-mini-cube.hdr`
- `samples/envi_sli/cubescope-mini-cube.img`
- `samples/envi_sli/usgs_splib06a_aviris95_envi.hdr`
- `samples/envi_sli/usgs_splib06a_aviris95_envi.sli`
- `samples/envi_sli/usgs_splib07_aviris95_envi.hdr`
- `samples/envi_sli/usgs_splib07_aviris95_envi.sli`

Expected shape:

- 50 records;
- 200 wavelength points per record;
- wavelength axis 1100.0 to 2500.0 nm;
- first record first value: `0.0367427170`;
- last record last value: `0.0608757548`.

Additional fixed control points for the current little-endian float32 fixture:

| Record | `y[0]` | `y[50]` | `y[100]` | `y[199]` |
|---|---:|---:|---:|---:|
| `S000` | `0.0367427170` | `0.3736431003` | `0.0838314667` | `-0.1465858221` |
| `S001` | `0.0118803680` | `0.2504318655` | `0.1165727973` | `0.0967217237` |
| `S010` | `-0.0723795667` | `0.6311128736` | `0.3589301407` | `0.0539555252` |
| `S049` | `0.1409859657` | `0.4480262399` | `0.4442021549` | `0.0608757548` |

The format is documented by ENVI and supported by Spectral Python
(`spectral`) and R `RStoolbox::readSLI()`. The current Rust reader is
clean-room. The synthetic library is the committed golden, while the USGS
splib06/07 fixtures are tested through both `.hdr` and direct `.sli` entry
paths.

Cube fixture checks:

- 48 by 48 pixels, 32 spectral bands;
- header and direct `.img` entry paths both dispatch to `envi-standard-cube`;
- ROI window `rows=2:4`, `cols=3:6` emits 6 records while preserving
  `pixel_y2_x3` through `pixel_y3_x5` coordinates and matching the full-cube
  spectra for those pixels;
- sparse mask `[(47,47), (0,0), (12,7)]` emits 3 records in caller order whose
  values match the corresponding pixels from the full-cube expansion, while the
  empty-mask and out-of-bounds-pixel paths return descriptive errors;
- first pixel first/last values: `100`, `3223`;
- last pixel first/last values: `152`, `3275`;
- map coordinates are read from `map info`, with `units=Meters` normalized to
  `spatial_unit = "m"` and UTM zone/hemisphere/datum promoted.

USGS AVIRIS95 spectral-library checks:

- `splib06a`: 1365 records, 224 wavelength points, `um` axis
  `0.38315..2.5082`, first sample `Acmite NMNH133746 Pyroxene s06av95a=a`.
- `splib07`: 3139 records, same 224-point axis, first two rows are wavelength
  and resolution metadata spectra as stored in the upstream library.
- `data ignore value` sentinels are currently preserved as numeric values
  rather than converted to missing values.

## Sidecar contract (M1, 2026-05-22)

ENVI is a sidecar-bearing format: the `.hdr` header and the binary
payload travel as a pair. Three entry points cover decoding:

- `open_path(path)` reads the `.hdr` + binary from disk.
- `open_with_sidecars(name, bytes, Arc<dyn SidecarResolver>)` decodes
  from in-memory bytes; pass either the `.hdr` text or the `.sli`/`.img`
  binary as the primary and supply the companion in the resolver.
- `open_bytes(name, bytes)` returns `Error::UnsupportedSidecar`.

## Next Work

- Add conformance output from Spectral Python once the optional
  dependency is present in the reverse-engineering environment.
- Add tests for non-zero `header offset` and big-endian payloads.
- Add real Specim/HySpex/Headwall/NEON cube fixtures to exercise the
  rectangular and sparse-mask extraction paths on production-scale
  layouts.
