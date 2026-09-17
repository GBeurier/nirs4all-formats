# cran-comments.md

## Submission summary

* This is a **new submission**.
* `nirs4allformats` — a thin R binding for the Rust-first `nirs4all-formats`
  NIRS / spectroscopy file-loading engine. The package compiles a small
  `extendr-api` static library from `src/rust/` at install time and dispatches
  probe / read / walk calls directly through Rust. Without Cargo on the build
  machine it falls back to invoking the `nirs4all-formats` CLI binary.
* This is the **default / complete build**: every reader, including the optional
  large ones (HDF5/netCDF, Parquet/Arrow, MATLAB) on top of the core readers
  (JCAMP-DX, SPC, OPUS, ASD, ENVI, CSV, Excel, ...). A smaller sibling package
  `nirs4allformats.lite` drops only the Parquet/Arrow reader for size-sensitive
  installs. See *Complete vs lite* below.
* **Source tarball: 13.3 MB** (13,936,340 bytes). CRAN's guidance is that source
  tarballs should, if possible, not exceed 10 MB; this complete build (every
  reader) is over that soft cap and would need a CRAN size exception, so the
  R-universe channel and the GitHub Release are the primary distribution. The
  crates.io dependency closure is shipped compressed (`vendor.tar.xz`) and the
  test-only `[dev-dependencies]` are stripped from the vendored manifests; the
  residual weight is the Apache Arrow/Parquet, HDF5/netCDF and MATLAB (`rds2rust`
  → `getrandom 0.4` WASI) closures, which `cargo vendor` collects for every
  target. The smaller `nirs4allformats.lite` (Parquet dropped) is ~10.7 MB.
* `R CMD check --as-cran`: **0 ERRORs**, only the Rust-static-library `abort`
  WARNING (inherent to linking extendr 0.7.x + std; surfaces on CRAN's Debian
  builder) and environment-specific NOTEs (detailed below).
* License: project dual license (`CeCILL-2.1 OR AGPL-3.0-or-later`), with the
  bundled file declared in `DESCRIPTION`.
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

`./configure` also strips the test-only `[dev-dependencies]` from the vendored
crate manifests before `cargo vendor`, so the multi-MB WASI/`getrandom` closure
that `tempfile` drags in (and which the R package never compiles) is kept out of
the tarball.

## CRAN Policy compliance: no writes to the user's `HOME`, no `target/` shipped

The install **never touches `~/.cargo` or `~/.rustup`**, per CRAN Policy.
`src/Makevars` and `src/Makevars.win` set, before every `cargo` invocation:

* `CARGO_HOME=$(CURDIR)/.cargo` — a build-local cargo registry/cache inside the
  package build tree, so cargo's index/git-db never land in the user's
  `~/.cargo` (the canonical extendr / `rextendr` and `gifski` pattern);
* `CARGO_TARGET_DIR=$(CURDIR)/rust/target` — the build output stays inside the
  package build tree.

Both are wiped by a `rust_clean` Make rule that runs **after** the staticlib is
linked into the package `.so` (`all: $(SHLIB) rust_clean`): it removes the entire
`target/`, the build-local `CARGO_HOME`, and the extracted `vendor/`, so the
installed source tree carries **no `target/`, no `.cargo/`, and no extracted
`vendor/`** for `R CMD check` to scan. The committed build inputs
(`rust/vendored/`, `rust/vendor.tar.xz`, `rust/Cargo.toml`) are untouched. This
was verified by installing the built tarball under a **pristine fake `HOME`**
with `CARGO_HOME`/`RUSTUP_HOME` unset and `CARGO_NET_OFFLINE=true`: the fake
`HOME` stays empty (no `.cargo` created) and the build succeeds offline.

Per CRAN's "Using Rust" policy, `src/Makevars` and `src/Makevars.win` pass
`-j 2` to every `cargo build` invocation, bounding Cargo's parallelism rather
than letting it default to every logical CPU on the build machine.

`src/Makevars` and `src/Makevars.win` echo `rustc --version` and
`cargo --version` (to stderr) **before** the first `Compiling` line, so the
`R CMD check --as-cran` "Rust compilation" check finds the reported toolchain
version (otherwise it WARNs "No rustc version reported prior to compilation").

The `Cargo` / `rustc` toolchain is declared in `SystemRequirements`.

## Windows / UCRT toolchain

`src/Makevars.ucrt` (used by R >= 4.2 on Windows) sets `CARGO_LINKER` to Rtools'
mingw linker and includes `src/Makevars.win`, which wires it into
`CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER` and adds the canonical extendr
`libgcc_mock/libgcc_eh.a` shim (Rtools' GCC ships no `libgcc_eh.a`, which the
`x86_64-pc-windows-gnu` target's link step references). Without this shim the
Windows install fails at the final link.

Because `R CMD check` installs into the deep
`<pkg>.Rcheck/00_pkg_src/<pkg>/src` tree (already ~200 chars), `src/Makevars.win`
relocates the cargo target dir and the build-local `CARGO_HOME` onto the short
Rtools `/tmp` mount (`cygpath -m /tmp`, a native Windows path cargo accepts) so the
heavy C crates' object files (e.g. `lzma-sys`'s
`.../build/lzma-sys-<hash>/out/<hash>-lzma_encoder_optimum_fast.o`) do not overrun
Windows' 260-char `MAX_PATH`, which otherwise makes the `cc-rs` archive step fail.

## Test environments

* Local development (Ubuntu/WSL2, R 4.6.0 conda-forge, rustc 1.95): standalone
  offline install of the built source tarball from a directory with **no network
  access and no access to the monorepo `crates/`** -> installs and loads
  cleanly; `nirs4allformats_native_available()` returns `TRUE` (the native Rust
  path, not the CLI fallback).
