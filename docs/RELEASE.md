---
orphan: true
---

# Release Procedure

This file documents the Phase 6 release pipeline shipped in M4. The
authoritative workflow lives at
`.github/workflows/release.yml`.

## Pipeline overview

`release.yml` runs four parallel build jobs and two conditional publish
jobs:

| Job | Output | Triggers |
|---|---|---|
| `python-wheels` | Python wheels via `cibuildwheel` for Linux (manylinux2014, x86_64 + aarch64), macOS (x86_64 + arm64) and Windows (AMD64) on CPython 3.10–3.13. | every push of a `v*` tag and every `workflow_dispatch`. |
| `python-sdist` | Source distribution built by `maturin sdist`. | same. |
| `c-abi-archive` | `nirs4all-io-capi-<target>.{tar.gz,zip}` per OS containing `lib/libnirs4all_io_capi.{a,so,dylib,dll}`, the generated `include/nirs4all_io.h`, and `LICENSE`. | same. |
| `r-source` | `nirs4allio_<version>.tar.gz` source tarball built via `R CMD build`. | same. |
| `publish-pypi` | Publishes wheels + sdist to PyPI via OIDC trusted publishing (no token). | only on a `v*` tag. |
| `github-release` | Attaches every artifact (wheels, sdist, C ABI archives, R source) to the GitHub Release. | only on a `v*` tag. |

`workflow_dispatch` runs the four build jobs but skips both publish
jobs, so it can be used as a dry-run before tagging.

## Tag-to-release flow

1. Bump the workspace version in `Cargo.toml` (and `bindings/python/pyproject.toml`).
2. Update `docs/STATUS.md` "Last Green Gate" with the release tag.
3. Verify the green gate locally (cargo fmt, test, clippy, sphinx, bindings).
4. Commit, then tag: `git tag v0.1.0 && git push --tags`.
5. CI runs the four build jobs, then publishes to PyPI and the GitHub
   release if all wheels succeed.

## Dry-run flow

```bash
gh workflow run release.yml --field dry_run=true
```

This builds every artifact but skips the publish steps. Download the
artifacts from the workflow-run UI to validate them locally:

- install a wheel into a fresh venv: `pip install <wheel>` then run
  `python -m pytest bindings/python/tests`.
- tar-extract the C ABI archive and compile the
  `crates/nirs4all-io-capi/examples/probe_version.c` smoke against it
  (the example documents the exact compile flags).

## Rollback / yank

PyPI wheels are immutable. Use
`pypi yank nirs4all-io 0.x.y --reason "..."` to mark a bad release as
unavailable to new installs without breaking existing pins. For the
GitHub release, delete the assets or the entire release via `gh release
delete vX.Y.Z`.

## Per-wheel capability matrix

The pure-Rust HDF5 and NetCDF crates make the wheels portable, so every
wheel ships with the full feature set today:

| Platform | Default features | Notes |
|---|---|---|
| Linux manylinux2014 x86_64 | `fmt-hdf5`, `fmt-matlab`, `fmt-parquet` | Full coverage. |
| Linux manylinux2014 aarch64 | full | Full coverage. |
| macOS x86_64 | full | Full coverage. |
| macOS arm64 | full | Full coverage. |
| Windows AMD64 | full | Full coverage. |

If a future feature requires a C dependency that fails on a specific
platform, document the per-OS gating here and add an override under
`[tool.cibuildwheel.overrides]` in `bindings/python/pyproject.toml`.

## C ABI versioning

`N4IO_ABI_VERSION` (currently `0.1.0`, defined in
`crates/nirs4all-io-capi/src/lib.rs`) bumps independently from the Rust
semver:

- patch bump for additive symbols that keep the ABI backward-compatible;
- minor bump for ABI-affecting changes (struct layout, removed symbols);
- major bump only for incompatible re-shapes.

Update `docs/VERSIONING.md` whenever the ABI moves.
