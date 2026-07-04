# Codex Gate 4 — final release-readiness (nirs4all-formats)

Consolidated into the ecosystem-level **Gate 5**. Per-repo Codex effort was concentrated on **Gate 3**
(`03_main_diff_review.md`); the SHA-pinner is comprehensive (56 pins, only `rust-toolchain@stable` left).

**Readiness snapshot:** `formats` is a Rust reader workspace (~58 format families) with Python/R/WASM/C
bindings and a tag-gated multi-registry release fleet (crates.io/PyPI/npm/CRAN). Push-hardening added the
community-health set (root `SECURITY.md` complementing the detailed `docs/SECURITY.md`), 56 SHA-pins, and a
CHANGELOG catch-up to `[0.2.1]`. **No parser/ABI/release changes.**

**Documented roadmap:** coverage floor for the reader core; `cargo-fuzz` on the parser surface (the primary
untrusted-input attack surface). Release is a multi-registry immutable fan-out — see `release_checklist.md`.