* Submission-grade checks run on **current R (release + devel)** before upload
  via the GitHub Actions matrix (`.github/workflows/release-r.yml`):
  - Ubuntu 22.04 (R release + devel)
  - macOS 14 (R release, arm64)
  - Windows Server 2022 (R release)
* win-builder and R-hub v2 are run manually before each CRAN submission.

## R CMD check --as-cran status

`R CMD check --as-cran` (R 4.6.0 conda-forge) finishes with **no ERRORs**. The
findings are:

1. **Compiled code — `Found 'abort'`.** The static Rust library references
   `abort` through the **Rust standard-library runtime**, not through any package
   logic: Rust's panic handler and its allocation-error handler call `abort()`
   when a panic must terminate or an allocation fails. Every extendr / Rust CRAN
   package links this symbol; there is no clean way to remove it (it is part of
   `std`, and even a `panic = "abort"` profile keeps the allocation-failure
   `abort`). It surfaces as a WARNING on CRAN's Debian builder and is the known,
   justified WARNING CRAN tolerates for Rust packages. It does not indicate a
   defect in the package's own C or R code.
2. **CRAN incoming feasibility — "New submission".** Always present for a first
   upload.
3. **Compilation flag `-march=nocona`.** Comes from conda-forge R's own
   `Makeconf` (`CFLAGS`), not from the package. The package's `src/Makevars` sets
   only `PKG_CFLAGS = -I.../rust/src`; the Cargo release profile sets only
   `opt-level = 2` + `lto = "thin"`. This NOTE does **not** appear on a vanilla
   (non-conda) R build such as CRAN's.
4. **checking compiled code** — a conda-forge `nm` symbol-table parser failure on
   the static Rust library inside R's `checkFF`/`tools` code; a parser failure in
   the local conda toolchain, not a finding about the package, and absent on
   CRAN's GNU binutils.

The full reader set vendors the Apache Arrow/Parquet, HDF5/netCDF and MATLAB + xz
codec closures. Their bundled third-party build artefacts — `lzma-sys`'s and
`r-efi`'s GNU-extension Makefiles (`xz-5.2/dos/Makefile`, `xz-5.2/po/Makevars`,
the autotools `Makefile.am`/`.inc` inputs, `half`'s `Makefile.toml`,
`r-efi/Makefile`) and chrono's top-level `CITATION.cff` — would otherwise trip
the "GNU make extensions" WARNING and the "CITATION file in a non-standard place"
NOTE. None of these files is used by the build (every C-library crate compiles
through its own `build.rs`/`cc`), so `./configure` **deletes them from the
vendored crates before packing `vendor.tar.xz`** and removes their entries from
each crate's `.cargo-checksum.json` (cargo only verifies the files it lists, so
the offline build still checksum-verifies — confirmed by an offline install).
The generated cargo build-script scratch (zstd-sys' `out/flag_check.c`,
`out/zstd.h` → "line endings"/"pragmas" NOTEs) lives only under `target/`, which
the `rust_clean` rule wipes after linking; it never ships and is never scanned.
With these in place `R CMD check --as-cran` reports
**`GNU extensions in Makefiles ... OK`**, **`pragmas ... OK`**,
**`line endings in Makefiles ... OK`** and **`Rust compilation ... OK`**.

## Anti-patterns avoided

* No `-O3`, `-march=native`, `-Werror`, or other non-portable compiler flags.
  The Cargo release profile uses `opt-level = 2` + `lto = "thin"` only.
* No internet access during configure, install, examples, or tests
  (`cargo build --offline` against the bundled `vendor/`).
* No filesystem writes outside the build / `tempdir()` tree.
* No `:::` calls to private functions of other packages.
* Only `jsonlite` is imported; the package is a leaf in the CRAN dependency graph.
* `cargo build` parallelism is bounded with `-j 2` per CRAN's "Using Rust"
  policy.

## Complete (this package) vs lite

This is the **complete** build: the facade is compiled with its default
`formats-all` features, so it ships every reader — the core readers plus the
optional large ones (HDF5/netCDF, Parquet/Arrow, MATLAB). The smaller sibling
package **`nirs4allformats.lite`** (`bindings/r/nirs4allformatslite`) compiles
the facade with `default-features = false, features = ["fmt-hdf5", "fmt-matlab"]`,
which keeps HDF5/netCDF, MATLAB and every core reader and drops **only** the
Parquet/Arrow reader (the single biggest dependency). The two packages share the
same Rust core and the same exported R API.

Feeding the lite build a Parquet file returns a clean, actionable R error naming
`nirs4allformats` (this package) and the exact `install.packages(...)` line, not
a generic "unsupported format". This is implemented by a graceful-degradation
stub reader in the facade that recognises the Parquet `PAR1` magic and refuses it
with the install hint. A genuinely unknown file still returns the generic
"unsupported format" error, so the stub never over-claims.

## CRAN version note

CRAN rejects SemVer pre-release suffixes (`0.1.0-alpha.1`). The submitted R
version is therefore the plain `0.1.0`. A `.9000` development version is the
R-universe / development spelling only and is **not** submitted to CRAN.

## Reviewer-facing notes

* The vendored core crates under `src/rust/vendored/` are an exact textual copy
  of `crates/nirs4all-formats-core` and `crates/nirs4all-formats` from the
  project's GitHub repository at the tag matching this version. The sync is
  automated via `scripts/bump_version.sh` and verified by the
  `.github/workflows/version-sync.yml` workflow on every PR.
* Maintainer is Grégory Beurier (CIRAD, `gregory.beurier@cirad.fr`).

Thank you for the review!
