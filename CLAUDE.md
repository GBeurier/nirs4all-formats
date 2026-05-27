# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> **This is `nirs4all-formats`, not `nirs4all`.** Parent-directory `CLAUDE.md` files describe
> the `nirs4all` Python modelling library and its webapp. This repo is a separate, **Rust-first
> low-level file-reader library** for NIRS/spectroscopy formats with thin Python/R/WASM/C bindings.
> It does **not** depend on `nirs4all`; it produces the records that `nirs4all` later models.

## What This Is

Rust-first low-level readers for ~58 NIRS and spectroscopy file format families, with stable
language bindings and conformance checks against reference loaders. The Rust core is the single
source of truth: bindings translate native `SpectralRecord`s into idiomatic numpy/pandas/sklearn/torch
(Python) and matrix/data.frame (R) shapes. Status and the live worklist are tracked in
[`docs/STATUS.md`](docs/STATUS.md) ("Last Green Gate" + "Next Agent Prompt"); the canonical plan is
[`docs/ROADMAP.md`](docs/ROADMAP.md).

## Core Principle (Critical)

**Parsers live only in Rust.** Bindings (`bindings/python`, `bindings/r`, `bindings/wasm`) and the
C ABI MUST NOT reimplement any format parsing — they decode through the registry and convert the
result. A new format = a new Rust reader. This is the architectural counterpart to the
nirs4all/webapp separation rule; treat it the same way.

Non-goals: no chemometrics/modelling, no GUI, no parser logic in bindings, and **no GPL reference
reader imported or linked into the MIT runtime core** (GPL readers are used only for conformance,
isolated behind subprocesses).

## Commands

Always source the Rust env first: `. "$HOME/.cargo/env"`.

```bash
# Core Rust workspace
cargo test --workspace                                    # all Rust tests
cargo test -p nirs4all-formats --test goldens                  # one integration test binary
cargo test -p nirs4all-formats <substring>                     # filter by test name
cargo fmt --all --check                                   # format gate
cargo clippy --workspace --all-targets -- -D warnings     # lint gate (warnings are errors)

# Feature-flag builds (readers gate on these)
cargo build -p nirs4all-formats --no-default-features                                   # core readers only
cargo build -p nirs4all-formats --no-default-features --target wasm32-unknown-unknown   # no-fs target

# CLI (binary name: nirs4all-formats)
cargo run -p nirs4all-formats-cli -- probe samples/jcamp_dx/TESTSPEC.DX
cargo run -p nirs4all-formats-cli -- read-json PATH [--rows 10:20 --cols 30:40 | --pixel R,C | --pixels-file f]
cargo run -p nirs4all-formats-cli -- read-json PATH --sidecar cube.hdr=PATH/cube.hdr   # sidecar formats
cargo run -p nirs4all-formats-cli -- scan DIR [--max-depth N --include-unsupported --json]

# Conformance vs external reference readers (pytest marker; weekly CI job)
pytest -m conformance tests/conformance/
NIRS4ALL_FORMATS_ACCEPT_GOLDENS=1 cargo test -p nirs4all-formats --test goldens   # re-bless goldens after a reviewed change
```

### Bindings

```bash
# Python (PyO3 + maturin; mixed python/ + src/ layout)
(cd bindings/python && maturin develop --release) && python -m pytest bindings/python/tests/
# or for the pure-Python compat helpers + reverse-lab:
pip install -e tools/reverse-lab -e "bindings/python[numpy,pandas]" && pytest tools/reverse-lab/tests bindings/python/tests

# R (extendr; native lib built at install time when Cargo is present, else CLI fallback)
R CMD INSTALL bindings/r/nirs4allformats

# WASM (wasm-pack; fmt-hdf5 ON, fmt-matlab/fmt-parquet OFF)
(cd bindings/wasm && wasm-pack build --target nodejs --release --out-dir pkg-node) && node bindings/wasm/tests/smoke.js

# Docs (Sphinx; -W treats warnings as errors)
sphinx-build -W -b html docs docs/_build/html
```

### The Green Gate (run before committing)

