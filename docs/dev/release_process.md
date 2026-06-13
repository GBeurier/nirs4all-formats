---
orphan: true
---

# Development — Release Process

How each binding of `nirs4all-formats` is versioned, gated, and published. The
Python wheels + sdist and the source/provenance bundle publish automatically;
the R (CRAN / R-universe) and JS/WASM (npm) legs are **wired by a follow-up**
and their manual web-form / scope steps are documented below.

The authoritative build workflow is [`.github/workflows/release.yml`](../../.github/workflows/release.yml);
the source/SBOM/provenance bundle is [`.github/workflows/release-source.yml`](../../.github/workflows/release-source.yml).

## Single source of truth

The canonical version is the **`[workspace.package] version` in the root
`Cargo.toml`** (Cargo SemVer, currently `0.1.0-alpha.1`).
`scripts/bump_version.sh` propagates it to every binding manifest, translating
the spelling each ecosystem requires:

| Spelling | Example (`0.1.0-alpha.1`) | Manifests |
|---|---|---|
| Cargo SemVer (verbatim) | `0.1.0-alpha.1` | `bindings/python/Cargo.toml`, `bindings/wasm/Cargo.toml`, `bindings/r/nirs4allformats/src/rust/Cargo.toml`, `bindings/r/nirs4allformatslite/src/rust/Cargo.toml` |
| PEP 440 | `0.1.0a1` (`alpha.N→aN`, `beta.N→bN`, `rc.N→rcN`; plain `X.Y.Z`→itself) | `bindings/python/pyproject.toml`, `bindings/python/python/nirs4all_formats/_version.py` |
| R | `0.1.0.9000` (plain `X.Y.Z` for a final; `X.Y.Z.9000` "in development toward X.Y.Z" for ANY pre-release, since CRAN rejects SemVer pre-release suffixes) | `bindings/r/nirs4allformats/DESCRIPTION`, `bindings/r/nirs4allformatslite/DESCRIPTION` |

```bash
scripts/bump_version.sh --check          # exit 1 on any drift (CI gate)
scripts/bump_version.sh --bump X.Y.Z     # rewrite the SoT, then sync
scripts/bump_version.sh                   # sync every manifest to the SoT
```

The C ABI version (`N4FMT_ABI_VERSION` in
`crates/nirs4all-formats-capi/src/lib.rs`, runtime `n4fmt_abi_version()`)
bumps **independently** from the Rust semver — see
[`docs/VERSIONING.md`](../VERSIONING.md).

## Binding → registry → automation

| Binding | Package | Registry | Automation | Trigger |
|---------|---------|----------|------------|---------|
| Python | `nirs4all-formats` | PyPI | **Automated** — `release.yml` (`python-wheels` cibuildwheel matrix + `python-sdist` maturin) publishes via Trusted Publishing | push tag `v*` (non-pre-release) → PyPI |
| R | `nirs4allformats` | CRAN / R-universe | **Build CI-automated** — `release.yml` (`r-source` job: `R CMD build`) attaches the tarball to the Release. **CRAN submission is the irreducible manual web form; R-universe is a registry entry.** Wired by the follow-up. | tag push attaches the tarball |
| JS / WASM | `@nirs4all/formats-wasm` | npm | **Manual today; automated `release-npm.yml` is the follow-up.** WASM module built with `wasm-pack`, published with `npm publish`. | — |
| Source + provenance | — | GitHub Release | **Automated** — `release-source.yml` (reproducible git-archive tar.gz + zip, CycloneDX SBOM, `SHA256SUMS`, keyless Sigstore provenance) | push tag `v*` (non-pre-release) |

## Exact release artifacts — what each binding ships, and where to upload it

Every artifact below is also attached to the **GitHub Release** for the tag, so
they are downloadable from one place.

