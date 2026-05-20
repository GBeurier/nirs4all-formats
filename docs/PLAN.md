# nirs4all-io Implementation Plan

> Status: decided. This document supersedes the earlier Python-first plan.
> Date: 2026-05-20.

## 0. Locked Decisions

| Topic | Decision |
|---|---|
| Core implementation | Rust is the reference implementation from day one. No Python-first or C++ hybrid core. |
| Public shape | Low-level Rust crates, additive C ABI surface, thin bindings. Same principle as `pls4all`, adapted to I/O. |
| First bindings | Python and R are planned from the start, but they bind the Rust core instead of owning parsing logic. |
| Python compatibility | Export numpy/pandas structures, sklearn-compatible dataset providers, and torch dataset adapters. |
| R compatibility | Expose native record loading plus idiomatic data.frame/tibble/matrix helpers; package builds locally first. |
| Later bindings | JavaScript/WASM, MATLAB/Octave, Android/JNI, Java, C#, Julia, Go, Rust, Ruby, Lua and other bindings are explicit short-term follow-ups after the first stable surfaces. |
| Formats | Add formats one by one. Start with open/documented formats or formats already reverse-engineered by existing libraries, then move toward harder proprietary formats. |
| Validation | Every reader has strict sniffing tests, golden records, fixture provenance, adversarial inputs, and conformance against reference loaders when they exist. |
| Reverse engineering | The repository contains a non-runtime lab for byte diffs, pattern scans, clean-room notes, and reproducible investigations. |
| License | MIT runtime. GPL reference readers may be invoked only through isolated conformance subprocesses. |

## 1. North Star

`nirs4all-io` provides a universal, low-level, high-performance reader for NIRS
and spectroscopy files. It normalizes vendor formats into a shared
`SpectralRecord` while preserving native axes, channels, metadata, targets,
warnings and provenance.

The library is useful standalone and is also the I/O foundation for the wider
`nirs4all` ecosystem. It does not implement chemometrics algorithms.

## 2. Architecture

```text
Rust core
  crates/nirs4all-io-core     data model, errors, units, provenance, sniffing
  crates/nirs4all-io          registry, readers, archive/sidecar policy
  crates/nirs4all-io-capi     stable additive C ABI for external bindings
  crates/nirs4all-io-cli      probe, inspect, convert, validate

Bindings
  bindings/python             native Python package, numpy/pandas/sklearn/torch helpers
  bindings/r                  R package, data.frame/matrix/tibble-compatible helpers
  future bindings             JS/WASM, MATLAB, Android/JNI, Java, C#, Julia, Go, ...

Validation and reverse engineering
  samples/                    source fixtures and provenance
  tests/golden                normalized expected records
  tests/conformance           reference-loader comparisons
  tests/adversarial           corrupted/truncated/ambiguous files
  tools/reverse-lab           byte-level exploration utilities
```

The bindings must stay thin. A binding can choose ergonomic return types, but
it must not fork format logic or maintain separate parsing rules.

## 3. Data Model

The core record shape is defined in `crates/nirs4all-io-core` and documented in
[`DATA_MODEL.md`](DATA_MODEL.md). Design constraints:

- each signal has its own spectral axis;
- axis unit, kind and order are preserved, not silently converted;
- multi-channel files expose named channels such as raw counts, dark reference,
  white reference, absorbance, reflectance, single beam or interferogram;
- lab values are `targets`, not metadata;
- metadata is exhaustive and JSON-serializable;
- provenance includes all source files, hashes, reader version and warnings;
- records carry quality flags instead of silently hiding incomplete support.

## 4. Reader Lifecycle

| Status | Meaning | Required gates |
|---|---|---|
| Experimental | Sniffer and partial parser exist. API can change. | fixture provenance, format note, smoke test |
| Beta | Works on representative real fixtures. | golden output, adversarial tests, documented limitations |
| Done | Reader is stable enough for downstream bindings. | conformance against reference reader or documented no-reference review, performance budget, no known sniffer false positives |

No reader reaches `Done` until its Python and R binding behavior is checked if
the public binding already exists.

## 5. Format Priority

Initial order:

1. Infrastructure-only skeleton: workspace, CI, docs, reverse lab, fixture
   governance.
2. Delimited text and simple exports: CSV/TSV/Ocean-style text, Avantes ASCII,
   Bruker DPT.
3. JCAMP-DX basic: AFFN and XYDATA, then DIF/DUP and NTUPLES.
4. Field spectrometers with strong references: Spectral Evolution `.sed`,
   SVC/GER `.sig`, ASD `.asd`.
5. Structured sidecar/container formats: ENVI SLI, HDF5/XML families.
6. Binary readers with existing reverse engineering: Galactic SPC, Bruker OPUS,
   Avantes 6/7/8, Nicolet OMNIC, BUCHI NIRCal.
7. Proprietary or sparse-reference formats: Foss native, Metrohm native,
   Perten native, FGI, Allotrope ADF and adjacent formats.

After three `Done` readers, implement the first native Python and R bindings.
After bindings are validated, continue adding formats with binding regression
tests at each step.

## 6. Conformance Strategy

For each format, store:

- fixture hash and license;
- expected sniff result;
- normalized golden JSON;
- reference loader name/version when available;
- field-level tolerance policy;
- known missing metadata or unsupported subblocks;
- performance benchmark for representative files.

Reference readers are used for validation, not as runtime dependencies. GPL
readers are isolated through subprocess boundaries.

## 7. Reverse-Engineering Workflow

Every proprietary format investigation gets a review note under
`docs/reviews/` or `docs/formats/`:

- source fixture set and legal redistribution status;
- byte-level observations;
- controlled diffs between related files;
- hypotheses and falsified hypotheses;
- parser decisions and ambiguity;
- comparison to existing libraries when available.

`tools/reverse-lab` provides basic byte-diff and pattern-scan tools. More
format-specific tools can be added there without becoming runtime code.

## 8. Release and Distribution

Short-term:

- GitHub source repository and GitHub Actions;
- Read the Docs build;
- local Python editable install;
- local R package build;
- no registry publication until readers and bindings have passed gates.

Medium-term:

- PyPI wheels using the Rust core;
- R package build pipeline and local binary checks;
- GitHub Release artifacts for C ABI headers/libraries;
- release matrices for Linux, macOS and Windows;
- later language packages once the C ABI and native Rust API are stable.

## 9. Current Checkpoint

The current checkpoint and next actionable prompt are kept in
[`STATUS.md`](STATUS.md). The phase roadmap is in [`ROADMAP.md`](ROADMAP.md).
