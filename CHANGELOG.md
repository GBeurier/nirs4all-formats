# Changelog

All notable changes to **nirs4all-formats** are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project
uses an `0.1.0-alpha.*` line whose public surface is stable in shape but may
still change before 1.0.

## [Unreleased]

### Added

- The export listing gives every writer a `maturity`: `standard` (published or open
  layouts: CSV, Excel, JCAMP-DX, Galactic SPC, JSON records), `experimental`
  (reverse-engineered FOSS WinISI and NSAS layouts) or `other` (two-column text, OPUS
  data point table). Additive field; existing entries are unchanged.
- Demo: the decoded-file workspace puts the conversion controls in a side panel next to
  the spectra, a top-level Export menu grouped by writer maturity that applies the
  conversions a target needs, an export of the spectra as shown, and one overlaid
  before/after chart with a second y-axis when the units differ.

## [0.3.1] - 2026-11-26

### Fixed

- Java: the Maven Central bundle no longer carries local-repository files
  (`maven-metadata-local.xml`, `_remote.repositories`), which the Central Portal refused;
  central-publishing-maven-plugin 0.12.0. 0.3.1 is the first release on Maven Central.
  No code change.

## [0.3.0] - 2026-11-26

First release of the 0.3 series: decoded values change (WinISI and ISIscan constituents of
0 read as missing; JSON readers parse numbers with correct rounding). It adds writers,
conversion plans, a C ABI and Java, C#, Julia and Docker distributions.

### Added

- Export records to JCAMP-DX 4.24, Galactic SPC, CSV, Excel, two-column text, OPUS data
  point tables and JSON, with presets for common spectroscopy and chemometrics software.
  Writers refuse or declare everything a target cannot carry and never resample or convert
  units. Available from Rust (`write_records`), the CLI (`convert`, `targets`), Python, R,
  WASM and the demo. Imports into vendor software are documented but not tested yet.
- Export spectra to FOSS WinISI `.nir` / `.cal` and NSAS `.da` / `.cn` files, with presets
  for WinISI and Metrohm Vision. The layouts are reverse-engineered and not yet tested in
  vendor software.
- Export requirements: `export_targets()` (CLI `targets`, Python, R, WASM) also lists, per
  format, the signal types, axis units, evenly spaced axis rule, segment and point limits,
  missing-value support and known grids (WinISI 700/1050 points, the four NSAS axes).
- Conversion plans: `plan_conversion(records, target, options)` (CLI `plan`, Python
  `plan_conversion`, WASM `planConversion`) returns the steps that make a signal writable
  for a target: reflectance from white/dark references, detector splice correction, signal
  type and percent/fraction conversion, point selection for overlapping detectors, axis
  unit conversion and resampling onto a named or even grid inside the measured range.
  Each numerical step names the nirs4all-methods method that computes it; Formats computes
  no values. Missing information becomes questions; raw counts without references,
  derivatives, preprocessed signals, interferograms, Raman shifts to nm and ratios of raw
  counts with different integration times are refused with a fix.
- Writers report conversions listed under the record metadata key `conversions` (output
  `conversions`), write them into JCAMP-DX (`##$CONVERSION n` labels) and SPC log text, and
  declare `conversions_not_written` for the other formats.
- Demo site: presents the library (install channels and a tabbed example per language) and
  is the tool itself. Every decoded file gets a conversion panel that plans a conversion for a
  software or format (or for viewing only), runs it with the vendored `@nirs4all/methods` WASM
  (exact version pinned in `demo/vendor/methods.version`, 1.5.2) loaded on demand, adds the
  result as a derived signal shown before, after or next to the original with a validation
  summary, and exports it with its losses and conversions. The format catalogue sorts by name,
  coverage and status.
- Read WinISI files of other instruments (model `Unknown`), recognized by a consistent header.
- Records with `null` values, as JSON represents missing points, deserialize as NaN.
- C ABI 0.2.0: library version, reader catalog, probe and open (path or bytes, with
  read options), export targets with requirements, conversion plans, in-memory write
  (base64 files, declared losses and recorded conversions) and path-to-path convert,
  all exchanging the record JSON. Calls return a status
  code and an error message; panics are caught. The 0.1.0 symbols are unchanged;
  the ABI version change makes the next release a minor, as already planned.
- Java (`org.nirs4all:nirs4all-formats`, JNA), C# (`Nirs4all.Formats`, .NET 8) and
  Julia (`NIRS4ALLFormats`) bindings over the C ABI: typed records, a signal matrix
  for identical axes, export targets and requirements, conversion plans (computed by
  nirs4all-methods, not by Formats), write and convert. A BinaryBuilder recipe for
  `nirs4all_formats_capi_jll` is ready for Yggdrasil.
