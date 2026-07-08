---
orphan: true
---

# Repository audit — nirs4all-formats

> Generated from the automated pre-release audit (workflow wf_1fc87351-29f); the **Deepest hardening roadmap** section records the fullest realistic hardening even where the pragmatic pass does not implement it. Reviewed at Codex Gate 1.

- **Mode:** IN SCOPE — pragmatic hardening + push
- **Baseline HEAD:** `181946f`
- **Role:** Low-level Rust-first file readers for ~58 NIRS/spectroscopy format families, with thin Python (PyO3/maturin), R (extendr), WASM (wasm-pack), and C-ABI (cbindgen) bindings. Parsers live only in Rust; bindings translate SpectralRecords and never re-parse. Produces records consumed downstream by nirs4all/nirs4all-io.
- **Stack:** Rust 2021 (Cargo workspace: nirs4all-formats-core/-formats/-capi/-cli) as the source of truth; feature flags fmt-hdf5/fmt-matlab/fmt-parquet (default formats-all). Bindings excluded from workspace: Python >=3.10 via maturin/PyO3 (cibuildwheel cp310-cp313), R via extendr, WASM via wasm-pack, C ABI via cbindgen. Docs: Sphinx+MyST+furo on Read the Docs. Key deps: anyhow, clap, serde/serde_json, sha2, thiserror, pure-Rust hdf5-reader/netcdf-reader, arrow/parquet. gh CLI + Python packaging in version-guard.

## Release-readiness verdict
nirs4all-formats is a mature, release-ready Rust-first parser library with an unusually complete release apparatus: tag-triggered publishing to PyPI (OIDC trusted publishing), npm, and crates.io, plus SBOM + Sigstore provenance, version-guard/version-sync guardrails, and a two-tier golden+conformance test strategy. CI is currently all-green on main. The main hardening gaps are supply-chain/permissions hygiene rather than functionality: no top-level least-privilege permissions on ci.yml/release.yml, actions pinned by tag not SHA despite holding multi-registry publish rights, no coverage measurement, and a docs-CI strictness drift (RTD fail_on_warning vs CI without -W). Standard-file gaps (root SECURITY.md, CODE_OF_CONDUCT, CITATION.cff, .editorconfig, dependabot, PR template) and a stale CHANGELOG (stops at 0.1.0-alpha.1 while manifests are 0.2.1) are all low-risk quick wins. Push safety is the sharpest concern: a single production tag fans out to four registries including immutable crates.io, and pushes to main can auto-deploy the public Pages demo and trip the version guardrails.

## Gate commands (detected)
| key | value |
|---|---|
| `install` | python -m pip install -e tools/reverse-lab -e "bindings/python[numpy,pandas]" pytest   (Rust core needs no install: cargo builds on demand) |
| `test` | cargo test --workspace   (bindings: python -m pytest tools/reverse-lab/tests bindings/python/tests ; conformance: pytest -m conformance tests/conformance/) |
| `lint` | cargo clippy --workspace --all-targets -- -D warnings   (python binding configures ruff>=0.8 but it is not wired into CI) |
| `typecheck` | mypy is configured strict in bindings/python/pyproject.toml but no gate runs it; effective typecheck is the Rust compiler via cargo check/clippy |
| `format` | cargo fmt --all --check |
| `docs_build` | sphinx-build -W -b html docs docs/_build/html |
| `package_build` | python -m cibuildwheel --output-dir wheelhouse bindings/python   (sdist: maturin sdist --manifest-path bindings/python/Cargo.toml ; crates: cargo build --release ; wasm: wasm-pack build bindings/wasm --target nodejs --release) |

