# NumPy NPY / NPZ

Native Rust reader for NumPy array datasets used in ML workflows.

## Scope Implemented

- Sniffs `.npy` files by NumPy magic and `.npz` files by ZIP magic plus
  extension.
- Decodes NPY v1/v2/v3 C-order arrays with numeric dtypes
  `f4/f8/i*/u*`.
- Decodes fixed-width string arrays (`S*`) and Unicode arrays (`U*`) for
  `sample_ids`.
- Reads bare `.npy` 1D/2D numeric arrays as spectra with a generated index
  axis.
- Reads canonical `.npz` archives with:
  - `X.npy`: spectra matrix, rows are samples;
  - `wavelengths.npy`: optional spectral axis in `nm`;
  - `y.npy`: optional target vector exported as target `y`;
  - `sample_ids.npy`: optional sample identifiers.
- Refuses Fortran-order arrays and non-numeric spectra arrays.

## Record Mapping

- one `SpectralRecord` per row of `X`;
- signal name: `spectrum`;
- `.npz` axis: `wavelength`, `nm` when `wavelengths.npy` is present;
- `.npy` axis: generated `index` axis with warning
  `numpy_npy_axis_generated_index`;
- metadata: `sample_id` and `numpy` object with container, shape and row
  indices;
- targets: optional `y`.

## Fixtures and Reference Checks

Committed fixtures:

| File | Expected output |
|---|---|
| `samples/numpy/synthetic_nirs_X.npy` | 50 records, 200 index points, no targets |
| `samples/numpy/synthetic_nirs.npz` | 50 records, 200 wavelength points, target `y` |

Reference reader: `numpy.load`.

## Missing / Next Work

- Add object-array refusal tests if such files are contributed.
- Add support for explicit companion metadata files if a bare `.npy` matrix is
  used in production.
