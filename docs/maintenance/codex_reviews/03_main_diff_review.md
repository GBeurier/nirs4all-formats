---
orphan: true
---

# Codex Gate 3 — main diff review (nirs4all-formats)

**Reviewer:** Codex CLI 0.142.5 — `codex exec review --uncommitted`, 2026-07-04 (background, long budget).

## Verdict
> "The workflow pinning itself appears syntactically valid." One dependency-coverage finding, fixed.

## Findings & disposition

| # | sev | finding | disposition |
|---|---|---|---|
| P2 | minor | dependabot `cargo` covered only the root workspace, missing the **binding crates excluded from it** (`bindings/python`=pyo3, `bindings/wasm`=wasm-bindgen, R `src/rust`=extendr-api) → those deps get no update/security PRs. | **Fixed** — added `cargo` updaters for `/bindings/wasm`, `/bindings/python`, and both R `src/rust` crates. |

## Also reconciled (self-noticed during review)
- formats already ships a detailed `docs/SECURITY.md` (parser runtime rules). The new **root** `SECURITY.md`
  (which GitHub surfaces) now points to it rather than diverging.

## Verified
- 56 action pins across 10 workflows; only `dtolnay/rust-toolchain@stable` left (floating by design).
- `release{,-crates,-npm,-r,-source}.yml` are tag/dispatch-gated — a branch push does not publish.
