# MATLAB MAT / R RData Datasets

> **Status:** Supported (scoped) · **Vendor:** MATLAB / R ecosystem · **Extensions:** `.mat`, `.MAT`, `.RData`, `.rda` · **Feature flag:** `fmt-matlab`

MATLAB `.mat` files and R `.RData` workspaces are common carriers for NIRS
matrices, calibration datasets and Eigenvector-style data set objects. This
reader maps simple matrix-style layouts and selected academic structured
datasets in Rust.

## Instruments & software

Vendor-neutral research and ML tooling: MATLAB, the Eigenvector PLS_Toolbox data
sets, and the R `prospectr` package. Committed fixtures include synthetic
matrices, Eigenvector corn / NIR shootout / DSO datasets and the prospectr
`NIRsoil` workspace.

## File structure

The reader is gated behind the `fmt-matlab` feature, which itself requires
`fmt-hdf5` because MATLAB v7.3 is an HDF5 container. Dispatch is by extension
plus content:

- **MAT v5** (`.mat` / `.MAT`, probe `Definite`) — numeric arrays via the
  pure-Rust `matfile` crate, with a targeted native parser for Eigenvector-style
  object/struct datasets (numeric/char arrays, cells, structs/objects, zlib
  compression, little/big endian). Arrays are MATLAB column-major.
- **MAT v7.3** (`.mat` with HDF5 magic, probe `Likely`) — decoded via
  `hdf5-reader`; `X` is often stored `bands x samples`, and both orientations
  are detected.
- **R RData** (`.RData` / `.rda`) — RDX3/XDR streams (probe `Definite`) and
  XZ-compressed workspaces (probe `Likely`) via `rds2rust`, schema-mapped to the
  prospectr `NIRsoil` data.frame and its `spc` matrix.

Single-file `.mat` (v5 and v7.3) and `.RData` all decode in-memory through
`open_bytes` with no companion files. The exception is the Indian Pines cube,
which is sidecar-bearing: `open_path` reads the optional `indian_pines_gt.mat`
from disk, and `open_with_sidecars` lets a resolver serve that ground-truth file
(the GT filename is hard-coded). Plain `open_bytes` still succeeds without it,
dropping the target column.

The expected matrix layout is an `X` spectra matrix, a wavelength axis named
`wavelengths`, `wavelength`, `wavelength_nm` or `x`, and an optional numeric `y`
target vector.

## What nirs4all-io extracts

- **Signals** — the spectra matrix, emitting one record per sample. Eigenvector
  datasets expose their named spectral blocks as separate signals (e.g.
  `m5spec`, `mp5spec`, `mp6spec`; `instrument_1`, `instrument_2`).
- **Axis** — the wavelength / wavenumber axis when present; otherwise a generated
  index axis (e.g. for the Indian Pines cube, which carries no calibration).
- **Targets** — the `y` vector or the dataset's labelled target columns
  (`moisture`, `oil`, `protein`, `starch`; `weight`, `hardness`, `assay`;
  `Nt`, `Ciso`, `CEC`; `land_cover_class`).
- **Metadata & provenance** — container type, axis/orientation hints, source
  file + SHA-256; a provenance warning when an axis is generated for lack of
  wavelength calibration.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| MAT v5 numeric (`X` + axis + `y`) | Supported | Column-major arrays via `matfile`. |
| MAT v5 Eigenvector data set objects | Supported | Schema-mapped; multi-signal + labelled targets. |
| MAT v7.3 (HDF5) | Supported | Via `hdf5-reader`; both matrix orientations. |
| prospectr `NIRsoil.RData` | Supported | RDX3/XZ workspace, mapped from the data.frame. |
| Indian Pines cube + `_gt.mat` | Experimental (local-only) | One record per pixel, generated band index. |
| Arbitrary MATLAB structs / RData objects | Planned | Generic heterogeneous structures not yet mapped. |

## Limitations & known gaps

- Unknown MATLAB structs/objects are not treated as generic numeric arrays:
  their spectra, labels, axis scales and targets live inside nested objects
  rather than top-level `X`/`wavelengths` arrays.
- R workspace support is intentionally schema-mapped — the `.RData` path accepts
  the prospectr `NIRsoil` fixture and validates its expected columns.
- The Indian Pines MATLAB path is schema-mapped and local-only (academic-use
  source without a clear redistribution license); CI skips it when
  `samples_local/` is absent. The cube emits one record per pixel with a
  generated band-index axis and a warning because it carries no wavelength
  calibration.
- Generic MAT/RData structures, MAT v7.3 cubes and heterogeneous metadata/targets
  remain to be broadened.

## Reference readers

`scipy.io` and the `hdf5-reader` crate (MAT), R serialization and `prospectr`
(RData). nirs4all-io adds axis detection, signal typing, target mapping and
provenance.

## Samples & validation

Fixtures under `samples/matlab/`: `synthetic_nirs_v5.mat` / `synthetic_nirs_v73.mat`
(50 records each, `absorbance`/`y`), `eigenvector_corn.mat` (80 records, three
spectral signals, four targets), `eigenvector_nir_shootout_2002.mat` (655
records, two instruments), `scpdata_dso.mat` (20 records, `cm-1` axis),
`scpdata_als2004dataset.MAT` (204 records, index axis) and
`prospectr_NIRsoil.RData` (825 records, `nm` axis). The local-only
`samples_local/hyperspectral_cubes/indian_pines_corrected.mat` (21,025 pixels x
200 bands, `land_cover_class` from the optional `_gt.mat`) is covered when
present. All committed fixtures are golden-backed in
`crates/nirs4all-io/tests/goldens/`.
