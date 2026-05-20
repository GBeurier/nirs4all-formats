# NetCDF / ANDI `.cdf` / `.nc`

ASTM E1947 ANDI is the **chromatography-MS** standard, not NIR/FTIR. We list NetCDF as an *adjacent* format for inspiration and to support generic-NetCDF NIRS exporters.

## Samples

| File | Size | Source | License |
|---|---|---|---|
| `synthetic_nirs.nc` | ~50 KB | Generated locally | CC-0 | NetCDF3 classic file with `sample` and `wavelength` dimensions, `spectra`/`wavelengths`/`protein` variables, and `units` attributes. |
| `f03tst_open_mem.nc` | 6 KB | [`Unidata/netcdf-c@main/nc_test/f03tst_open_mem.nc`](https://github.com/Unidata/netcdf-c/blob/main/nc_test/f03tst_open_mem.nc) | BSD-3-Clause | Canonical Unidata test fixture — for negative-path tests (NetCDF file that is *not* a NIRS dataset). |
| `air_temperature.nc` | 7.4 MB | [`pydata/xarray-data@master/air_temperature.nc`](https://github.com/pydata/xarray-data/blob/master/air_temperature.nc) | (no SPDX — xarray sample) | NCEP reanalysis temperature data. Another non-NIRS NetCDF for negative-path tests. |

## Parser hints

- Reference readers: `netCDF4`, `scipy.io.netcdf_file`, `xarray`; `nirs4all-io` uses the pure-Rust `netcdf-reader` crate for the native path.
- For NetCDF3 use `scipy.io.netcdf_file` (no native HDF5 dep). For NetCDF4 (HDF5-backed) you need `netCDF4` or `h5netcdf`.
- ANDI MS files have a specific variable structure (`scan_acquisition_time`, `intensity_values`, …). Detecting them is straightforward — refuse with a clear pointer to `pyteomics`.
