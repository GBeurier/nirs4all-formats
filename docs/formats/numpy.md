# NumPy NPY / NPZ

> **Status:** Supported · **Vendor:** Python / NumPy · **Extensions:** `.npy`, `.npz`

Native Rust reader for NumPy array datasets, the common interchange shape for ML
workflows that hand spectra to and from Python. A `.npy` file holds a single
array; a `.npz` file is a ZIP archive of named arrays following a canonical
spectra/axis/target layout.

## Instruments & software

Vendor-neutral; produced by `numpy.save` / `numpy.savez` and consumed across the
Python scientific stack. Especially useful for the Python binding round-trip.

## File structure

- **`.npy`** — detected by the NumPy magic (`\x93NUMPY`). The reader decodes
  v1/v2/v3 headers, C-order arrays of numeric dtypes (`f4`/`f8`/`i*`/`u*`) and
  fixed-width byte (`S*`) / Unicode (`U*`) string arrays; little- and big-endian
  are both handled. Fortran-order arrays are refused.
- **`.npz`** — detected by the ZIP magic (`PK\x03\x04`) plus extension. The
  canonical members are `X.npy` (spectra matrix, rows = samples; required),
  `wavelengths.npy` (optional axis), `y.npy` (optional target vector) and
  `sample_ids.npy` (optional string identifiers). Member lengths are validated
  against `X`.

## What nirs4all-formats extracts

- **Signals** — one `SpectralRecord` per row of the spectra matrix, each with a
  single signal named `spectrum`, typed `Unknown` (the raw array carries no
  signal-type declaration).
- **Axis** — for `.npz` with `wavelengths.npy`, unit `nm`, kind `Wavelength`.
  Otherwise a generated index axis (kind `Index`) is emitted with a warning
  (`numpy_npy_axis_generated_index` for bare `.npy`,
  `numpy_npz_axis_generated_index` for `.npz` without wavelengths).
- **Targets** — the optional `y.npy` vector is exported as target `y`.
- **Metadata** — `sample_id` (from `sample_ids.npy` or a generated `row_N`), plus
  a `numpy` object recording container, shape, row index and row/column counts.
- **Provenance** — source file + SHA-256, reader name and version.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| `.npy` 1D/2D numeric matrix | Supported | Generated index axis + warning. |
| `.npz` with `X` + `wavelengths` | Supported | `nm` wavelength axis. |
| `.npz` with optional `y` / `sample_ids` | Supported | Target `y` and string identifiers. |
| `.npy`/`.npz` string arrays (`S*` / `U*`) | Supported | Decoded for `sample_ids` only. |
| Fortran-order arrays | Detected / refused | Refused with an explicit error. |
| Non-numeric spectra arrays | Detected / refused | Refused with an explicit error. |

## Limitations & known gaps

- Bare `.npy` matrices carry no axis or metadata, so they get a generated index
  axis; companion metadata files for a standalone `.npy` matrix are not read.
- The spectra array must be 1D or 2D and numeric; higher-rank, object and
  Fortran-order arrays are refused rather than guessed.
- Signal type is always `Unknown` because the array format does not record
  absorbance/reflectance/etc.

## Reference readers

`numpy.load` reads the same files; nirs4all-formats adds the canonical `.npz` schema
mapping, axis/target handling, signal naming and provenance.

## Samples & validation

Fixtures live under `samples/numpy/`, covered by golden summaries in
`crates/nirs4all-formats/tests/goldens/` (`numpy_*`):
`synthetic_nirs_X.npy` yields 50 records over a 200-point generated index axis
with no targets, and `synthetic_nirs.npz` yields 50 records over a 200-point
`nm` wavelength axis with target `y`. The probe reports format `numpy-npy` /
`numpy-npz` at `Confidence::Definite`.