| Binding | Registry | Exact file(s) | Upload |
|---|---|---|---|
| Python `nirs4all-formats` | PyPI | `nirs4all_formats-<version>-*.whl` (cibuildwheel: Linux x86_64/aarch64 manylinux2014, Windows AMD64, cp310–cp313) + `nirs4all_formats-<version>.tar.gz` (maturin sdist) | **Automated** — Trusted Publishing, *no manual upload* |

> **macOS binary wheels are deferred for the initial release.** The default
> `formats-all` feature set links the **system HDF5** (`fmt-hdf5`; `fmt-matlab`
> requires it), whose transitive homebrew `liblzma` is built for macOS 15 — so
> `delocate` rejects the wheel against an 11.0 deployment target. A portable
> macOS wheel needs a **from-source static HDF5 build** (tracked follow-up).
> Until then **macOS users `pip install nirs4all-formats` and get the sdist**,
> which compiles against their own HDF5; Linux + Windows wheels ship the full
> feature set, and the C-ABI macOS archives (`x86_64`/`aarch64-apple-darwin`)
> are published on the GitHub Release.
| R `nirs4allformats` | CRAN | **`nirs4allformats_<version>.tar.gz`** (source tarball) | **Manual** — web form (see *R → CRAN* below) |
| R `nirs4allformats` | R-universe | — (built from Git, no upload) | **Automated once registered** — registry repo + app (see *R → R-universe*) |
| JS / WASM `@nirs4all/formats-wasm` | npm | the staged `pkg/` package (via `npm publish`) | **Manual today** — `release-npm.yml` is the follow-up (needs `NPM_TOKEN`) |
| Source + provenance | GitHub Release | `nirs4all-formats-<version>-src.tar.gz` · `…-src.zip` · `nirs4all-formats-<version>.cdx.json` (SBOM) · `SHA256SUMS` | **Automated** — `release-source.yml` |

**For R/CRAN, upload the source `.tar.gz` only** — never a binary, the GitHub
repo zip, or the Python artifacts. The PyPI files publish from CI (no manual
upload); they are listed here only so the GitHub Release carries every artifact.

## Pre-release gates (release blockers)

Run these before tagging or publishing anything:

1. **Version sync** — `scripts/bump_version.sh --check`. The canonical version
   lives in the root `Cargo.toml` `[workspace.package] version`; the script
   syncs it into every binding manifest (the three Cargo manifests, the two
   PEP 440 Python files, and the R `DESCRIPTION`). **Bump with**
   `bump_version.sh --bump X.Y.Z[-pre]`. Enforced in CI by `version-sync.yml`.
2. **Green gate** — `cargo fmt --check`, `cargo clippy -D warnings`,
   `cargo test --workspace`, the conformance suite, and the Python/R/WASM
   binding smokes (see `docs/STATUS.md` "Last Green Gate" and `CONTRIBUTING.md`).
3. **C ABI sanity** — the generated `crates/nirs4all-formats-capi/include/nirs4all_formats.h`
   matches the current surface; bump `N4FMT_ABI_VERSION` only on an ABI change
   and update `docs/VERSIONING.md`.

## Tag-to-release flow

1. `scripts/bump_version.sh --bump X.Y.Z` (rewrites the SoT + syncs every
   manifest), then run `scripts/bump_version.sh --check` to confirm.
2. Update `docs/STATUS.md` "Last Green Gate" with the release tag.
3. Verify the green gate locally.
4. Commit, then tag: `git tag vX.Y.Z && git push --tags`.
5. CI builds wheels + sdist + C ABI archives + R tarball + source/SBOM bundle,
   then — **for a non-pre-release tag** — publishes to PyPI and cuts the GitHub
   Release.

**Pre-release tags** (anything containing `-`, e.g. `v0.1.0-alpha.1`) are
**excluded from publishing**: both the `publish-pypi` and `github-release` jobs
gate on `!contains(github.ref_name, '-')`, matching the nirs4all-methods
convention, so a pre-release never reaches PyPI or cuts a public Release. To
publish the current alpha to PyPI, tag it with the PEP 440 spelling
(`v0.1.0a1`) — the `publish-pypi` job validates that the tag minus `v` equals
the built wheel/sdist version (`X.Y.Z[aN|bN|rcN]`).

