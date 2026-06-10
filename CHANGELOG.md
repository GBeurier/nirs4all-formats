# Changelog

All notable changes to **nirs4all-formats** are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project
uses an `0.1.0-alpha.*` line whose public surface is stable in shape but may
still change before 1.0.

## [Unreleased]

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
