# nirs4all-io Python Binding

This package is a thin Python surface over the Rust core. The first published
binding will expose:

- native records as Python dataclasses or extension types;
- `to_numpy()` and `to_pandas()` export helpers;
- sklearn-compatible dataset providers;
- optional torch dataset adapters.

The package is intentionally skeletal until three Rust readers have passed the
conformance gate described in `../../docs/ROADMAP.md`.
