# Generic HDF5 NIRS Datasets

> **Status:** Supported (scoped) · **Vendor:** Vendor-neutral · **Extensions:** `.h5`, `.hdf5` · **Feature flag:** `fmt-hdf5`

HDF5 is a general-purpose hierarchical container used by many NIRS pipelines,
spectrometers and research datasets to store spectra alongside a wavelength (or
wavenumber) axis, targets and free-form attributes. This is a schema-aware
reader for the common "spectra + axis" layouts; it is deliberately conservative
and refuses HDF5 files it cannot map confidently.

## Instruments & software

Vendor-neutral. The reader targets HDF5 files written by NIRS instruments,
conversion tools and analysis pipelines that follow the usual dataset naming
conventions. Committed fixtures are synthetic, exercising single- and
multi-signal layouts, nested groups, common dataset aliases and a transposed
matrix orientation.

## File structure

Detected by the HDF5 magic (`\x89HDF\r\n\x1a\n`) combined with an `.h5` / `.hdf5`
extension; the NIRS schema itself is validated only on read. Decoding uses the
pure-Rust `hdf5-reader` crate (gated behind the `fmt-hdf5` feature), so it works
on the no-filesystem `wasm32-unknown-unknown` target as well. The reader also
routes through the sidecar resolver, so external-file and external-link
references inside the container are followed via the same companion-file path.

The reader searches the root group first, then nested groups up to four levels
deep, looking for:

- a 2-D spectral dataset shaped `samples x bands`, or `bands x samples` when the
  axis length identifies the band dimension unambiguously;
- a matching 1-D axis dataset (`wavelengths`, `wavenumbers`, `wn`, `lambda`,
  `x_axis`, and related `*_nm` / `*_cm-1` aliases);
- optional 1-D numeric target datasets matching the sample dimension.

## What nirs4all-io extracts

- **Signals** — one signal per recognised spectral dataset. Multiple compatible
  datasets in the same group (e.g. `absorbance` and `reflectance` sharing one
  `/wavelengths`) are emitted as separate signals on each record; duplicate
  names get stable numeric suffixes. Dataset names such as `spectra`,
  `absorbance`, `reflectance`, `transmittance`, `intensity`, `raw`, `counts` or
  `data` are recognised, and the signal type is inferred from the name or a
  `units` attribute.
- **Axis** — values from the axis dataset, with the unit taken from its `units`
  attribute and the kind (Wavelength / Wavenumber / Index) inferred from the
  dataset name and unit.
- **Targets** — any 1-D numeric dataset that matches the sample count and is not
  the axis or a spectral dataset.
- **Metadata** — root and group attributes, the source group path, the matrix
  orientation when non-default, and per-signal unit hints.
- **Provenance** — source file + SHA-256, reader name and version.

The reader emits one `SpectralRecord` per sample row.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| `spectra` + `wavelengths`, single signal | Supported | Canonical `samples x bands` layout. |
| Multiple compatible datasets in one group | Supported | Emitted as separate signals sharing the axis. |
| Common dataset aliases (`absorbance`, `data`, `wn`, …) | Supported | Name- and unit-based signal/axis typing. |
| Transposed `bands x samples` matrix | Supported | Accepted only when the axis length is unambiguous. |
| Real metadata-rich vendor schemas | Planned | Synthetic fixtures only; real-world schemas still wanted. |

## Limitations & known gaps

- Dispatch is intentionally conservative: HDF5 files without a recognised 2-D
  spectral dataset and a matching 1-D axis are refused.
- Ambiguous transposes (where rows and columns could both match the axis) are
  rejected rather than guessed.
- FGI XML+HDF5 pairs are handled by the dedicated [`fgi-hdf5-xml`](fgi-hdf5-xml.md)
  reader; MATLAB v7.3 `.mat` files ([`matlab`](matlab.md)) and Allotrope ADF
  ([`allotrope-adf`](allotrope-adf.md)) use separate schema mappers even though
  their payloads are HDF5-backed.
- Real instrument schemas with rich metadata, complex axes, non-trivial targets
  and heterogeneous group conventions are still needed to harden the mapping.

## Reference readers

`h5py`, the `hdf5-reader` crate and PyTables (`tables`) open the same containers;
nirs4all-io adds axis detection, signal typing, target extraction and provenance
on top.

## Samples & validation

Fixtures live under `samples/hdf5/` (`synthetic_nirs.h5` — 50 records, two
signals on a shared `/wavelengths`; `generic_aliases_data_group.h5` — a
`bands x samples` `/data/absorbance` with a `cm-1` axis) and `samples/fgi/`. They
are covered by golden summaries in `crates/nirs4all-io/tests/goldens/`, and
non-spectral refusals are locked in. The probe reports `hdf5-nirs-container` at
`Confidence::Likely`.
