# NetCDF NIRS Datasets

Status: experimental.

The NetCDF reader uses the pure-Rust `netcdf-reader` crate. It currently maps
simple NIRS NetCDF datasets with:

- a 2-D `spectra` variable shaped `sample x wavelength`;
- a 1-D `wavelengths` axis variable matching the spectral dimension;
- optional 1-D target variables matching the sample dimension.

It also carries two schema-specific atmospheric/sun-photometer paths that are
not generic NetCDF NIRS datasets:

- Microtops MAN AOT series;
- local ARM MFRSR b1 7-channel irradiance/voltage/ratio time series.
- local ARM SURFSPECALB derived 6-filter surface-albedo time series.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Targets |
|---|---:|---|---|---|
| `samples/netcdf/synthetic_nirs.nc` | 50 | wavelength, `nm`, 200 points | `absorbance` | `protein` |
| `samples/microtops/microtops_arc_msm114_2.nc` | 378 | wavelength, `nm`, 5 AOT channels | `aot`, `aot_std` | none |
| `samples_local/mfr/arm_mfrsr_sgp_E11_20210329.nc` | 4,320 | wavelength, `nm`, 7 filters | hemispheric/diffuse/direct irradiance, alltime voltage, direct/diffuse ratio | none |
| `samples_local/netcdf/arm_nsa_surfspecalb_20160609.nc` | 986 | wavelength, `nm`, 6 filters | `surface_albedo` | none |

Global attributes are preserved under `metadata.global_attributes` when the
pure-Rust metadata stack can decode them. The reader emits one `SpectralRecord`
per sample row or per non-missing time row for derived time-series products.

The Microtops MAN NetCDF path is intentionally narrower than the generic NIRS
schema. The committed PANGAEA MSM114/2 fixture is NetCDF4/HDF5 with contiguous
`aot_380`, `aot_440`, `aot_500`, `aot_675` and `aot_870` series plus matching
`*_std` series. Current pure-Rust NetCDF/HDF5 metadata reconstruction cannot
resolve this file, so the fixture is decoded through a SHA-256-guarded fallback
and emits `microtops_man_netcdf_known_fixture_layout`.

The ARM MFRSR path is validated locally only. It maps filter variables
`*_filter1..7` onto a wavelength axis from `centroid_wavelength` attributes,
emits one record per `time` row, and preserves per-signal QC arrays in metadata.
The ARM SURFSPECALB path is also local-only and adjacent: it maps the derived
`surface_albedo_mfr_narrowband_10m(time, filter)` product, drops rows where all
filters are missing (`-9999`), and emits a reflectance-like `surface_albedo`
signal.

## Dispatch Boundaries

NetCDF is a container. The reader probes NetCDF classic and HDF5-backed
containers, then validates the NIRS schema at read time. ANDI/MS containers are
detected by their chromatography/MS variable set and refused with a dedicated
message. Other non-NIRS NetCDF files, such as weather datasets and the
committed PyrNet pyranometer fixture, are refused because they do not contain a
supported NIRS schema or known sun-photometer channel set. Local ARM AOSMET is a
weather product and remains a refusal case.