`workflow_dispatch` runs the build jobs only (dry run); both publish jobs are
also gated on `github.event_name != 'workflow_dispatch'`.

---

## Gated / maintainer one-time setup

### Python → PyPI (Trusted Publisher)

`release.yml`'s `publish-pypi` uses PyPI Trusted Publishing (OIDC,
`id-token: write`) — no API token. One-time maintainer setup at
<https://pypi.org/manage/account/publishing/>:

| Field | Value |
|---|---|
| PyPI Project Name | `nirs4all-formats` |
| Owner | `GBeurier` |
| Repository name | `nirs4all-formats` |
| Workflow filename | `release.yml` |
| Environment | **`pypi`** |

> The `publish-pypi` job runs in the GitHub `pypi` environment, so the OIDC
> token carries an `environment: pypi` claim — the Trusted Publisher MUST be
> created with **Environment = `pypi`** (same as nirs4all-methods'
> `release-wheels.yml`). A publisher whose Environment field differs (blank or
> anything else) fails with `invalid-publisher`. Because the project does not
> exist on PyPI yet, create this as a **pending publisher** (same form, at the
> URL above). **One convention across the whole ecosystem: Environment = `pypi`.**

### JS → npm (`@nirs4all/formats-wasm`) — follow-up

The npm leg (`release-npm.yml`) is **wired by the follow-up**. Manual build +
publish until then:

```bash
# 0. Gate: scripts/bump_version.sh --check (syncs bindings/wasm/Cargo.toml).
wasm-pack build bindings/wasm --release --scope nirs4all   # → bindings/wasm/pkg/
cd bindings/wasm/pkg
npm publish --access public      # scoped public package; needs npm login + 2FA
```

One-time: own the `@nirs4all` scope on [npmjs.com](https://www.npmjs.com)
(*Add Organization* → create the free org `nirs4all`), and — for the automated
`release-npm.yml` — generate a granular **Automation** token with read+write on
the `@nirs4all/formats-wasm` package and add it as the GitHub Actions secret
`NPM_TOKEN`.

### Complete (default) vs lite (no Parquet) — two packages, one core

The R binding ships as **two packages** built from the same Rust core and
exposing the **same `nirs4allformats_*` API**; only the compiled reader set and
the package name differ. CRAN's real guidance is that source tarballs should, if
possible, not exceed **10 MB** (a soft, exception-requestable cap — not a 5 MB
hard limit), so the complete package is the canonical default:

| Package | Dir | Readers | Channel | Tarball |
|---|---|---|---|---|
| **`nirs4allformats`** (complete, default) | `bindings/r/nirs4allformats` | **every** reader incl. HDF5/netCDF, Parquet/Arrow, MATLAB — facade default `formats-all` | **R-universe** + GitHub Release (CRAN needs a >10 MB size exception) | ~13.3 MB |
| **`nirs4allformats.lite`** (smaller) | `bindings/r/nirs4allformatslite` | full **minus Parquet/Arrow** — keeps HDF5/netCDF + MATLAB + every core reader; facade built `default-features = false, features = ["fmt-hdf5", "fmt-matlab"]` | **R-universe** + GitHub Release | ~10.7 MB |

Both packages' `./configure` strips the test-only `[dev-dependencies]` from the
vendored facade manifest (a harmless ~4 MB saving). The lite build additionally
drops the Parquet/Arrow reader (the single biggest dependency) so `cargo vendor`
never collects the Arrow/Parquet/zstd closure. The facade carries a feature-gated
**graceful-degradation stub** for the excluded Parquet reader: feeding the lite
build a Parquet file returns a clean R error naming the complete package
`nirs4allformats`, not the generic "unsupported format". The complete build keeps
the default features and so vendors the full closure.

### R → R-universe (registration) — follow-up

R-universe builds binaries (Windows/macOS/Linux) straight from Git — no review,
no submission. Users then
`install.packages("nirs4allformats", repos = "https://gbeurier.r-universe.dev")`
(or `"nirs4allformats.lite"` for the smaller no-Parquet variant).

- **Registry repo**: public `GBeurier/GBeurier.r-universe.dev` with one
  `packages.json` entry per package (both the complete and the lite):
  ```json
  { "package": "nirs4allformats",      "url": "https://github.com/GBeurier/nirs4all-formats", "subdir": "bindings/r/nirs4allformats" },
  { "package": "nirs4allformats.lite", "url": "https://github.com/GBeurier/nirs4all-formats", "subdir": "bindings/r/nirs4allformatslite" }
  ```
  No `branch` field → it tracks `main`.
- **GitHub App** (one manual browser step): install
  <https://github.com/apps/r-universe> on the `GBeurier` account.
- **Verify**: watch <https://gbeurier.r-universe.dev> (it *shows* the
  `R CMD check` result but, unlike CRAN, does not block on a NOTE/WARNING).
  The complete build's GNU-make-extensions WARNING (from the vendored
  lzma-sys/r-efi Makefiles) is expected and not gated by R-universe.

