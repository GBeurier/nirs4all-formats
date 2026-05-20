# Conformance Policy

Reader validation has two levels: strict normalized-output checks and external
reference comparisons.

## Strict Checks

These fields are compared exactly in golden tests:

- detected format and reader;
- axis unit, kind and order;
- signal names, roles, units and signal types;
- dimensions;
- metadata keys in the typed subset;
- provenance source hashes;
- warnings and quality flags.

Floating point arrays use explicit tolerances per format and per reference
reader. Defaults are strict and must be relaxed only with a documented reason.

## Reference Readers

When existing loaders are available, they are used to validate extracted data.
Examples include `asdreader`, `spectrolab`, `lightr`, `opusreader2`,
`brukeropus`, `spc-spectra`, `spectrochempy`, `spectral`, `jcamp` and
specialized vendor/community tools.

GPL readers are isolated through subprocesses and never imported into the MIT
runtime library path.

## No Reference Case

If no reference loader exists, the format can reach `Done` only after:

- fixtures cover controlled variations;
- reverse-engineering notes document each decoded field;
- adversarial tests cover truncation and corruption;
- at least one independent review note is committed.
