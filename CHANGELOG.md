# Changelog

All notable changes to **nirs4all-formats** are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project
uses an `0.1.0-alpha.*` line whose public surface is stable in shape but may
still change before 1.0.

## [0.2.8] - 2026-09-02

### Fixed

- Exclude the tracked local/customer corpus archive from public source
  archives and fail release qualification if local, encrypted, or key material
  is present in an archive, including case variants and Windows-style member
  separators.
- Fail reference-reader conformance when zero non-skipped cases execute,
  preventing an unavailable binding or reference environment from producing a
  false-green gate.
- Declare NumPy as the base Python runtime dependency used by the public record
  model, and make wheel smoke tests qualify the default install without extras.
- Align licensing and platform documentation with the release workflows
  actually in use; ship both project license texts in every Rust crate and C
  ABI archive.
- Make the crates.io dry-run validate unpublished workspace versions through
  local path patches while leaving the production registry chain unchanged.

### Changed

- Synchronize Rust, Python, R and WASM package versions at `0.2.8`.

## [0.2.4] - 2026-07-07

### Fixed
- Pin the Python `numpy` optional extra below NumPy 2.5 so cibuildwheel's
  manylinux2014 wheel tests install binary-compatible NumPy wheels instead of
  attempting to build a newer NumPy sdist with GCC 10.2.

## [0.2.1] - 2026-06-29

Consolidates the 0.1.0 → 0.2.1 releases (the CHANGELOG had lagged at `0.1.0-alpha.1`). The reader API
and `SpectralRecord` shape are stable; this line graduates the project past alpha.

### Added
- **OpenSpecy RDS reader** (new format family).
- **R bindings**: `nirs4allformats` LITE/CRAN build (core readers), `nirs4allformats.full` R-universe
  build, and a self-contained vendored build for off-tree CRAN / R-universe installs.
- Release machinery: `version-guard`, `version-sync`, and `release-source.yml` (source archive + SBOM +
  build provenance + checksums); a consolidated `release_process.md`.
- Privacy-respecting GoatCounter analytics beacon on the demo page.

### Fixed
- macOS wheel: pin the deployment target (11.0) and the cross-compile Rust target for delocate.
- Pin `wasm-pack` to a working version (jetli "latest" resolved to a broken build).
- R test fixtures skip off-tree builds cleanly.

## [0.1.0-alpha.1] - 2026-06-10

First published release (PyPI + GitHub release).

### Added

- **FOSS DS-series native `.nir` support** (`foss-ds-nir`). The newer FOSS DS2500
  and DS3 F benches emit the same binary container as the ISIscan/WinISI files
  but carry no `ISIscan`/`NIRSystems` identity string; the `foss_winisi` reader
  now recognises them by their `NIRS DS` instrument model at offset `0x82` and
  decodes them through the existing parser (spectra-only, version word `1`).
  Resolves the `.nir` extension collision with BUCHI NIRCal without a new reader.
- Synthetic CC0 fixtures `samples/foss_winisi/synthetic_ds2500.nir` (two-segment
  axis) and `synthetic_ds3f.nir` (single-segment axis), with golden summaries and
  `cargo test` coverage; a generator in `scripts/gen_synthetic.py`.
- New reader/probe tests for the DS-series path, including a local-only check
  against the real DS2500/DS3 F corpora.
- Encrypted, committed mirror of the local-only sample corpus
  (`samples_local.tar.gz.enc`) plus `scripts/samples_local_crypt.sh`
  (encrypt/decrypt/verify) and `docs/dev/SAMPLES_LOCAL_ARCHIVE.md`. The passphrase
  (`samples_local.key`) stays gitignored.

### Changed

- Integrated the full MicroNIR `.sam` corpus (120 spectra + vendor CSV) and the
  PerkinElmer MIR `.sp` corpus into `samples_local/`; updated the format matrix,
  dashboard, `FORMATS_STATUS.md` and the FOSS per-format page to reflect the
  DS-series support and to correct the now-decoded FOSS native row.
- Regenerated `demo/formats.json` from the matrix (150 validated variants).
