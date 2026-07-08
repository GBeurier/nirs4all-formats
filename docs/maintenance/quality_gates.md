---
orphan: true
---

# Quality gates — nirs4all-formats

A Rust workspace (the reader core) with Python / R / WASM / C bindings. Parsers live only in Rust.

## Local green gate

```bash
cargo fmt --all --check          # format
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace           # Rust reader tests

# Python bindings:
cd bindings/python && pip install -e ".[dev]" && ruff check . && pytest -q   # (maturin build)
```

Optional local hygiene hooks: `uvx pre-commit run --all-files`.

## CI gates (`.github/workflows/`)

| workflow | trigger | gate |
|---|---|---|
| `ci.yml` | push/PR | `cargo fmt`/`clippy`/`test`, Python bindings, WASM build |
| `conformance.yml` | push/PR | reader conformance suite |
| `demo-pages.yml` | push `main` | demo site → GitHub Pages |
| `version-guard.yml` / `version-sync.yml` | push/PR | manifest not ahead of tag; cross-manifest version sync |
| `release{,-crates,-npm,-r,-source}.yml` | **tag `v*` / dispatch** | build + publish (crates.io / PyPI / npm / CRAN) — **never on branch push** |

All third-party actions are **SHA-pinned** (56 pins across 10 workflows), Dependabot-tracked; the sole
exception is `dtolnay/rust-toolchain@stable`, left floating **by design**.

## Deepest-hardening roadmap

- Enforce a coverage floor for the reader core; add fuzzing (`cargo-fuzz`) for the parser surface
  (untrusted-input security — see `SECURITY.md`).
- Cross-manifest version single-source-of-truth (Rust workspace ↔ bindings) is already synced by
  `version-sync`; keep it green.
