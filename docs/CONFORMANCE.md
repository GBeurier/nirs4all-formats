# Conformance Policy

Reader validation has two levels: strict normalized-output checks and external
reference comparisons.

## Strict Checks

These fields are compared exactly in golden-summary tests:

- detected format and reader;
- axis unit, kind and order;
- signal names, roles, units and signal types;
- dimensions;
- metadata keys in the typed subset;
- provenance source hashes;
- warnings and quality flags.

Current goldens live under `crates/nirs4all-io/tests/goldens/` and are checked
by `cargo test --workspace`. They intentionally summarize arrays instead of
storing full arrays: axis length/first/last, value length/first/last and rounded
value sums. Full-array reference comparisons are added per format when a
trusted external reader is wired in.

To intentionally accept a changed normalized output:

```bash
NIRS4ALL_IO_ACCEPT_GOLDENS=1 cargo test -p nirs4all-io --test goldens
```

Only use acceptance after reviewing the reader change and updating the relevant
format notes.

Floating point summaries use six-decimal rounding. Full reference comparisons
use explicit tolerances per format and per reference reader. Defaults are strict
and must be relaxed only with a documented reason.

## Reference Readers (M2, 2026-05-23)

The conformance harness lives under `tests/conformance/`. Run it with:

```bash
pytest -m conformance tests/conformance/
```

Reference readers wired in M2:

| Format | Reference reader | Licence | How it's invoked |
|---|---|---|---|
| Bruker OPUS | `brukeropus` | MIT | Direct Python import. |
| Galactic SPC | `spc-spectra` | MIT | Direct Python import. |
| JCAMP-DX | `jcamp` (`jcamp_readfile`) | MIT | Direct Python import. |
| Spectral Evolution `.sed` | `spectrolab` | GPL-3 | `Rscript` subprocess (`tests/conformance/refreaders/sed_dump.R`). |
| SVC/GER `.sig` | `spectrolab` | GPL-3 | `Rscript` subprocess (`tests/conformance/refreaders/sig_dump.R`). |
| Allotrope ASM | canonical ASM JSON schema | n/a | `json.load` plus a walker over the standard `*… data cube` containers. ASM is itself the canonical encoding; no separate runtime reader exists. |
| Generic HDF5 | `h5py` | BSD | Direct Python import. |

GPL-licensed readers (`spectrolab`, `opusreader2`) stay isolated through
subprocess boundaries and are never imported into the MIT runtime library
path. Missing reference readers (R + `spectrolab` not installed, etc.)
make the matching tests skip rather than fail.

### Per-format tolerances

The comparison uses `abs(a − b) ≤ max(abs_tol, rel_tol × max(|a|, |b|))`.
Tolerances live in `tests/conformance/tolerances.toml`:

| Format | Axis abs | Axis rel | Values abs | Values rel | Rationale |
|---|---|---|---|---|---|
| OPUS | 0 | 1e-12 | 1e-7 | 1e-6 | Linear axis, CSF scaling matches `brukeropus`. |
| SPC | 1e-6 | 1e-7 | 1e-6 | 1e-6 | Explicit-X float32 round-trip. |
| JCAMP | 1e-1 | 5e-4 | 0 | 1e-9 | `jcamp_readfile` quantises Xi to textual precision; values stay tight via ASDF integers. |
| SED | 0 | 1e-6 | 0 | 1e-6 | ASCII text shared by both readers; harness auto-detects the `1.0`/`100.0`/`0.01` percent-vs-fractional convention per fixture. |
| SIG | 0 | 1e-6 | 0 | 1e-6 | Same as SED. |
| ASM | 0 | 1e-12 | 0 | 1e-12 | Bit-identical against the JSON arrays. |
| HDF5 | 0 | 0 | 0 | 0 | Bit-identical against `h5py`. |

### Known skips

`tests/conformance/known_skips.toml` documents fixtures whose reference
reader has structural limitations (e.g. `jcamp_readfile` concatenates
top-level multi-block JCAMP files into a single axis, breaking the
length-pairing logic). Each skip carries a one-line rationale.

### Current coverage (2026-05-23)

Initial M2 run on the committed corpus: **67 passed, 16 skipped, 0
failed** across 7 format harnesses. Skip reasons cluster around:

- non-spectral fixtures the reader refuses (vlen strings, peak
  assignments) — expected refusal path;
- empty-axis JCAMP fixtures (TESTFID, TESTSPEC) — `jcamp_readfile` does
  not handle those layouts;
- ambiguous block primacy in two OPUS ICR fixtures — covered separately
  by golden summaries.

## No Reference Case

If no reference loader exists, the format can reach `Done` only after:

- fixtures cover controlled variations;
- reverse-engineering notes document each decoded field;
- adversarial tests cover truncation and corruption;
- at least one independent review note is committed.
