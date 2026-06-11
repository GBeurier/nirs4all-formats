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
| Cargo SemVer (verbatim) | `0.1.0-alpha.1` | `bindings/python/Cargo.toml`, `bindings/wasm/Cargo.toml`, `bindings/r/nirs4allformats/src/rust/Cargo.toml` |
| PEP 440 | `0.1.0a1` (`alpha.N→aN`, `beta.N→bN`, `rc.N→rcN`; plain `X.Y.Z`→itself) | `bindings/python/pyproject.toml`, `bindings/python/python/nirs4all_formats/_version.py` |
| R | `0.1.0.9000` (plain `X.Y.Z` for a final; `X.Y.Z.9000` "in development toward X.Y.Z" for ANY pre-release, since CRAN rejects SemVer pre-release suffixes) | `bindings/r/nirs4allformats/DESCRIPTION` |

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
| Environment | *(leave BLANK)* |

> The `publish-pypi` job intentionally declares no GitHub `environment:`, so the
> OIDC token carries no `environment` claim. The Trusted Publisher MUST be
> created with the **Environment field empty** — a publisher that specifies an
> environment will NOT match and the upload fails with `invalid-publisher`.
> Because the project does not exist on PyPI yet, create this as a **pending
> publisher** (same form, at the URL above).

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

### R → R-universe (registration) — follow-up

R-universe builds binaries (Windows/macOS/Linux) straight from Git — no review,
no submission. Users then
`install.packages("nirs4allformats", repos = "https://gbeurier.r-universe.dev")`.

- **Registry repo**: public `GBeurier/GBeurier.r-universe.dev` with a
  `packages.json` entry:
  ```json
  { "package": "nirs4allformats", "url": "https://github.com/GBeurier/nirs4all-formats", "subdir": "bindings/r/nirs4allformats" }
  ```
  No `branch` field → it tracks `main`.
- **GitHub App** (one manual browser step): install
  <https://github.com/apps/r-universe> on the `GBeurier` account.
- **Verify**: watch <https://gbeurier.r-universe.dev> (it *shows* the
  `R CMD check` result but, unlike CRAN, does not block on a NOTE/WARNING).

### R → CRAN (submission) — follow-up

CRAN is the canonical R repo; submission is a **manual web form** with human
review. Build the source tarball locally or take it from the `r-source` job's
artifact / the attached Release asset:

```bash
cd bindings/r && R CMD build nirs4allformats   # → nirs4allformats_<version>.tar.gz
```

Upload **only `nirs4allformats_<version>.tar.gz`** at
<https://cran.r-project.org/submit.html>:

| Field | Value |
|---|---|
| Your name | `Gregory Beurier` |
| Your email | `beurier@cirad.fr` (matches the `Maintainer:` in DESCRIPTION) |
| Upload | the package's `.tar.gz` |
| Optional comment | the release notes (see `cran-comments.md`, which is `.Rbuildignore`d) |

> **CRAN version note:** CRAN rejects SemVer pre-release suffixes
> (`0.1.0-alpha.1`). The R spelling is therefore `0.1.0.9000` while the project
> is pre-`0.1.0`; the first CRAN-eligible R version is the plain `0.1.0` cut by
> `scripts/bump_version.sh --bump 0.1.0`. **A `.9000` dev version is not
> submitted to CRAN** — it is the R-universe / development spelling only.

After uploading, CRAN emails a confirmation link — click it to complete.

---

## Rollback / yank

PyPI wheels are immutable. Use `pip` / the PyPI web UI to **yank** a bad release
(`nirs4all-formats X.Y.Z`) so it is unavailable to new installs without breaking
existing pins. For the GitHub Release, `gh release delete vX.Y.Z` (and re-run
`release-source.yml` for a corrected tag).
