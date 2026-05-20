# Generic HDF5 NIRS Datasets

Status: experimental.

The HDF5 reader uses the pure-Rust `hdf5-reader` crate. It is a schema-aware
fallback for HDF5 containers and currently maps simple NIRS layouts with:

- a 2-D `spectra` dataset shaped `sample x wavelength`;
- a 1-D axis dataset named `wavelengths`, `wavelength`, `wavelength_nm`,
  `wavenumbers`, `wavenumber` or `x`;
- optional 1-D numeric target datasets matching the sample dimension.

The reader searches the root group first, then nested groups up to four levels.
It emits one `SpectralRecord` per sample row.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Targets / metadata |
|---|---:|---|---|---|
| `samples/hdf5/synthetic_nirs.h5` | 50 | wavelength, `nm`, 200 points | `absorbance` | `protein`, root attributes |
| `samples/fgi/synthetic_fgi.h5` | 50 | wavelength, `nm`, 200 points | `absorbance` | group attributes from `/Measurement1` |

## Dispatch Boundaries

Generic HDF5 is intentionally conservative. HDF5 files without a `spectra`
dataset and matching spectral axis are refused. FGI XML sidecars, MATLAB v7.3
`.mat` files and Allotrope ADF remain separate schema mappers even when their
payloads are HDF5-backed.
