# License Matrix

The runtime code is MIT. Fixture and reference-reader licenses vary and are
tracked per format under `samples/`.

## Runtime Rule

The Rust crates, Python package, R package and reverse-lab code are MIT.

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
