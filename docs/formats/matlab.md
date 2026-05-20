# MATLAB MAT Datasets

Status: experimental.

The MATLAB reader covers simple matrix-style and selected academic structured
NIRS datasets in Rust:

- MAT v5 numeric arrays via the pure-Rust `matfile` crate;
- MAT v5 Eigenvector-style object/struct datasets via a targeted native
  parser for numeric arrays, char arrays, cells, structs/objects, zlib
  compression and little/big endian payloads;
- MATLAB v7.3 HDF5 files via `hdf5-reader`;
- an `X` matrix, a wavelength axis named `wavelengths`, `wavelength`,
  `wavelength_nm` or `x`, and an optional numeric `y` target vector.

MAT v5 arrays are stored in MATLAB column-major order. MATLAB v7.3/HDF5 fixtures
often expose `X` as `bands x samples`; the reader detects both orientations and
emits one `SpectralRecord` per sample.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Targets |
|---|---:|---|---|---|
| `samples/matlab/synthetic_nirs_v5.mat` | 50 | wavelength, `nm`, 200 points | `absorbance` | `y` |
| `samples/matlab/synthetic_nirs_v73.mat` | 50 | wavelength, `nm`, 200 points | `absorbance` | `y` |
| `samples/matlab/eigenvector_corn.mat` | 80 | wavelength, `nm`, 700 points | `m5spec`, `mp5spec`, `mp6spec` | `moisture`, `oil`, `protein`, `starch` |
| `samples/matlab/eigenvector_nir_shootout_2002.mat` | 655 | wavelength, `nm`, 650 points | `instrument_1`, `instrument_2` | `weight`, `hardness`, `assay` |
| `samples/matlab/scpdata_dso.mat` | 20 | wavenumber, `cm-1`, 426 points | `absorbance` | none |
| `samples/matlab/scpdata_als2004dataset.MAT` | 204 | index axis, 96 points | `signal` | `component_1` ... `component_4` |

## Dispatch Boundaries

Supported Eigenvector Data Set Objects are mapped by explicit dataset schema.
Unknown MATLAB structs/objects are not treated as generic numeric arrays,
because their spectral matrix, labels, axis scales and target columns live
inside nested objects rather than top-level `X`/`wavelengths` arrays.

`.RData` files remain outside this reader and need a dedicated R serialization
mapper.
