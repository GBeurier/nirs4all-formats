# CI Matrix

## Phase 0

- Rust stable on Linux.
- `cargo fmt --check`.
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo test --workspace`.
- Python helper packages on Python 3.11.
- Sphinx/RTD documentation build.

## Phase 1+

- Linux, macOS and Windows for Rust tests.
- Python 3.10, 3.11, 3.12 and 3.13 for bindings.
- R package checks on Linux once native binding work starts.
- scheduled conformance jobs for reference readers.
- fuzz/adversarial jobs for binary readers.

## Release Gates

- no undocumented public ABI changes;
- fixture manifest and license matrix current;
- docs build from the release commit;
- Python and R examples validated for every `Done` reader.
