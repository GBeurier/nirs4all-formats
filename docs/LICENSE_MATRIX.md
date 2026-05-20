# License Matrix

The runtime code is MIT. Fixture and reference-reader licenses vary and are
tracked per format under `samples/`.

## Runtime Rule

The Rust crates, Python package, R package and reverse-lab code are MIT.
Runtime parser dependencies must remain permissive. Current container helpers
include pure-Rust crates such as `calamine`, `hdf5-reader`, `matfile` and
`cfb`; the added `cfb` OLE2 reader is MIT, with immediate transitive
dependencies under MIT / Apache-2.0-compatible terms.

## Reference Reader Rule

Reference readers may be:

- permissive and usable as development dependencies;
- GPL and usable only through isolated subprocess conformance jobs;
- unavailable or vendor-only, requiring clean-room reverse engineering.

No GPL package is imported by the runtime Python package or linked into the
Rust core.

## Fixture Rule

Every committed fixture needs source, license and hash documentation. Private
or non-redistributable fixtures must stay outside the public repository and be
referenced only by local manifest entries.
