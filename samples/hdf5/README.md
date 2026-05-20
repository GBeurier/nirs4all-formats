# HDF5 generic (`.h5` / `.hdf5`)

Generic hierarchical container. Vendor schema layered on top determines whether a particular HDF5 file is a NIRS dataset.

## Samples

| File | Size | Source | License |
|---|---|---|---|
| `synthetic_nirs.h5` | ~50 KB | Generated locally | CC-0 | A minimally idiomatic NIRS HDF5: `/spectra` (samples × bands, gzip-compressed), `/wavelengths`, `/protein`, plus `units` attributes and `instrument` root attribute. |
| `vlen_string_dset.h5` | 6 KB | [`h5py/h5py@master/h5py/tests/data_files/vlen_string_dset.h5`](https://github.com/h5py/h5py/blob/master/h5py/tests/data_files/vlen_string_dset.h5) | BSD-3-Clause | Canonical h5py test fixture — used here for negative-path tests (HDF5 file that is *not* a NIRS dataset). |

## Parser hints

- Reference readers: `h5py`, `pytables` / `tables`.
- Schema detection: walk the tree, look for a 2-D dataset with one dimension matching a 1-D `wavelengths`/`wavelength_nm`/`x` dataset. Heuristic guidance lives in `nirs_loader/dispatchers/hdf5.py`.
- Vendor-specific schemas (FGI, Allotrope ADF, custom in-house) need dedicated mappers — generic HDF5 should be a last-resort fallback.
