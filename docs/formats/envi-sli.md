# ENVI Spectral Library / Standard Cubes

Experimental native Rust reader for ENVI Spectral Library sidecars and a
minimal ENVI Standard cube-to-point-spectra path.

## Scope Implemented

- Sniffs `.hdr` files whose header starts with `ENVI` and declares
  `file type = ENVI Spectral Library` or `file type = ENVI Standard`.
- Opens either the `.hdr` path or the paired `.sli` binary path.
- Resolves the binary payload from `data file = ...` when present, otherwise
  from the sibling `.sli`/`.SLI` file.
- Decodes one-band BSQ spectral-library payloads with ENVI `data type = 4`
  (`float32`) and `data type = 5` (`float64`), little- or big-endian.
- Decodes ENVI Standard BSQ/BIL/BIP cubes from `.img` or `.dat` sidecars with
  integer and float ENVI scalar dtypes currently used by the fixtures.
- Uses `wavelength = { ... }` plus `wavelength units` to build the spectral
  axis.
- Emits one `SpectralRecord` per `spectra names` entry, with one `spectrum`
  signal and `metadata.sample_id`.
- Emits one `SpectralRecord` per ENVI Standard pixel, with `pixel_x`,
  `pixel_y` and optional map coordinates in metadata.

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
  - `spatial_x`, `spatial_y`, `spatial_unit` when `map info` is present;
  - `envi` object with dimensions, interleave, data type, byte order and map
    info.

## Fixtures and Reference Checks

Current fixture:

- `samples/envi_sli/synthetic_lib.hdr`
- `samples/envi_sli/synthetic_lib.sli`
- `samples/envi_sli/cubescope-mini-cube.hdr`
- `samples/envi_sli/cubescope-mini-cube.img`

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
clean-room and uses the local synthetic fixture as the committed golden.

Cube fixture checks:

- 48 by 48 pixels, 32 spectral bands;
- first pixel first/last values: `100`, `3223`;
- last pixel first/last values: `152`, `3275`;
- map coordinates are read from `map info`.

## Next Work

- Add conformance output from Spectral Python once the optional dependency is
  present in the reverse-engineering environment.
- Add a real USGS/ECOSTRESS `.sli` fixture if license-compatible.
- Add tests for non-zero `header offset`, big-endian payloads and `float64`.
- Add real Specim/HySpex/Headwall/NEON cube fixtures and an explicit
  mask/ROI extraction API; the current path is whole-cube pixel expansion only.