`docs/STATUS.md` → "Last Green Gate" holds the **authoritative** full pre-commit sequence (fmt,
test, clippy, no-default-features build, wasm build, per-binding clippy, Python/R/WASM tests, docs,
`git diff --check`). CI (`.github/workflows/ci.yml`) mirrors a subset (Rust fmt/clippy/test, Python,
R smoke, docs). Run the green gate locally and **update `docs/STATUS.md` after each green gate.**

## Architecture

Cargo workspace (`Cargo.toml`). The `bindings/*` crates are **excluded** from the workspace and
built independently.

| Crate | Role |
|---|---|
| `crates/nirs4all-formats-core` | Canonical data model, errors, signal types, `FormatProbe`/`Confidence` sniff contract, `SidecarResolver` trait. No vendor/binding deps. |
| `crates/nirs4all-formats` | Reader registry + 40+ native readers + directory walker. The public facade. |
| `crates/nirs4all-formats-capi` | Additive C ABI; `build.rs` regenerates `include/nirs4all_formats.h` via cbindgen (`cbindgen.toml`). |
| `crates/nirs4all-formats-cli` | `probe` / `read-json` / `scan` (also the current transport for bindings before native paths fill in). |

### Data model (`nirs4all-formats-core/src/model.rs`)

Every reader emits `Vec<SpectralRecord>`. Canonical shape — bindings mirror it, never replace it:

- `SpectralRecord { signals: BTreeMap<String, SpectralArray>, signal_type, targets, metadata, provenance, quality_flags }`
- `SpectralArray { axis, values, dims, signal_type, unit, role, source }` — **each signal owns its
  own `SpectralAxis`**; never assume channels share an x-axis. `dims` must contain exactly one `"x"`.
- `SpectralAxis { values, unit, kind (Wavelength/Wavenumber/Frequency/Energy/Time/Index), order }`;
  order is auto-detected (ascending/descending/non-monotonic).
- `SignalType` (`signal.rs`): Absorbance, Reflectance, Transmittance, Radiance, Irradiance, RawCounts,
  SingleBeam, Interferogram, KubelkaMunk, Derivative, Preprocessed, AerosolOpticalThickness, Uncertainty, Unknown.
- `Provenance` carries reader name/version, per-source SHA-256 (`SourceFile`), and `warnings`.
  Preserving provenance through to `nirs4all` is a project goal — see `docs/INTEGRATION_NIRS4ALL.md`.

### Reader registry & dispatch (`nirs4all-formats/src/registry.rs`)

Every reader implements the `Reader` trait: `name()`, `sniff(head, path) -> Option<FormatProbe>`,
`read_path()`. Optional overrides:
- `read_bytes()` — filesystem-free decode; **override it whenever the decoder can be**, both for
  perf and to work on `wasm32-unknown-unknown`. Default returns "does not support in-memory reads".
- `sniff_with_sidecars()` / `read_bytes_with_sidecars()` — for formats needing companion files.

Dispatch: `open_path`/`open_bytes` read up to the first 8192 bytes, call `sniff` on every registered
reader, sort candidates by `Confidence` (then format name) and pick the highest. `probe_path` returns
all positive candidates. `walk_path`/`scan` recurse a directory and mark each file parsed/error/unsupported.

Facade entry points (re-exported from `lib.rs`): `open_path`, `open_path_with_options`, `open_bytes`,
`open_with_sidecars[_and_options]`, `probe_path`, `walk_path`.

### Feature flags (`nirs4all-formats/Cargo.toml`)

- `default = formats-all = fmt-hdf5 + fmt-matlab + fmt-parquet`.
- `fmt-hdf5` — pure-Rust `hdf5-reader`/`netcdf-reader` (HDF5, NetCDF, Allotrope ADF, FGI XML+HDF5).
- `fmt-matlab` — **requires `fmt-hdf5`** (MATLAB v7.3 is an HDF5 container); also MAT v5 + prospectr RData.
- `fmt-parquet` — Arrow/Parquet tables.
- Readers behind a flag are gated with `#[cfg(feature = "...")]` in both `readers/mod.rs` and the
  `readers()` list. The no-default-features and wasm builds in the green gate exist to keep this honest.

### Sidecar formats

