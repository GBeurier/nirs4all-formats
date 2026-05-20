# ENVI Spectral Library

Experimental native Rust reader for ENVI Spectral Library sidecars.

## Scope Implemented

- Sniffs `.hdr` files whose header starts with `ENVI` and declares
  `file type = ENVI Spectral Library`.
- Opens either the `.hdr` path or the paired `.sli` binary path.
- Resolves the binary payload from `data file = ...` when present, otherwise
  from the sibling `.sli`/`.SLI` file.
- Decodes one-band BSQ spectral-library payloads with ENVI `data type = 4`
  (`float32`) and `data type = 5` (`float64`), little- or big-endian.
- Uses `wavelength = { ... }` plus `wavelength units` to build the spectral
  axis.
- Emits one `SpectralRecord` per `spectra names` entry, with one `spectrum`
  signal and `metadata.sample_id`.
- Detects `file type = ENVI Standard` image cubes but refuses to load them.

Image cubes are intentionally out of scope for v1. They should be handled later
by a cube extractor that produces point spectra, not by silently flattening
pixels into regular records.

## Record Mapping

Each spectrum in the library becomes one record:

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

## Fixtures and Reference Checks

Current fixture:

- `samples/envi_sli/synthetic_lib.hdr`
- `samples/envi_sli/synthetic_lib.sli`

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

## Next Work

- Add conformance output from Spectral Python once the optional dependency is
  present in the reverse-engineering environment.
- Add a real USGS/ECOSTRESS `.sli` fixture if license-compatible.
- Add tests for non-zero `header offset`, big-endian payloads and `float64`.
- Add a clear cube-extraction API before accepting `ENVI Standard` image cubes.
