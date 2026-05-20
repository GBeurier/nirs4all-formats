# NetCDF NIRS Datasets

Status: experimental.

The NetCDF reader uses the pure-Rust `netcdf-reader` crate. It currently maps
simple NIRS NetCDF datasets with:

- a 2-D `spectra` variable shaped `sample x wavelength`;
- a 1-D `wavelengths` axis variable matching the spectral dimension;
- optional 1-D target variables matching the sample dimension.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Targets |
|---|---:|---|---|---|
| `samples/netcdf/synthetic_nirs.nc` | 50 | wavelength, `nm`, 200 points | `absorbance` | `protein` |

Global attributes are preserved under `metadata.global_attributes`. The reader
emits one `SpectralRecord` per sample row.

## Dispatch Boundaries

NetCDF is a container. The reader probes NetCDF classic and HDF5-backed
containers, then validates the NIRS schema at read time. ANDI/MS and weather
NetCDF samples are refused because they do not contain a `spectra` variable
with a matching wavelength axis.