Formats needing companion files (ENVI `.sli/.img`+`.hdr`, FGI XML+HDF5, MATLAB Indian Pines `_gt.mat`,
ARM MFRSR NetCDF + QC YAML) decode via `open_with_sidecars` and a `SidecarResolver`
(`FsSidecars` / `InMemorySidecars` / `NoSidecars`). `open_bytes` refuses them explicitly with
`Error::UnsupportedSidecar` so bindings can route to the sidecar path. See `docs/dev/SIDECAR_RESOLVER.md`.

### Image cubes

`ReadOptions` selects pixels for cube readers (ENVI Standard, AVIRIS/ERDAS LAN): `CubeWindow`
(half-open rectangular ROI) or `CubeMask` (ordered sparse `(row, col)` list). CLI exposes these as
`--rows/--cols` vs `--pixel/--pixels-file` (mutually exclusive).

## Adding or Validating a Format

A reader is **not** accepted because it parses one file. Acceptance requires the sniffer, normalized
output, metadata, provenance, warnings, adversarial behavior (truncation/corruption) **and** a
reference-loader comparison all documented and tested (`docs/DIRECTIONS.md`, `docs/CONFORMANCE.md`).

To add a reader: create `crates/nirs4all-formats/src/readers/<fmt>.rs`, wire it into `readers/mod.rs`
(`pub mod` + `pub use`, with `#[cfg(feature=…)]` if HDF5/MATLAB/Parquet-backed), and register it in
the `readers()` Vec in `registry.rs`. Then follow the public trail (README "Adding Or Validating A
Format"): place the sample, document its source/license, add a `docs/formats/<fmt>.md` page, update
`docs/FORMAT_MATRIX.md` (+ `IMPLEMENTATION_DASHBOARD.md` on material status change), add tests, and
pass the green gate.

### Two-tier conformance

1. **Golden summaries** — `crates/nirs4all-formats/tests/goldens/*.summary.json`, checked by
   `cargo test --workspace`. Strict-compare format/reader, axis unit+kind+order, signal
   names/roles/units/types, dims, typed metadata subset, provenance hashes, warnings, quality flags.
   Arrays are *summarized* (length/first/last + 6-decimal rounded sum), not stored whole. Re-bless a
   reviewed change with `NIRS4ALL_FORMATS_ACCEPT_GOLDENS=1`.
2. **Reference readers** — `tests/conformance/` (`pytest -m conformance`, weekly +
   `workflow_dispatch` via `.github/workflows/conformance.yml`). Compares full arrays against
   `brukeropus` (OPUS), `spc-spectra` (SPC), `jcamp` (JCAMP), `spectrolab` (SED/SIG, R subprocess —
   GPL, isolated), canonical ASM JSON, `h5py` (HDF5). Per-format tolerances in `tolerances.toml`;
   structural reference-reader limits documented in `known_skips.toml`.

## Samples & Docs Map

- `samples/` — redistributable fixtures by format family (each with provenance/license notes);
  `samples_local/` — private/local-only fixtures (a local-only sweep runs outside CI).
- `tools/reverse-lab/` — Python clean-room reverse-engineering helpers (bitdiff CLI, etc.).
- Key docs: `STATUS.md` (live status + next steps), `ROADMAP.md`, `FORMATS.md` (scope),
  `FORMAT_MATRIX.md` / `IMPLEMENTATION_DASHBOARD.md` (per-variant status), `CONFORMANCE.md`,
  `DATA_MODEL.md`, `CLI.md`, `RELEASE.md`, `FORMAT_GAPS.md` / `MISSING_SAMPLES.md` (gaps & fixture needs),
  `dev/SIDECAR_RESOLVER.md`, per-format pages under `docs/formats/`.

## Release

`.github/workflows/release.yml` (tag-triggered, with `workflow_dispatch` dry-run) builds Python
wheels via `cibuildwheel` (manylinux2014 x86_64+aarch64, macOS x86_64+arm64, Windows AMD64; CPython
3.10–3.13), a maturin sdist, per-OS C ABI archives (with generated `nirs4all_formats.h`), and the R source
tarball; tagged releases publish to PyPI via OIDC trusted publishing. See `docs/RELEASE.md`.