- A non-root Docker image of the command-line tool for `linux/amd64` and `linux/arm64`.
- CI tests for the C ABI, Java, C#, Julia and the Docker image. Tag workflows publish
  to Maven Central, NuGet.org and GHCR when their credentials are configured, and
  TagBot tags Julia registrations; nothing is published without them.

### Changed

- **Decoded values change; the next release starts the 0.3 series.** WinISI `.cal` and
  ISIscan text exports store missing constituents as 0. They are now read as `null` with a
  `foss_winisi_zero_constituents_read_as_missing` warning instead of as reference values of
  0. Spectra and non-zero constituents are unchanged. A true 0 cannot be told apart.
- Python: `to_sklearn` and `to_spectrodataset` return missing numeric targets as NaN in a
  float array instead of `None` in an object array.
- JSON readers (Siware API, Allotrope ASM) parse numbers with correct rounding, so every
  build returns the value written in the file; some values change by one unit in the last
  place. Goldens are unchanged.
- Saving an export creates the output directory when it is missing (Rust, CLI, Python, R,
  C ABI, Java, C#).

### Fixed

- WinISI `.nir` / `.cal` reader: skip deleted samples instead of returning them and losing as
  many live ones, read the product code as the 16-bit field it is, and take the record size
  from the file length when the trailing index proves it. Constituents whose names differ
  only by case or punctuation no longer overwrite each other.

## [0.2.20] - 2026-11-24

Released as a patch by maintainer decision: spectra, axes, identifiers and record
order of files 0.2.19 already read are unchanged. The persisted
`foss_nsas_da_synthetic` golden changes only by added metadata keys and two reworded
warnings. NSAS DA files are now refused when a same-stem CN companion has a size that
does not match the DA records, or when the constituent word varies between records or
exceeds 32; 0.2.19 ignored both.

### Added

- Extend the experimental NSAS Rust reader to four observed DA geometries and optional
  CN reference targets, preserving raw names/bytes, both source hashes and missing-reference
  warnings. Add public CC0 fixtures, corruption tests and Python/R/WASM sidecar checks.
- Expose constituent count and spectral geometry metadata. Preserve the validated
  Beer Analyzer absorbance interpretation and label other profiles as unknown.

- Document the Rapitec/NSAS calibration workflow from the supplied manuals, with
  page citations and explicit separation of logical file roles from unverified
  binary layouts. Extend private DA qualification to three files, 191 spectra
  and exact reconstruction of all 200550 spectral values and opaque bytes.

- NSAS accepts one CN payload registered under several casings and refuses companions
  whose bytes differ. In-memory sidecar lookup stays exact, as in 0.2.19; the demo maps a
  case-differing companion to the declared name only when no exact spelling was supplied.

### Changed

- The R packages `nirs4allformats` and `nirs4allformatslite` declare the dual license
  `CeCILL-2.1 | AGPL (>= 3)` (SPDX `CECILL-2.1 OR AGPL-3.0-or-later`) instead of
  `file LICENSE` with the AGPL text only; both texts ship under `inst/`. Cargo, PyPI
  and npm metadata already declared the dual license.

## [0.2.19] - Unreleased

### Fixed

- The R packages install again on Windows with Rust 1.100 and later: `Makevars.win`
  links `-lsynchronization`, which the Rust standard library now requires from the
  final linker (`undefined reference to WaitOnAddress` / `WakeByAddressSingle`).
  The 0.2.17 and 0.2.18 releases carry no R source tarball for that reason.

### Changed

- CI runs the Windows `R CMD check --as-cran` for both R packages on every pull
  request and on `main`, with the same vendored, offline steps as the R release
  workflow, so a toolchain break is seen before a release tag.

## [0.2.18] - Unreleased

### Added

- Decode the observed legacy Rapitec / NIRSystems 6500 NSAS `.DA` layout: 1050-point
  absorbance records, acquisition metadata, transducer values and preserved opaque
  fields. Includes CC0 fixtures and exact full-array comparison with paired SPC
  exports; private originals remain local-only.
- Publish a macOS arm64 Python wheel (`macosx_11_0_arm64`, abi3 for CPython 3.10+)
  from the release workflow, so Apple Silicon installs no longer compile the sdist.
  The wheel bundles no external dynamic library: liblzma is compiled from the bundled
  xz sources (`LZMA_API_STATIC`). macOS x86_64 keeps installing from the sdist.

## [0.2.16] - Unreleased

### Security

- Update the pinned Calamine dependency to 0.38.0 for RUSTSEC-2026-0436.

## [0.2.15] - Unreleased

### Added

- Expose the native XLSX decoded-cell API and its typed limits/usage model for
  IO materialization. Reader ownership remains in Formats; the record schema
  and C ABI 0.1.0 are unchanged.

## [0.2.14] - 2026-11-02

### Fixed

- The delimited-text reader no longer drops a spectrum with missing or
  non-numeric spectral cells, which misaligned it with sibling target and
  metadata tables. Such cells are kept as `NaN` gaps and the record carries the
  `missing_spectral_values` quality flag. Rows without any numeric spectral
  value are still skipped, now with a provenance warning.

## [0.2.13] - 2026-10-29

### Security

- Replace the unsound HDF5 cache closure with the separately identified,
  MIT/Apache-licensed `nirs4all-hdf5-reader` and `nirs4all-netcdf-reader`
  `0.9.3-nirs4all.1` backports using `lru` 0.18.5. Upstream parser source bytes
  remain unchanged; original source hashes, VCS evidence and both licenses ship
  with Rust, Python, R and WASM distributions.
- Update ZIP, Arrow/Parquet and supported dependencies while keeping the public
  record model and real ZSTD codec support. Native packages require Rust 1.88.

### Fixed

- Propagate fallible ZIP member names without bypassing path validation.
- Retain the compatible PyO3 0.29.3 / pythonize 0.29 pair.
- Execute installed R tests and produce standalone R source packages with the
  licensed reader backports. Exclude editable Python caches from distributions.

### Changed

- Synchronize Rust, Python, R and WASM at `0.2.13`; publish the two backport crates
  before the Formats crates. Publication remains gated by the release workflows.

## [0.2.12] - 2026-10-28

### Added

- Native Raman Open Database `.rod` reader for Raman-shift/intensity loops,
  scalar metadata and multiline source fields, with scoped semantic coverage.

### Changed

- Synchronize the Rust, Python, R and WASM packages at `0.2.12`.

## [0.2.11] - 2026-10-05

### Added

- Native Unscrambler `.00D` Unicode non-design binary revision 35 reader,
  available in all builds: column-major matrices, leaf signal groups, labelled
  targets, sample identities, vendor metadata and missing-value handling.
- MATLAB v5 Unscrambler exports with `VarLabels0` / `ObjLabels`, selecting the
  numeric matrix by dimensions and preserving every variable without inferring
  absent group boundaries, physical units or calibration.
- CC0 synthetic paired fixtures, golden summaries, every-value SciPy conformance,
  local paired-corpus checks and a byte-layout reverse engineering report.
- Validation of native counts, offsets, revisions, selections and overlapping
  groups; Unicode labels, compressed/endian MAT variants and malformed inputs.

### Fixed

- Preserve UTF-16 surrogate pairs and empty label rows in MATLAB character
  matrices. Reject complex structured arrays instead of discarding imaginary data.

### Changed

- Synchronize this repository's Rust, Python, R and WASM packages at `0.2.11`.
  User-supplied fixtures remain local-only and are excluded from public artifacts.

## [0.2.10] - 2026-09-17

### Fixed

- Migrate XML readers to the UTF-8 event API in `quick-xml` 0.42, restoring
  compilation after the grouped dependency update without changing reader contracts.
- Use fixed-size slice chunks compatible with the current Clippy gate.
- Reconcile main with the published 0.2.9 history, retaining its security fixes,
  source-archive exclusions, licensing and release qualification checks.

### Changed

- Update grouped Rust dependencies and GitHub Actions, including the HDF5,
  NetCDF and RDS reader dependency migrations already integrated on main.
- Synchronize Rust, Python, R and WASM package versions at `0.2.10`.

## [0.2.9] - 2026-09-03

### Security

- Upgrade the direct XML parser to `quick-xml` 0.41.0 and the Excel reader to
  `calamine` 0.36.1, collapsing both XML dependency paths onto the patched
  `quick-xml` line and resolving `RUSTSEC-2026-0194` and
  `RUSTSEC-2026-0195` without changing the public parsing contracts.
- Refresh `crossbeam-epoch` to 0.9.20 to resolve `RUSTSEC-2026-0204`, found by
  the same release-candidate audit through the HDF5/NetCDF reader chain.

### Changed

- Synchronize Rust, Python, R and WASM package versions at `0.2.9`.

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