### R → CRAN (submission)

CRAN is the canonical R repo; submission is a **manual web form** with human
review. **CRAN submission uses the complete `nirs4allformats_<version>.tar.gz`**
(the `nirs4allformats.lite` tarball is the smaller R-universe / Release variant
and does not need to go to CRAN). Get the **self-contained** source tarball —
either:

- download **`nirs4allformats_<version>.tar.gz`** from the matching GitHub
  Release (built + `--as-cran`-checked by `release-r.yml`), **or**
- build it locally — the `./configure` vendor step is **required** so the
  tarball is self-contained (a plain `R CMD build` ships unresolved
  `../crates` path deps and is uninstallable off-tree):

  ```bash
  cd bindings/r/nirs4allformats && N4FMT_R_VENDOR=1 ./configure
  cd .. && R CMD build nirs4allformats        # → nirs4allformats_<version>.tar.gz
  ```

Upload **only `nirs4allformats_<version>.tar.gz`** at
<https://cran.r-project.org/submit.html>:

| Field | Value |
|---|---|
| Your name | `Gregory Beurier` |
| Your email | **`gregory.beurier@cirad.fr`** — must match the `Maintainer` (`cre`) in `DESCRIPTION` **exactly** |
| Upload | `nirs4allformats_<version>.tar.gz` (the R source tarball only — never a binary, the repo zip, or the Python sdist) |
| Optional comment to CRAN | **paste the block below** |

**Paste-ready CRAN submission comment** (kept in sync with
`bindings/r/nirs4allformats/cran-comments.md`):

