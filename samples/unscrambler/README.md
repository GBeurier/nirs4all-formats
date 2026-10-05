# Unscrambler fixtures

`synthetic.00D` is generated from invented values by
`python3 scripts/gen_unscrambler_fixtures.py`, alongside
`samples/matlab/synthetic_unscrambler.mat`. License: **CC0-1.0**.
No private sample values, identities or vendor code are copied into it.

The fixture contains two samples, one labelled target, two two-point signal
groups, an aggregate group and one vendor missing sentinel. It is covered by
the Rust golden suite and an independent SciPy comparison of the paired export.

SHA-256: `30b50b6701899594017056faebdbd4fdd79709362076845ae734f9cf745bcd1c`.

Real user-supplied fixtures are local-only under `samples_local/unscrambler/`.
Their provenance and hashes belong in `samples_local/INDEX.md`.
