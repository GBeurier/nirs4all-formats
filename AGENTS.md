# Repository Guidelines

## Project Structure & Module Organization

This is a Rust-first spectroscopy/NIRS reader library. Core data types and sniffing contracts are in `crates/nirs4all-io-core/`; the reader registry and format implementations are in `crates/nirs4all-io/`; the CLI is in `crates/nirs4all-io-cli/`; and the C ABI scaffold is in `crates/nirs4all-io-capi/`. Bindings live under `bindings/python/`, `bindings/r/nirs4allio/`, and `bindings/wasm/`. Rust integration tests and golden summaries are under `crates/nirs4all-io/tests/`; conformance tests are under `tests/conformance/`. Fixtures belong in `samples/` when redistributable and `samples_local/` when private.

## Build, Test, and Development Commands

Source Cargo first if needed: `. "$HOME/.cargo/env"`.

- `cargo fmt --all --check` checks Rust formatting.
- `cargo clippy --workspace --all-targets -- -D warnings` runs Rust linting with warnings as errors.
- `cargo test --workspace` runs the Rust test suite, including golden checks.
- `cargo run -p nirs4all-io-cli -- probe samples/jcamp_dx/TESTSPEC.DX` exercises CLI probing.
- `python -m pip install -e tools/reverse-lab -e "bindings/python[numpy,pandas]" pytest` installs Python helpers for local testing.
- `python -m pytest tools/reverse-lab/tests bindings/python/tests` runs Python tests.
- `R CMD INSTALL bindings/r/nirs4allio` builds and installs the R binding.
- `sphinx-build -W -b html docs docs/_build/html` builds docs with warnings as failures.

## Coding Style & Naming Conventions

Use Rust 2021 style and `rustfmt`; keep `clippy` clean. Python targets 3.10+, uses Ruff with a 100-character line length, and mypy strict mode. Name new Rust readers by format in `snake_case` under `crates/nirs4all-io/src/readers/` and register them through the existing registry. Parsers must live in Rust; bindings only call the core and convert records.

## Testing Guidelines

Add focused Rust tests in `crates/nirs4all-io/tests/`. Golden expectations live in `crates/nirs4all-io/tests/goldens/`; re-bless only after review with `NIRS4ALL_IO_ACCEPT_GOLDENS=1 cargo test -p nirs4all-io --test goldens`. Use `pytest -m conformance tests/conformance/` for reference-reader comparisons.

## Commit & Pull Request Guidelines

Recent commits use short scopes such as `python: ...`, `core: ...`, `WASM: ...`, `cubes: ...`, and `docs: ...`. Keep commits focused and mention the affected area first. Pull requests should describe behavior changes, list tests run, link issues, and note fixture provenance or license changes. Update `docs/FORMAT_MATRIX.md`, `docs/IMPLEMENTATION_DASHBOARD.md`, or per-format pages when reader support changes.

## Security & Configuration Tips

Do not import or link GPL reference readers into the MIT runtime path; use them only through isolated conformance subprocesses. Keep private or license-restricted fixtures in `samples_local/`, not in committed `samples/`.
