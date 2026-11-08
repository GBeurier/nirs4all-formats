# Formats contributor instructions

## Ownership and navigation

- Formats decodes vendor formats, including vendor containers such as HDF5; never `.n4a`. Parsers/sniffing live only in Rust; Python/R/WASM/C bindings call the registry and convert records. No modelling or GUI policy belongs here.
- [IO](../nirs4all-io/AGENTS.md) owns inspection, configuration and dataset assembly; [Core](../nirs4all-core/AGENTS.md) owns the directed `.n4a` byte container/inventory. Sibling links are optional workspace navigation.
- Core types/sniff contracts: `crates/nirs4all-formats-core/`; registry/readers: `crates/nirs4all-formats/`; CLI/C ABI: their sibling crates. Binding crates are excluded from workspace tests and need their own gates.
- Start with [scope/roadmap](docs/ROADMAP.md), [current worklist and full gate](docs/STATUS.md), [data model](docs/DATA_MODEL.md) and [conformance](docs/CONFORMANCE.md).
- Detailed commands/contracts are retained in [operations](AGENT_OPERATIONS.md).

- Formats has no dependency on the Python `nirs4all` library; it produces the records consumed downstream.

## Reader and fixture invariants

- Each signal owns its axis; preserve dimensions, units/order, masks/quality flags, metadata and provenance hashes/warnings.
- Implement bytes/no-filesystem paths when possible; sidecar-required formats must refuse the unsupported path explicitly.
- Reader acceptance covers sniffing, normalized output, metadata/provenance/warnings, truncation/corruption and reference comparisons, not a single parsed file.
- Keep goldens distinct from full-array reference conformance and respect named tolerances/skips.
- Register new readers and feature gates in both module/registry lists.
- Add licensed samples and per-format docs; update FORMAT_MATRIX/IMPLEMENTATION_DASHBOARD on support changes.
- Rebless goldens only after review with the documented opt-in.
- Never import/link GPL reference readers into the MIT runtime.
- Isolate them in conformance subprocesses.
- Commit redistributable fixtures in `samples/`; restricted/private fixtures stay in `samples_local/`; public demo samples must be GPL-free.

## Commands and conventions

Source Cargo's environment if needed: `. "$HOME/.cargo/env"`. Run the full current sequence in `docs/STATUS.md`, including feature/no-fs, binding and docs checks; update that status only with actual evidence.

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- Use Rust 2021/rustfmt; Python 3.10+, Ruff at configured 100 columns and strict mypy. Reader names are snake_case. Focused Rust tests/goldens belong under `crates/nirs4all-formats/tests/`; reference checks use `tests/conformance/`.
- Keep area-first focused commits (python/core/WASM/cubes/docs); PRs state behavior, checks, issue and fixture/license impact.
- Release/build variants and commands are in operations and [release documentation](docs/RELEASE.md), not a frozen release claim.

## Working rules

- Read applicable local and deeper instructions before editing. Preserve dirty/concurrent work; make authorized owner fixes and propagate affected contracts/bindings/consumers. Do not reset another author's work.
- Use CodeGraph FIRST for implementation navigation only if this checkout's root already has `.codegraph/`; use its exact projectPath or run the CLI here. Never auto-index or silently substitute an ancestor index.
- Unindexed code and documentation use ordinary source navigation. Explicit source, tests and build tools are final authority. Owner links are navigation; this policy works without a parent workspace.
- All application/project content is English: UI text/subtitles/tooltips/errors, docs/comments/docstrings, test/fixture prose, reports/issues/PRs, commits and generated material. Preserve identifiers, quotations and historical/provenance meaning; converse with the maintainer in French.
- Do not commit credentials, private data, generated workspaces, caches or build outputs. Follow local licensing/fixture restrictions and report actual commands/exits/skips; a command reference grants no execution/publication permission.
