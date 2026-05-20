# MATLAB MAT Datasets

Status: experimental.

The MATLAB reader covers simple matrix-style NIRS datasets in Rust:

- MAT v5 numeric arrays via the pure-Rust `matfile` crate;
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

## Dispatch Boundaries

MATLAB structs, Eigenvector Data Set Objects and `.RData` files need dedicated
schema mappers. They are not treated as generic numeric arrays, because their
spectral matrix, labels, axis scales and target columns live inside nested
objects rather than top-level `X`/`wavelengths` arrays.
