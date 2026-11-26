# Release Procedure

This file documents the Phase 6 release pipeline shipped in M4. The
authoritative workflow lives at
`.github/workflows/release.yml`.

## Pipeline overview

`release.yml` runs three parallel build jobs and two conditional publish
jobs:

| Job | Output | Triggers |
|---|---|---|
| `python-wheels` | Python wheels via `cibuildwheel` for Linux manylinux2014 x86_64, Windows AMD64 and macOS arm64 on CPython 3.10–3.13. | every push of a `v*` tag and every `workflow_dispatch`. |
| `python-sdist` | Source distribution built by `maturin sdist`. | same. |
| `c-abi-archive` | `nirs4all-formats-capi-<target>.{tar.gz,zip}` per OS containing `lib/libnirs4all_formats_capi.{a,so,dylib,dll}`, the generated `include/nirs4all_formats.h`, `LICENSING.md`, and both project license texts. | same. |
| `publish-pypi` | Publishes wheels + sdist to PyPI via OIDC trusted publishing (no token). | only on a `v*` tag. |
| `github-release` | Attaches the wheels, sdist and C ABI archives to the GitHub Release. | only on a `v*` tag. |

The other language channels have their own workflows, triggered by the same
`v<version>` tags. Each publishes only when its credentials are configured and
otherwise skips with a notice:

| Workflow | Channel | Credentials |
|---|---|---|
| `release-bindings.yml` | Java jar to Maven Central (Sonatype Central Portal), NuGet package to NuGet.org; both bundle the C ABI library for Linux x86-64/aarch64, macOS x86-64/arm64 and Windows x86-64. | `MAVEN_CENTRAL_USERNAME`, `MAVEN_CENTRAL_PASSWORD`, `MAVEN_GPG_PRIVATE_KEY`, `MAVEN_GPG_PASSPHRASE`; variable `NUGET_USER` (trusted publishing) or secret `NUGET_API_KEY`. See [Java](bindings/java.md) and [C#](bindings/csharp.md). |
| `release-docker.yml` | `ghcr.io/gbeurier/nirs4all-formats:<version>` and `:latest`, `linux/amd64` + `linux/arm64`. | `GITHUB_TOKEN` only. See [Docker](DOCKER.md). |
| `TagBot.yml` | Tags `NIRS4ALLFormats-v<version>` after a JuliaRegistrator registration; the JLL comes from Yggdrasil. | None. See [Julia](bindings/julia.md). |

`workflow_dispatch` runs the three build jobs but skips both publish jobs, so
it can be used as a dry-run before tagging. R source packages are built and
attached separately by `release-r.yml`.

## Tag-to-release flow

1. Bump the workspace version in `Cargo.toml` (and `bindings/python/pyproject.toml`).
2. Update `docs/STATUS.md` "Last Green Gate" with the release tag.
3. Verify the green gate locally (cargo fmt, test, clippy, sphinx, bindings).
4. Commit, then tag: `git tag vX.Y.Z && git push --tags`.
5. CI runs the build/publish fan-out. Python publishes to PyPI, Rust crates
   publish to crates.io, WASM publishes to npm, R source tarballs attach to the
   GitHub Release/R-universe flow, and the GitHub Release receives the built
   artifacts when the registry jobs succeed.

## Dry-run flow

```bash
gh workflow run release.yml --field dry_run=true
```

This builds every artifact but skips the publish steps. Download the
artifacts from the workflow-run UI to validate them locally:

- install a wheel into a fresh venv: `pip install <wheel>` then run
  `python -m pytest bindings/python/tests`.
- tar-extract the C ABI archive and compile the
  `crates/nirs4all-formats-capi/examples/probe_version.c` smoke against it
  (the example documents the exact compile flags).

For a local qualification bundle, generate the SBOM and release receipt before
the checksum manifest. `SHA256SUMS` must cover every deliverable and evidence
file, including `release-receipt.json` (but not the manifest itself). A
CycloneDX dependency graph advertised as complete must also contain a node for
every component; artifact-file leaves use an explicit empty `dependsOn` list.

## npm authentication and retry

`release-npm.yml` uses Node 24 and npm 11.19.1, enabling npm trusted
publishing. In the settings for `@nirs4all/formats-wasm` on npm, the GitHub
trusted publisher must name owner `GBeurier`, repository `nirs4all-formats`,
and workflow `release-npm.yml`, with permission to publish. Configuring this
trust is a package-owner action; upgrading the workflow alone does not grant it.
The `NPM_TOKEN` repository secret remains the fallback for token-based uploads.

A successful WASM build/smoke followed by HTTP 401 or E404 at upload can indicate
missing or expired publishing credentials, even though the public package exists.
After restoring credentials or the trusted-publisher configuration, retry the
workflow with `publish=true` on the reviewed release ref. Existing versions are
skipped; never move a published tag to change authentication configuration.

```bash
gh workflow run release-npm.yml --ref main --field publish=true
```

Verify that main still carries the intended release version before dispatching.

## Rollback / yank

PyPI wheels are immutable. Use
`pypi yank nirs4all-formats 0.x.y --reason "..."` to mark a bad release as
unavailable to new installs without breaking existing pins. For the
GitHub release, delete the assets or the entire release via `gh release
delete vX.Y.Z`.

## Per-wheel capability matrix

The pure-Rust HDF5 and NetCDF crates make the wheels portable, so every
wheel ships with the full feature set today:

| Platform | Default features | Notes |
|---|---|---|
| Linux manylinux2014 x86_64 | `fmt-hdf5`, `fmt-matlab`, `fmt-parquet` | Full coverage. |
| Windows AMD64 | full | Full coverage. |
| Linux aarch64 | source build | No wheel is produced by the current x86_64 Linux runner. |
| macOS arm64 | full | Full coverage; `macosx_11_0_arm64`, with liblzma compiled from the bundled xz sources (`LZMA_API_STATIC`). |
| macOS x86_64 | source build | Python installs from the sdist; the C ABI archive is still built for this architecture. |

If a future feature requires a C dependency that fails on a specific
platform, document the per-OS gating here and add an override under
`[tool.cibuildwheel.overrides]` in `bindings/python/pyproject.toml`.

## C ABI versioning

`N4FMT_ABI_VERSION` (currently `0.2.0`, defined in
`crates/nirs4all-formats-capi/src/lib.rs`) bumps independently from the Rust
semver:

- patch bump for fixes that keep every symbol and JSON shape;
- minor bump for added symbols, every existing one unchanged (0.1.0 to 0.2.0
  added the read, export and convert entry points);
- major bump for changed or removed symbols and incompatible re-shapes.

Any ABI version change makes the next crate release a minor (see
[Versioning](VERSIONING.md)).

Update `docs/VERSIONING.md` whenever the ABI moves.

### Rust 1.88 dependency resolution

The workspace and independently built Python, WASM and R bindings use Cargo resolver 3.
Fresh builds prefer dependency releases compatible with the declared Rust 1.88 minimum;
this avoids selecting `uuid >=1.27` (via `cfb`), which requires Rust 1.89.
The resolver does not suppress security audits or alter parsing/numerical behavior.
