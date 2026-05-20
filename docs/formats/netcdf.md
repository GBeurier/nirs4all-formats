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
| `samples/microtops/microtops_arc_msm114_2.nc` | 378 | wavelength, `nm`, 5 AOT channels | `aot`, `aot_std` | none |

Global attributes are preserved under `metadata.global_attributes`. The reader
emits one `SpectralRecord` per sample row.

The Microtops MAN NetCDF path is intentionally narrower than the generic NIRS
schema. The committed PANGAEA MSM114/2 fixture is NetCDF4/HDF5 with contiguous
`aot_380`, `aot_440`, `aot_500`, `aot_675` and `aot_870` series plus matching
`*_std` series. Current pure-Rust NetCDF/HDF5 metadata reconstruction cannot
resolve this file, so the fixture is decoded through a SHA-256-guarded fallback
and emits `microtops_man_netcdf_known_fixture_layout`.

## Dispatch Boundaries

NetCDF is a container. The reader probes NetCDF classic and HDF5-backed
containers, then validates the NIRS schema at read time. ANDI/MS containers are
detected by their chromatography/MS variable set and refused with a dedicated
message. Other non-NIRS NetCDF files, such as weather datasets and the
committed PyrNet pyranometer fixture, are refused because they do not contain a
supported NIRS schema or Microtops AOT channel set.