## CI
- **Latest status:** All green. gh run list --limit 8 shows version-sync, version-guard, CI all [ok] on the most recent pushes; no failing runs on main.
- **Workflows:**
- ci.yml (push main + rc/**, PR, dispatch: Rust fmt/clippy/test, Python helper+binding pytest, R smoke, Sphinx docs build WITHOUT -W)
- conformance.yml (weekly cron Mon 03:19 + dispatch: reference-reader pytest -m conformance)
- demo-pages.yml (push main paths demo/bindings/crates + dispatch: wasm-pack build -> GitHub Pages deploy)
- release.yml (tag v* + dispatch dry_run: cibuildwheel Linux+Windows, maturin sdist, C-ABI archives, PyPI OIDC trusted publish, GitHub release)
- release-crates.yml (tag v[0-9]* + dispatch: cargo publish leaf-first to crates.io)
- release-npm.yml (tag v[0-9]* + dispatch: wasm-pack nodejs -> npm @nirs4all/formats-wasm with provenance)
- release-r.yml (R source tarball -> CRAN-style, attach to Release)
- release-source.yml (tag v*: git-archive tarball+zip, CycloneDX SBOM via syft, SHA256SUMS, Sigstore keyless attest-build-provenance)
- version-guard.yml (push/PR main+rc: manifest must not be ahead of latest tag)
- version-sync.yml (push/PR main+rc: scripts/bump_version.sh --check across all binding manifests)
- **Gaps:**
- ci.yml docs job runs sphinx-build WITHOUT -W, but .readthedocs.yaml sets fail_on_warning: true -> RTD can fail on a warning that CI passes (drift between CI and RTD strictness)
- No coverage measurement or threshold in any workflow (no cargo-llvm-cov/tarpaulin/codecov); coverage.xml exists locally but is gitignored/untracked
- ci.yml and release.yml have NO top-level permissions block (default GITHUB_TOKEN scope); other workflows correctly set contents: read
- No conformance gating on release; conformance is weekly-only, so a release tag can ship without a fresh reference-reader comparison
- No mypy/ruff gate in CI though bindings/python/pyproject configures strict mypy + ruff
- GitHub Actions are tag-pinned (@v4, @stable, @v2, @v0), not SHA-pinned, despite OIDC publish rights to PyPI/npm/crates.io

## Standard files
- **Present:** readme, changelog, contributing, license, gitignore, issue_template
- **Missing:** security, code_of_conduct, citation, editorconfig, precommit, pr_template, dependabot

## Packaging
- **name:** `nirs4all-formats (Rust workspace crates: -core/-formats/-capi/-cli; Python wheel nirs4all-formats; npm @nirs4all/formats-wasm; R nirs4allformats/nirs4allformatslite)` — **version:** `0.2.1`
- **issues:**
- CHANGELOG.md is STALE: only documents 0.1.0-alpha.1 while every manifest is at 0.2.1 -> no release notes for 0.2.0/0.2.1 (Keep a Changelog format claimed but not maintained)
- macOS binary wheels intentionally deferred (release.yml matrix = ubuntu+windows only); macOS users must build sdist against their own HDF5 -> degraded install UX, documented but a packaging gap
- Version SoT is [workspace.package] in Cargo.toml, mirrored to 8 downstream manifests by scripts/bump_version.sh; any manual edit that skips the script drifts (version-sync.yml catches it in CI, but it is fragile)
- Python wheel depends on system/bundled HDF5 via fmt-hdf5/fmt-matlab default features; sdist build requires a Rust toolchain + HDF5 on the user machine
- Cargo.lock present but bindings Cargo.lock files are gitignored (generated on first build) -> binding builds not fully lock-pinned

## Tests
- **framework:** Rust cargo test (golden summaries + adversarial), pytest for Python bindings + reverse-lab, pytest -m conformance vs external reference readers, R smoke test, Node WASM smoke
- **estimate:** ~30 Rust source files containing #[test] across 14 test binaries + tests/{golden,adversarial,conformance}; 4 Python test_*.py files; 1 R smoke assertion; 1 Node smoke
- **coverage:** No coverage config or threshold anywhere. coverage.xml (3.9MB) is generated locally but gitignored; no cargo-llvm-cov/tarpaulin/codecov integration in CI.

## Docs
- **system:** Sphinx (docs/conf.py) with MyST + furo theme, sphinx-design/copybutton/opengraph; published to Read the Docs (.readthedocs.yaml, ubuntu-24.04, python 3.12, fail_on_warning: true, htmlzip). Rich docs/ tree incl. per-format pages under docs/formats/.
- **status:** Buildable. RTD config valid and requirements.txt present. Note: CI docs job builds WITHOUT -W while RTD enforces fail_on_warning, so RTD is stricter than CI and can fail on a warning CI lets through.

## Risks
| severity | area | detail |
|---|---|---|
| high | release/crates.io | release-crates.yml auto-publishes 4 crates to crates.io on any pushed v[0-9]*.[0-9]*.[0-9]* tag. crates.io is IMMUTABLE (yank-only, never replace). A mistaken tag permanently burns that version number across the ecosystem. Validate with the workflow_dispatch dry_run first, every time. |
| high | release fan-out | A single production tag push fans out to PyPI (OIDC trusted publish), npm @nirs4all/formats-wasm, crates.io, and two GitHub Releases (release.yml + release-source.yml both attach assets via softprops/action-gh-release@v2). Partial failure leaves registries in mixed state; idempotency is handled ad hoc (skip-existing on PyPI, npm-view pre-check, already-uploaded tolerance on crates). |
| medium | ci-permissions | ci.yml and release.yml lack a top-level permissions: block, so jobs inherit the repo default GITHUB_TOKEN scope (potentially read/write). release.yml relies on job-level grants (publish-pypi id-token: write, github-release contents: write) but the other jobs run with the broad default. |
| medium | supply-chain | All third-party actions are tag-pinned not SHA-pinned (actions/checkout@v4, dtolnay/rust-toolchain@stable, softprops/action-gh-release@v2, jetli/wasm-pack-action@v0.4.0, anchore/sbom-action@v0, pypa/gh-action-pypi-publish@release/v1). A compromised/moved tag on any of these runs inside jobs that hold PyPI/npm/crates publish rights. |
| medium | docs-ci-drift | RTD enforces fail_on_warning: true but ci.yml docs job omits -W; a warning-introducing docs change passes CI then breaks the RTD build post-merge. |
| low | security-policy-discoverability | SECURITY.md lives at docs/SECURITY.md; GitHub only surfaces the 'Security policy' link from a root, .github/, or docs/ SECURITY.md — docs/ is honored, but a root SECURITY.md is the conventional expectation and the current one says reporting is maintainer-direct pre-public. |
| low | changelog | CHANGELOG.md documents only 0.1.0-alpha.1 while the repo is at 0.2.1 — two minor releases undocumented, undermining the Keep-a-Changelog claim. |
| low | repo-weight | Large binary fixtures are tracked in git (samples_local.tar.gz.enc 47MB, samples/raman_witec/Sa4.wip 19MB, multiple 5-9MB cubes) inflating clone size; not Git-LFS managed. |

## Security
- **info** — No plausible real secrets found in tracked source (crates/bindings/src/tools/scripts). All token references are `${{ secrets.* }}` in workflows (CARGO_REGISTRY_TOKEN, NPM_TOKEN) or gitignored (samples_local.key).
- **low** — PyPI publish uses OIDC trusted publishing with an environment: pypi gate (good). But the pypi environment is not shown to have required reviewers/branch protection here; combined with tag-pinned (non-SHA) actions this is the main residual supply-chain exposure.
- **info** — samples_local.tar.gz.enc (46MB encrypted corpus) is committed; passphrase samples_local.key is gitignored. Encryption-at-rest is fine, but a committed encrypted blob means a future key leak retroactively exposes non-redistributable/license-restricted fixtures. Confirm the AES scheme in scripts/samples_local_crypt.sh is authenticated.
- **info** — docs/SECURITY.md correctly states the threat model (parsing untrusted binary files, fail-closed, bound reads/decompression, reject path traversal/symlinks) — appropriate for a parser library; ensure fuzzing/adversarial coverage backs these claims (tests/adversarial/ exists).

## Quick wins (pragmatic scope — safe to apply now)
- Add a root SECURITY.md (or symlink/point to docs/SECURITY.md) so GitHub surfaces the security policy; update the reporting section to GitHub private vulnerability reporting now that the repo is public.
- Add top-level 'permissions: contents: read' to ci.yml and release.yml (least-privilege default; job-level id-token/contents write grants already exist where needed).
- Add -W to the ci.yml docs sphinx-build so CI matches RTD's fail_on_warning: true and catches doc warnings pre-merge.
- Update CHANGELOG.md with 0.2.0 and 0.2.1 sections (it currently stops at 0.1.0-alpha.1 while manifests are 0.2.1).
- Add .editorconfig (Rust 4-space, Python/YAML settings) — none exists across this multi-language repo.
- Add .github/dependabot.yml for cargo, pip (bindings/python), npm (bindings/wasm), and github-actions ecosystems to keep deps + pinned actions current.
- Add a CODE_OF_CONDUCT.md and CITATION.cff (author/ORCID/DOI) — both absent; CITATION.cff matters for the CIRAD/academic audience.
- Add a .github/PULL_REQUEST_TEMPLATE.md referencing the green-gate sequence in docs/STATUS.md.
- Wire the already-configured ruff + strict mypy (bindings/python/pyproject.toml) into the CI Python job as a lint/typecheck step.
- Add a .pre-commit-config.yaml running cargo fmt --check, ruff, and the version-sync --check locally.

## Deepest hardening roadmap (fullest realistic hardening)
- SHA-pin every third-party GitHub Action (checkout, dtolnay/rust-toolchain, softprops/action-gh-release, jetli/wasm-pack-action, anchore/sbom-action, Swatinem/rust-cache, actions/*) with a Dependabot 'github-actions' updater to keep the SHAs fresh — critical because these run in jobs holding PyPI/npm/crates publish rights.
- Protect the release path: require the 'pypi' GitHub environment to have required reviewers and restrict deployment to tag refs; consider the same for a 'crates-io'/'npm' environment, so no single unreviewed tag push can fan out to 4 immutable/public registries.
- Introduce coverage: cargo-llvm-cov for the Rust workspace + pytest-cov for the Python binding, upload to Codecov/Coveralls, and set a ratcheting threshold (start at measured baseline, forbid regressions). coverage.xml is already generated locally — wire it up.
- Gate releases on a fresh conformance run: make the tag-triggered release depend on (or re-run) the reference-reader conformance suite rather than relying on the weekly cron, so no release ships without a current external-reader comparison.
- Add a fuzzing tier (cargo-fuzz / libFuzzer targets per reader family, or arbitrary-driven property tests) and run it on a schedule + on PRs touching readers/ — a format-parser library ingesting untrusted bytes is the canonical fuzz target; back the docs/SECURITY.md 'fail closed' claims with it.
- Expand the CI matrix: build/test on macOS + Windows (currently Rust/Python CI is ubuntu-only) and run the no-default-features and wasm32-unknown-unknown builds that CLAUDE.md's green gate requires but ci.yml does not mirror, so feature-flag gating stays honest in CI.
- Resolve the macOS wheel gap: static-link HDF5 from source (as noted in release.yml comments) so macOS gets binary wheels matching Linux/Windows, and add manylinux aarch64 to the wheel matrix.
- Add MSRV declaration (rust-version in [workspace.package]) + an MSRV CI job, and pin/audit the dependency tree with cargo-audit + cargo-deny (license + advisory gates) in CI — LICENSE_MATRIX.md/THIRD_PARTY_NOTICES.md exist but nothing enforces them.
- Reproducible-build verification: release-source.yml already does SBOM + Sigstore provenance; extend to verify byte-reproducibility of the source archive across two runners and publish the attestation verification instructions in docs/RELEASE.md.
- Migrate the large tracked binary fixtures (47MB enc corpus + multi-MB cubes) to Git LFS or an external fixture store to keep clone weight down; document fixture governance (docs/FIXTURE_GOVERNANCE.md exists — enforce it).
- Automate CHANGELOG maintenance (git-cliff or release-please for the Rust workspace) so version bumps and release notes cannot drift again.
- Add a CODEOWNERS file and branch protection requiring the CI + version-guard + version-sync checks before merge to main.

## Push-safety notes
- Tag push is a 4-registry publish trigger: pushing any v[0-9].[0-9].[0-9] tag simultaneously fires release.yml (PyPI OIDC), release-npm.yml (npm @nirs4all/formats-wasm), release-crates.yml (crates.io), release-source.yml + github-release. crates.io is IMMUTABLE — a bad tag permanently consumes that version. Always run the workflow_dispatch dry_run first (release.yml dry_run default 'true', release-crates.yml cargo publish --dry-run).
- release.yml:128/213 and the other release workflows gate publish on non-'-' tags, so pre-release tags (v0.1.0-alpha.1) are safe; but a plain vX.Y.Z tag has no manual approval step beyond the 'pypi' environment — verify that environment has required reviewers before relying on it as a gate.
- demo-pages.yml deploys the PUBLIC GitHub Pages site (formats.nirs4all.org) on every push to main touching demo/**, bindings/wasm/**, or crates/** — a routine core change auto-redeploys the public demo; only GPL-free samples may reach it (per CLAUDE.md).
- version-guard.yml (push/PR main+rc) FAILS the build if the in-repo version (Cargo [workspace.package] 0.2.1) is ahead of the latest git tag — so bumping the version on main before tagging will red the pipeline. The workflow expects: bump on a branch, tag = vX.Y.Z, never merge a bump ahead of its tag.
- version-sync.yml (push/PR main+rc) FAILS if any of the 8 downstream binding manifests drift from the Cargo workspace version — hand-editing any single manifest version without running scripts/bump_version.sh will break the push.
- Cross-repo coupling: the release workflows and OIDC/trusted-publisher config are explicitly modeled on nirs4all-methods (owner=GBeurier, Environment=pypi convention). Changing the workflow filename or environment name here breaks the PyPI Trusted Publisher binding and blocks publish.
- CI (ci.yml) and both version workflows run on push to both main and rc/** branches, so rc branch pushes exercise the full gate (intended per commit 32fc87f), but the release-* workflows only fire on tags — an rc branch push will not publish, only a tag will.
