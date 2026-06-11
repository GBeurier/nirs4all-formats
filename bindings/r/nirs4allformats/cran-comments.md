# cran-comments.md

## Submission summary

* This is a **new submission**.
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
  `cargo vendor` and shipped **compressed** (`xz`, as the CRAN "Using Rust"
  guidance recommends). `src/Makevars` / `src/Makevars.win` extract it and build
  offline against it, passing the crates.io source replacement INLINE to cargo
  (`cargo build --offline --config source.crates-io.replace-with="vendored-sources"
  --config source.vendored-sources.directory="vendor"`). The replacement is
  passed inline rather than via a `.cargo/config.toml` so the package ships no
  hidden `.cargo/` directory. The deps are shipped as an archive rather than a
  raw directory because `R CMD build`'s tarball step strips VCS dotfiles
  (`.gitmodules`, ...) from inside vendored crates, which would otherwise break
  cargo's offline checksum verification — the same pattern the `gifski` and
  `prqlr` CRAN packages use for their Rust vendor trees.

The `Cargo` / `rustc` toolchain is declared in `SystemRequirements`.

## Windows / UCRT toolchain

`src/Makevars.ucrt` (used by R >= 4.2 on Windows) sets `CARGO_LINKER` to Rtools'
mingw linker and includes `src/Makevars.win`, which wires it into
`CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER` and adds the canonical extendr
`libgcc_mock/libgcc_eh.a` shim (Rtools' GCC ships no `libgcc_eh.a`, which the
`x86_64-pc-windows-gnu` target's link step references). Without this shim the
Windows install fails at the final link — the cause of the earlier win-builder
"Installation failed" ERROR.

## Test environments

* Local development (Ubuntu/WSL2, R 4.6.0 conda-forge, rustc 1.95): standalone
  offline install of the built source tarball from a directory with **no network
  access and no access to the monorepo `crates/`** -> installs and loads
  cleanly; `nirs4allformats_native_available()` returns `TRUE` (the native Rust
  path, not the CLI fallback). `R CMD check --as-cran`: **0 ERRORs, 0 WARNINGs,
  3 NOTEs**, all environment / inherent-to-Rust artefacts described below.
* Submission-grade checks run on **current R (release + devel)** before upload
  via the GitHub Actions matrix (`.github/workflows/release-r.yml`):
  - Ubuntu 22.04 (R release + devel)
  - macOS 14 (R release, arm64)
  - Windows Server 2022 (R release)
* win-builder and R-hub v2 are run manually before each CRAN submission.

## R CMD check --as-cran status

`R CMD check --as-cran` (R 4.6.0) finishes with **no ERRORs and no WARNINGs**.
The remaining NOTEs are:

* **CRAN incoming feasibility — "New submission"** (and the tarball size). Always
  present for a first upload.
* **Compilation flag `-march=nocona`** — comes from conda-forge R's own
  `Makeconf` (`CFLAGS`), not from the package. The package's `src/Makevars` sets
  only `PKG_CFLAGS = -I.../rust/src`; the Cargo release profile sets only
  `opt-level = 2` + `lto = "thin"`. This NOTE does **not** appear on a vanilla
  (non-conda) R build such as CRAN's.
* **checking compiled code** — the static Rust library links the Rust standard
  library, which references `abort` (panic / allocation-failure paths). On CRAN's
  Debian builder this surfaces as the *"compiled code calls abort"* WARNING; it
  is **inherent to statically linking any Rust dependency tree** with
  `extendr-api` 0.7.x and cannot be removed without dropping the native backend.
  Locally (conda R) it shows up as a benign symbol-table parser NOTE.

The previous WARNINGs/NOTEs about GNU-make extensions (`lzma-sys`, `r-efi`), a
CITATION file in a non-standard place (`chrono/CITATION.cff`), pragmas and
non-LF line endings inside the vendored crates, and "No rustc version reported"
have all been **fixed** in this version:

* `./configure` strips stray `CITATION.cff` / `CITATION` files from the vendored
  tree (and removes their entries from each crate's `.cargo-checksum.json`, so
  the offline checksum verification still passes).
* `src/Makevars(.win)` prune the cargo build-script scratch (`$(LIBDIR)/build`,
  which holds the generated `zstd-sys` `flag_check.c` + `zstd.h`) and the
  extracted crates.io `vendor/` tree (the `lzma-sys` / `r-efi` GNU Makefiles and
  vendored zstd headers) immediately after linking, so `R CMD check` no longer
  scans third-party build artefacts the package compiles but does not author.
* `.Rbuildignore` excludes `src/rust/target/`, so the compiled build directory
  never ships in the source tarball.
* `src/Makevars(.win)` echo `rustc --version` to the install log before
  compiling, satisfying the "Rust compilation" check.
* `inst/WORDLIST` lists the package's domain vocabulary (NIRS, extendr, ...).

## Anti-patterns avoided

* No `-O3`, `-march=native`, `-Werror`, or other non-portable compiler flags.
  The Cargo release profile uses `opt-level = 2` + `lto = "thin"` only.
* No internet access during configure, install, examples, or tests
  (`cargo build --offline` against the bundled `vendor/`).
* No filesystem writes outside the build / `tempdir()` tree.
* No `:::` calls to private functions of other packages.
* Only `jsonlite` is imported; the package is a leaf in the CRAN dependency graph.

## CRAN feasibility — honest assessment

Two gates remain that are **not** fixable in package code:

1. **Source-tarball size.** The tarball is ~14 MB; almost all of it is
   `src/rust/vendor.tar.xz`, the compressed crates.io closure for the full
   reader set (Apache Arrow / Parquet, pure-Rust HDF5 / NetCDF, zstd / lzma
   codecs). CRAN's submission checker **auto-rejects tarballs over 5 MB**
   ("Please reduce to less than 5 MB for a CRAN package"); the documented relief
   is a manually-granted limit increase (uncertain for ~14 MB) or pinned
   download-at-install (which defeats the self-contained offline design CRAN's
   "Using Rust" page otherwise prefers). The size cannot be brought under 5 MB
   without dropping readers.
2. **The `abort` WARNING.** Inherent to a vendored Rust static library with
   `extendr-api` 0.7.x (see above).

Because of (1) — a hard auto-reject — **R-universe is the realistic distribution
channel** for this package (it builds binaries straight from Git and does not
gate on size or on the Rust `abort` WARNING). A CRAN submission would require
either a feature-trimmed variant under 5 MB or a maintainer size-exception
request, and even then must carry the inherent Rust `abort` WARNING. This file
is kept for the day a trimmed CRAN variant is attempted.

## CRAN version note

CRAN rejects SemVer pre-release suffixes (`0.1.0-alpha.1`). While the project is
pre-`0.1.0` the R spelling is therefore the development version `0.1.0.9000`; the
first CRAN-eligible R version is the plain `0.1.0`. A `.9000` development version
is the R-universe / development spelling only and is **not** submitted to CRAN.

## Reviewer-facing notes

* The vendored core crates under `src/rust/vendored/` are an exact textual copy
  of `crates/nirs4all-formats-core` and `crates/nirs4all-formats` from the
  project's GitHub repository at the tag matching this version. The sync is
  automated via `scripts/bump_version.sh` and verified by the
  `.github/workflows/version-sync.yml` workflow on every PR.
* Maintainer is Grégory Beurier (CIRAD, `gregory.beurier@cirad.fr`).

Thank you for the review!
