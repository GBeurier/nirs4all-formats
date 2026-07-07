# Release checklist — nirs4all-formats

**Multi-registry, immutable fan-out.** A `vX.Y.Z` tag drives `release.yml` (+ `release-crates/npm/r/source`)
to publish to crates.io, PyPI, npm, and the R-universe/GitHub Release R flow.
Branch pushes to `main` never publish.

## Pre-release

- [ ] Green gate + CI green on the release commit (see `quality_gates.md`).
- [ ] `CHANGELOG.md` has a dated entry for the target version.
- [ ] Cross-manifest versions in sync (`version-sync` green); manifest not ahead of the tag (`version-guard`).
- [ ] **Dry-run each registry** via `workflow_dispatch` (crates/npm/R) and inspect artifacts + SBOM/provenance.
- [ ] Registry ownership confirmed: crates.io `nirs4all-formats-*`, PyPI, npm `@nirs4all/formats-wasm`, and R-universe package entries.

## Release

- [ ] Tag `vX.Y.Z` on the **exact release commit**; push it. Publishes are idempotent (skip already-published).
- [ ] Watch every `release-*` run to green; on a partial-registry failure, re-run the failed job (do NOT re-tag).

## Post-release

- [ ] `pip install nirs4all-formats==X.Y.Z` in a clean venv; smoke a reader import.
- [ ] Verify the version on crates.io / PyPI / npm / R-universe and the GitHub Release assets.
