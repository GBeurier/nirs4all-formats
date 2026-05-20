# License Matrix

The runtime code is MIT. Fixture and reference-reader licenses vary and are
tracked per format under `samples/`.

## Runtime Rule

The Rust crates, Python package, R package and reverse-lab code are MIT.
Runtime parser dependencies must remain permissive. Current container helpers
include pure-Rust crates such as `calamine`, `hdf5-reader`, `matfile` and
`cfb`; the added `cfb` OLE2 reader is MIT, with immediate transitive
dependencies under MIT / Apache-2.0-compatible terms. `flate2` is used for
zlib-compressed MAT v5 blocks and is MIT / Apache-2.0-compatible. R workspace
support uses `rds2rust` (MIT) plus `xz2` / `lzma-sys` (MIT / Apache-2.0) for
XZ-compressed `.RData` payloads.

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

Horiba / JobinYvon fixtures are split across GPL-3.0 RosettaSciIO XML samples
and MIT text/map samples from SpectroChemPy data and `ccoverstreet/horiba-raman`.
They are test fixtures only; no GPL reader code is linked or imported by the
runtime.

Renishaw WDF fixtures are split across GPL-3.0 RosettaSciIO samples and MIT
SpectroChemPy data samples. They are test fixtures only; reference readers stay
outside the runtime.

Princeton TriVista TVF fixtures come from GPL-3.0 RosettaSciIO test data. They
are committed as conformance fixtures only; `rsciio.trivista` is used for layout
comparison outside the runtime.

DigitalSurf SUR/PRO fixtures come from GPL-3.0 RosettaSciIO test data. They are
committed as conformance fixtures only; `rsciio.digitalsurf` is used for layout
and value comparison outside the runtime.

Hamamatsu IMG fixtures come from GPL-3.0 RosettaSciIO test data. They are
committed as conformance fixtures only; `rsciio.hamamatsu` is used for layout,
axis and value comparison outside the runtime.

mzML fixtures come from MIT-licensed `pymzML` test data. They are committed for
format detection and non-NIRS refusal tests.
