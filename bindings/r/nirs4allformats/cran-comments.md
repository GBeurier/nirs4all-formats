# cran-comments.md

## Submission summary

* This is a **new submission** to CRAN.
* `nirs4allformats` — a thin R binding for the Rust-first `nirs4all-formats`
  NIRS / spectroscopy file-loading engine. The package compiles a small
  `extendr-api` static library from `src/rust/` at install time and dispatches
  probe / read / walk calls directly through Rust. Without Cargo on the build
  machine it falls back to invoking the `nirs4all-formats` CLI binary.
* License: `MIT + file LICENSE`.
* The same Rust core powers the project's Python (PyPI),
  JavaScript / WebAssembly (npm) and C-ABI bindings.

## Self-contained source tarball

The package vendors **everything it needs to build offline**, so CRAN's build
farm never touches the network and needs no out-of-tree sources:

* `src/rust/vendored/` — the two workspace core crates
  (`nirs4all-formats-core`, `nirs4all-formats`) copied verbatim from the
  project's monorepo. In the monorepo these are reached by relative `path =`
  dependencies; that is rewritten to in-tarball paths by `./configure`.
* `src/rust/vendor.tar.xz` — every crates.io transitive dependency, produced by
  `cargo vendor` and shipped **compressed**. `src/Makevars` /
  `src/Makevars.win` extract it and build offline against it, passing the
  crates.io source replacement INLINE to cargo
  (`cargo build --offline --config source.crates-io.replace-with="vendored-sources"
  --config source.vendored-sources.directory="vendor"`). The replacement is
  passed inline rather than via a `.cargo/config.toml` so the package ships no
  hidden `.cargo/` directory (which would trip an R CMD check NOTE). The deps
  are shipped as an archive rather than a raw directory because `R CMD build`'s
  tarball step strips VCS dotfiles (`.gitmodules`, ...) from inside vendored
  crates, which would otherwise break cargo's offline checksum verification —
  the same pattern the `arrow`, `gifski` and `polars` CRAN packages use for
  their Rust vendor trees.

The `Cargo` / `rustc` toolchain is declared in `SystemRequirements`.

## Test environments

* Local development (Ubuntu/WSL2, R 4.3.3, rustc 1.95): standalone offline
  install of the built source tarball from a directory with **no network
  access and no access to the monorepo `crates/`** → installs and loads
  cleanly; `nirs4allformats_native_available()` returns `TRUE` (the native
  Rust path, not the CLI fallback). `R CMD check --as-cran --no-manual`: the
  only NOTEs are environment / cargo-build artifacts described below.
* Submission-grade checks run on **current R (release + devel)** before upload
  via the GitHub Actions matrix (`.github/workflows/release-r.yml`):
  - Ubuntu 22.04 (R release + devel)
  - macOS 14 (R release, arm64)
  - Windows Server 2022 (R release)
* win-builder and R-hub v2 are run manually before each CRAN submission; their
  results are attached to the matching GitHub Release.

## Known notes (all expected)

Local `R CMD check --as-cran --no-manual` (R 4.3.3) finished with no ERRORs;
the WARNING and NOTEs are all from the bundled / vendored third-party Rust
sources or from the local toolchain, not from the package's own R or build
logic:

* **Installed package size** (NOTE: `installed size is ~15.7Mb`, `libs`) — the
  static Rust library links the full reader dependency closure (Apache Arrow /
  Parquet, HDF5 and NetCDF readers, compression codecs). This is inherent to a
  self-contained native binding.
* **GNU extensions in Makefiles** (WARNING) and **CITATION in a non-standard
  place** / **pragmas suppressing diagnostics** / **line endings** /
  **hidden files** (NOTEs) — these all point at files INSIDE the vendored
  crates.io sources extracted from `src/rust/vendor.tar.xz` at build time
  (e.g. `lzma-sys/.../Makefile`, `chrono/CITATION.cff`, `zstd-sys/.../zstd.h`).
  They are upstream third-party files the package compiles but does not author,
  exactly like every other CRAN package that vendors a Rust dependency tree.
  The package itself ships no hidden `.cargo/` directory — the crates.io source
  replacement is passed to cargo inline via `--config` instead.
* **Compilation flag `-march=nocona`** (NOTE) — comes from conda-forge R's own
  `Makeconf`, not from the package (the Cargo release profile sets only
  `opt-level = 2` + `lto = "thin"`; `src/Makevars` sets no `PKG_*FLAGS`).
* **CRAN incoming feasibility / future file timestamps** (NOTEs) — offline /
  NTP artifacts of the local check environment.
* **Rust / Cargo build** — the package builds a Rust static library at install
  time; the SystemRequirements `Cargo (Rust's package manager), rustc` declare
  the toolchain. The install is fully offline.
* **New submission** — first upload.

## Anti-patterns avoided

* No `-O3`, `-march=native`, `-Werror`, or other non-portable compiler flags.
  The Cargo release profile uses `opt-level = 2` + `lto = "thin"` only.
* No internet access during configure, install, examples, or tests
  (`cargo build --offline` against the bundled `vendor/`).
* No filesystem writes outside the build/`tempdir()` tree.
* No `:::` calls to private functions of other packages.
* Only `jsonlite` is imported; the package is a leaf in the CRAN dependency
  graph.

## CRAN version note

CRAN rejects SemVer pre-release suffixes (`0.1.0-alpha.1`). While the project
is pre-`0.1.0` the R spelling is therefore the development version
`0.1.0.9000`; the first CRAN-eligible R version is the plain `0.1.0` cut by
`scripts/bump_version.sh --bump 0.1.0`. A `.9000` development version is the
R-universe / development spelling only and is **not** submitted to CRAN.

## Reviewer-facing notes

* The vendored core crates under `src/rust/vendored/` are an exact textual copy
  of `crates/nirs4all-formats-core` and `crates/nirs4all-formats` from the
  project's GitHub repository at the tag matching this version. The sync is
  automated via `scripts/bump_version.sh` and verified by the
  `.github/workflows/version-sync.yml` workflow on every PR.
* Maintainer is Grégory Beurier (CIRAD, `gregory.beurier@cirad.fr`).

Thank you for the review!
