# MATLAB `.mat` (and `.RData`)

Already supported by `nirs4all` via `MatlabLoader`. Many academic NIR datasets are distributed as MATLAB structs.

## Samples

| File | Size | Source | License |
|---|---|---|---|
| `synthetic_nirs_v5.mat` | 84 KB | Generated locally | CC-0 | MATLAB v5 file with `X`, `wavelengths`, `y`, `sample_ids` variables — minimal NIR dataset structure. |
| `synthetic_nirs_v73.mat` | 86 KB | Generated locally | CC-0 | MATLAB v7.3 file (HDF5-backed). Tests the HDF5-backed `.mat` path. |
| `eigenvector_corn.mat` | 1.4 MB | [Eigenvector data archive — Corn](https://eigenvector.com/data/Corn/corn.mat) | (Eigenvector public dataset, redistribution permitted for non-commercial research) | The canonical **Corn dataset** (Cargill / Eigenvector). Three NIR spectrometers (m5, mp5, mp6), 80 samples, moisture / oil / protein / starch reference values. Used by virtually every chemometrics tutorial. |
| `eigenvector_nir_shootout_2002.mat` | 6.9 MB | [Eigenvector data archive — NIR Shootout 2002](https://eigenvector.com/data/tablets/nir_shootout_2002.mat) | (Eigenvector public dataset) | **NIR Shootout 2002** pharma tablet dataset — two instruments, calibration / validation / test splits, three property values. |
| `scpdata_dso.mat` | 44 KB | [`spectrochempy/spectrochempy_data@master/testdata/matlabdata/dso.mat`](https://github.com/spectrochempy/spectrochempy_data/blob/master/testdata/matlabdata/dso.mat) | CeCILL-B | Eigenvector "Data Set Object" container — round-trip test. |
| `scpdata_als2004dataset.MAT` | 207 KB | [`spectrochempy/spectrochempy_data@master/testdata/matlabdata/als2004dataset.MAT`](https://github.com/spectrochempy/spectrochempy_data/blob/master/testdata/matlabdata/als2004dataset.MAT) | CeCILL-B | ALS 2004 dataset (Multivariate Curve Resolution benchmark). |
| `prospectr_NIRsoil.RData` | 2.0 MB | [`l-ramirez-lopez/prospectr@master/data/NIRsoil.RData`](https://github.com/l-ramirez-lopez/prospectr/blob/master/data/NIRsoil.RData) | MIT | Classic `NIRsoil` reference dataset shipped with `prospectr`. Not strictly MATLAB but a serialized R dataframe — exposed here for cross-language tests and mapped natively by `nirs4all-formats`. |

## Parser hints

- Reference readers:
  - Python: `scipy.io.loadmat` for v5; `h5py` or `mat73` for v7.3.
  - R: `R.matlab::readMat()`; for `.RData` use `load()` or `pyreadr.read_r()`.
- MATLAB column-major: when read via `h5py` (v7.3 path), shapes are transposed vs. v5 path. Normalize to row-major (samples × bands).
- Eigenvector DSOs wrap data in a "Data Set Object" struct — see Eigenvector docs.