```text
This is a new submission.

nirs4allformats is a thin R binding for the Rust-first nirs4all-formats NIRS /
spectroscopy file-loading engine. It compiles a small extendr-api static
library from src/rust/ at install time and dispatches probe / read / walk calls
through Rust; without Cargo it falls back to the nirs4all-formats CLI binary.
License: MIT + file LICENSE.

Self-contained source tarball: the package vendors everything it needs to build
offline. src/rust/vendored/ holds the two workspace core crates copied from the
project monorepo (relative path deps rewritten by ./configure), and
src/rust/vendor.tar.xz holds every crates.io transitive dependency produced by
`cargo vendor`, shipped compressed (xz) and extracted by src/Makevars(.win)
which build offline with the crates.io source replacement passed INLINE to cargo
(so the package ships no hidden .cargo/ directory). This mirrors the gifski and
prqlr CRAN packages. The Cargo / rustc toolchain is declared in
SystemRequirements; the install is fully offline. On Windows, src/Makevars.ucrt
points CARGO_LINKER at Rtools' mingw linker and src/Makevars.win adds the
canonical extendr libgcc_eh shim (Rtools' GCC ships no libgcc_eh.a, which the
x86_64-pc-windows-gnu link step references).

Test environments: local Ubuntu/WSL2 R 4.6.0 (offline standalone install from
outside the monorepo -> installs, loads, native path active); CI matrix
(release-r.yml) on Ubuntu 22.04 (R release + devel), macOS 14 (R release,
arm64), Windows Server 2022 (R release); win-builder + R-hub v2 run manually
before submission.

R CMD check --as-cran: 0 ERRORs, 0 WARNINGs, 3 NOTEs. The NOTEs are: "New
submission"; -march=nocona (from conda-forge R's Makeconf, not the package -
absent on vanilla R); and the compiled-code check (the static Rust library
references abort via std's panic / allocation-failure paths - inherent to
linking any Rust dependency tree with extendr 0.7.x; surfaces as a WARNING on
CRAN's Debian builder). The earlier GNU-make-extensions / CITATION / pragmas /
line-endings / "No rustc version" findings are all fixed (./configure strips
vendored CITATION files + patches the cargo checksums; src/Makevars(.win) prune
$(LIBDIR)/build + the extracted vendor/ tree after linking and echo `rustc
--version`; .Rbuildignore excludes src/rust/target/; inst/WORDLIST carries the
domain vocabulary). The package ships no hidden .cargo/ directory, sets no -O3 /
-march=native / -Werror, does no network access during install/examples/tests,
and imports only jsonlite.

Maintainer: Grégory Beurier (CIRAD), gregory.beurier@cirad.fr.
```

> **CRAN version note:** CRAN rejects SemVer pre-release suffixes
> (`0.1.0-alpha.1`). While the project is pre-`0.1.0` the R spelling is the
> development version `0.1.0.9000`, which is **R-universe / dev only and is NOT
> submitted to CRAN**. The first CRAN-eligible R version is the plain `0.1.0`
> cut by `scripts/bump_version.sh --bump 0.1.0`.

> **CRAN feasibility verdict — measured sizes are above the 10 MB guideline.**
> CRAN's real guidance is that source tarballs should, if possible, **not exceed
> 10 MB** (a soft, exception-requestable cap — there is no 5 MB hard auto-reject).
> The measured tarballs are: complete `nirs4allformats` **13.3 MB**
> (13,936,340 bytes); `nirs4allformats.lite` **10.7 MB** (11,197,025 bytes).
> Both are over the 10 MB soft cap, almost entirely `src/rust/vendor.tar.xz`.
> `./configure` strips the test-only `[dev-dependencies]` from the vendored
> manifest, but the dominant weight is **not** removable: the MATLAB/RData reader
> pulls `rds2rust → tempfile → getrandom 0.4 → the WASI wit-bindgen/wasmparser
> component-model toolchain` as a REAL runtime dependency (not a dev-dep), and
> `cargo vendor` collects the per-target closures (`windows-sys`, `linux-raw-sys`,
> `web-sys`, the WASI crates) for every platform regardless of the host build.
> Dropping Parquet (the lite build) saves ~2.7 MB; the remaining gap is the
> MATLAB + extendr R-glue closure. The only residual `R CMD check --as-cran`
> finding on either build is the inherent Rust-static-library `abort` WARNING
> (extendr 0.7.x + std) plus environment-specific NOTEs; both build offline, set
> no `-O3`/`-march=native`/`-Werror`, and import only jsonlite. **Ship both via
> R-universe** (`gbeurier.r-universe.dev` — builds binaries from Git, no size or
> WARNING gate) **and the GitHub Release**. A true <10 MB CRAN submission would
> need a download-at-install vendoring scheme or a per-target vendor prune; the
> complete `nirs4allformats` is the CRAN candidate (with a size exception).

After uploading, CRAN emails a confirmation link — click it to complete.

---

## Rollback / yank

PyPI wheels are immutable. Use `pip` / the PyPI web UI to **yank** a bad release
(`nirs4all-formats X.Y.Z`) so it is unavailable to new installs without breaking
existing pins. For the GitHub Release, `gh release delete vX.Y.Z` (and re-run
`release-source.yml` for a corrected tag).
