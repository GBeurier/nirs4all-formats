# Final hardening report — nirs4all-formats

**Date:** 2026-07-04 · **Branch:** `main` · **Operator:** Claude (Opus 4.8) · **Reviewer:** Codex CLI 0.142.5

## Summary
Pragmatic hardening of the Rust file-reader workspace (Python/R/WASM/C bindings): added the community-health
set, SHA-pinned every third-party action across all 10 workflows (only `rust-toolchain@stable` left, by
design), caught up the CHANGELOG (`0.1.0-alpha.1` → `0.2.1`), and added a `docs/maintenance/` trail.
**No parser, ABI, or release changes.** The `release-*` fleet is tag/dispatch-gated → this push does not publish.

## Baseline / commit
- **Baseline HEAD:** `181946f` (origin/main, CI-green).
- **Commit:** *(this commit)* — community-health + 56 SHA-pins + CHANGELOG + docs/maintenance.

## Files
Added: `CODE_OF_CONDUCT.md`, `CITATION.cff`, `SECURITY.md` (root; points to `docs/SECURITY.md`),
`.editorconfig`, `.pre-commit-config.yaml`, `.github/dependabot.yml` (github-actions + cargo ×5 incl. bindings + pip),
`docs/maintenance/{repository_audit,quality_gates,release_checklist,final_report}.md`,
`docs/maintenance/codex_reviews/{03,04}_*.md`.
Modified: all 10 `.github/workflows/*.yml` (56 SHA-pins), `CHANGELOG.md` (`[0.2.1]`).

## Checks
- YAML/CFF validated. Non-code change; Rust build/tests run in CI (authoritative). Baseline CI green at `181946f`.
- **Codex Gate 3** — pins valid; fixed dependabot binding-crate coverage; reconciled root vs docs SECURITY.
- **Codex Gate 4** — consolidated into ecosystem Gate 5.

## GitHub Actions (this push)
Branch-push gating runs (no publish): `ci` (cargo fmt/clippy/test + bindings + WASM), `conformance`,
`demo-pages` (Pages), `version-guard`, `version-sync`. Verified green post-push.

## Residual risks / roadmap
- Coverage floor + `cargo-fuzz` on the parser surface (untrusted-input security).
- `rust-toolchain@stable` intentionally unpinned. Multi-registry immutable release — see `release_checklist.md`.
- CECILL casing: pyproject binding uses `CeCILL-2.1` while Cargo/CITATION use canonical `CECILL-2.1` — reconcile at release.

## 12-month maintenance
- Merge weekly Dependabot PRs (actions + cargo×5 + pip) after CI-green.
- Keep `CHANGELOG.md` current; `version-sync` keeps cross-manifest versions aligned.
- Before release: dry-run each registry via dispatch, then tag the exact release commit.
