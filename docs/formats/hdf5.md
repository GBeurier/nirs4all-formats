# Generic HDF5 NIRS Datasets

Status: experimental.

The HDF5 reader uses the pure-Rust `hdf5-reader` crate. It is a schema-aware
fallback for HDF5 containers and currently maps simple NIRS layouts with:

- a 2-D spectral dataset shaped `sample x wavelength`, or `wavelength x
  sample` when the spectral axis matches only the first dimension;
- spectral dataset names such as `spectra`, `spectrum`, `X`, `absorbance`,
  `reflectance`, `transmittance`, `intensity`, `raw`, `counts` or `data`;
- a 1-D axis dataset named `wavelengths`, `wavelength`, `wavelength_nm`,
  `wl`, `lambda`, `wavenumbers`, `wavenumber`, `wn`, `x_axis`, `axis` or
  related `*_nm` / `*_cm-1` aliases;
- optional 1-D numeric target datasets matching the sample dimension.

The reader searches the root group first, then nested groups up to four levels.
It emits one `SpectralRecord` per sample row.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Targets / metadata |
|---|---:|---|---|---|
| `samples/hdf5/synthetic_nirs.h5` | 50 | wavelength, `nm`, 200 points | `absorbance` | `protein`, root attributes |
| `samples/hdf5/generic_aliases_data_group.h5` | 3 | wavenumber, `cm-1`, 4 points | `absorbance` from `/data/absorbance` stored `bands x samples` | `temperature`, root attributes |
| `samples/fgi/synthetic_fgi.h5` | 50 | wavelength, `nm`, 200 points | `absorbance` | group attributes from `/Measurement1` |

## Dispatch Boundaries

Generic HDF5 is intentionally conservative. HDF5 files without a recognized
2-D spectral dataset and matching 1-D spectral axis are refused. Transposed
matrices are accepted only when the axis length identifies the band dimension
without ambiguity. FGI XML sidecars are handled by the dedicated
`fgi-hdf5-xml` reader, while MATLAB v7.3 `.mat` files and Allotrope ADF use
separate schema mappers even when their payloads are HDF5-backed.
