# NumPy `.npy` / `.npz`

Common in ML workflows. Supported in `nirs4all` and by the native Rust
`nirs4all-io` reader for bare numeric `.npy` matrices and canonical `.npz`
datasets.

## Samples

All generated locally (CC-0):

| File | Notes |
|---|---|
| `synthetic_nirs_X.npy` | Bare `(50, 200)` float32 spectra array — no metadata. |
| `synthetic_nirs.npz` | Compressed archive with `X` (spectra), `wavelengths`, `y` (protein), `sample_ids`. The recommended format for sharing NIR ML datasets without a dependency on pandas/HDF5. |

## Parser hints

- Reference readers: `numpy.load`.
- `.npy` is a single array; the native reader emits a generated index axis.
- `.npz` is a dict of arrays; the native reader looks for canonical keys
  (`X`, `wavelengths`, `y`, `sample_ids`).
