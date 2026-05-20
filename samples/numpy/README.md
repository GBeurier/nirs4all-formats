# NumPy `.npy` / `.npz`

Common in ML workflows. Already supported in `nirs4all`.

## Samples

All generated locally (CC-0):

| File | Notes |
|---|---|
| `synthetic_nirs_X.npy` | Bare `(50, 200)` float32 spectra array — no metadata. |
| `synthetic_nirs.npz` | Compressed archive with `X` (spectra), `wavelengths`, `y` (protein), `sample_ids`. The recommended format for sharing NIR ML datasets without a dependency on pandas/HDF5. |

## Parser hints

- Reference readers: `numpy.load`.
- `.npy` is a single array — the loader cannot infer wavelengths / sample IDs, so it should accept companion args or refuse.
- `.npz` is a dict of arrays — look for the canonical keys (`X`, `wavelengths`, `y`).
