# JCAMP-DX

Experimental native Rust reader for common JCAMP-DX `XYDATA`, `NTUPLES`,
sparse `PEAK TABLE` / `PEAK ASSIGNMENTS`, and Ocean Optics `LINK` records.

## Scope Implemented

- Sniffs labeled-data records containing `##JCAMP-DX=` or `##JCAMPDX=`.
- Reads `XYDATA=(X++(Y..Y))` data tables, including top-level multi-block
  files where each block becomes one `SpectralRecord`.
- Supports plain AFFN rows.
- Supports packed ASDF ordinate encodings:
  - PAC-style adjacent signed numbers;
  - SQZ pseudo-digits;
  - DIF difference-coded pseudo-digits;
  - DUP repeat counts.
- Reads NMR `NTUPLES` pages with `VAR_FORM=AFFN,ASDF,ASDF,AFFN` and emits
  real/imaginary channels as separate normalized signals on the same record.
- Reads `DATA TYPE=LINK` files whose child blocks use `XYPOINTS=(XY..XY)`;
  the committed Ocean Optics LINK fixture is mapped to `sample`,
  `dark_reference`, `white_reference` and a computed transmittance signal.
  Child blocks with incompatible axes are rejected instead of silently merging
  mismatched signals.
- Reads sparse `PEAK TABLE` and `PEAK ASSIGNMENTS` blocks as a single
  `peak_intensity` signal whose axis carries the listed peak positions.
- Applies `YFACTOR` to decoded ordinates and `XFACTOR` to peak abscissas /
  widths inside peak tables.
- Reconstructs the X axis from `FIRSTX` and `DELTAX`, or from `FIRSTX`,
  `LASTX` and `NPOINTS` when `DELTAX` is absent.
- Verifies `XYDATA` line-start X checkpoints against the reconstructed axis,
  accepting either physical checkpoints or checkpoints that require `XFACTOR`;
  mismatches are reported as provenance warnings.
- Uses `XUNITS`/`YUNITS` or NTUPLES `UNITS` to map axis kind/unit and signal
  type.

For `XYDATA` and `NTUPLES` blocks, `XFACTOR` is preserved in metadata but is
not applied to the reconstructed axis. In the committed Bruker fixtures,
`FIRSTX` and `DELTAX` are already in physical units while line-level X
checkpoints are stored in scaled integer form.

## Packed Data Notes

The ASDF decoder treats the first ordinate on each DIF line as a line
checkpoint. For all DIF lines after the first, that checkpoint is verified
against the previous decoded ordinate and then skipped so it is not counted
twice.

For DUP codes (`S`..`Z`, `s`), the encoded count is interpreted as the total
number of occurrences for the previous difference/value, so the reader emits
`count - 1` additional points.

Some legacy fixtures still decode a small number of trailing checkpoint values.
When `NPOINTS` is present and fewer points are declared than decoded, the reader
truncates to `NPOINTS` and records a provenance warning.

When `NPOINTS` declares more points than can be decoded, the reader now rejects
the file as malformed instead of emitting a shorter silent record.

## NTUPLES Notes

The NTUPLES path currently targets the high-value NMR real/imaginary page
layout used by the committed IUPAC fixtures:

- `SYMBOL=X,R,I,N`;
- `VAR_TYPE=INDEPENDENT,DEPENDENT,DEPENDENT,PAGE`;
- `DATA TABLE=(X++(R..R)), XYDATA` and `(X++(I..I)), XYDATA`.

`FIRST`/`LAST` are treated as physical axis endpoints. `FACTOR` is applied to
dependent ASDF ordinates and preserved in metadata for axis checkpoint work.
Time-domain axes with `UNITS=SECONDS` are represented as unit `s` with the
dedicated `time` axis kind.

## LINK / XYPOINTS Notes

`XYPOINTS=(XY..XY)` rows are parsed as explicit X/Y pairs rather than
reconstructing the axis from `FIRSTX` and `DELTAX`.

For Ocean Optics LINK exports, the three child blocks are raw sample, dark and
reference arrays even when the first child title says "processed". The reader
keeps those raw arrays and computes:

```text
processed = (sample - dark_reference) / (white_reference - dark_reference) * 100
```

When the denominator is zero, the current schema has no missing-value marker, so
the reader emits `0.0` for that point and records a provenance warning.

### LINK scope decision

The current v1 contract is: **one `##DATA TYPE=LINK` file emits one
`SpectralRecord`** whose signals must share a common axis. We deliberately
keep that scope tight rather than generalising LINK now:

- The model already permits per-signal axes, but downstream consumers treat
  axis-mismatched signals on the same record as ambiguous. Silently merging
  heterogeneous LINK children would push that ambiguity onto every caller.
- LINK files in practice come in disjoint varieties (Ocean Optics
  SpectraSuite, Bruker MULTIPLE SPECTRA, NMR multipulse, etc.) — each is
  best handled with explicit per-vendor logic rather than a permissive
  merge. We accept the committed Ocean Optics fixture and reject the
  rest with a clear error.
- Promoting `PEAK TABLE` children inside LINK would mix dense spectra with
  sparse peak lists on a single record, which has no consistent semantics
  in the current `SpectralRecord` model. LINK files whose children carry
  `##PEAK TABLE=` (or `##PEAK ASSIGNMENTS=`) are therefore rejected with a
  pointer to use a standalone peak-table file instead.

Top-level multi-block JCAMP files (multiple `##JCAMP-DX=` headers without
`##DATA TYPE=LINK`) are unaffected: each block is emitted as its own
record, including peak-table blocks.

## Peak Tables and Peak Assignments

The reader accepts JCAMP-DX 5.0 sparse peak descriptions, exposed as one
`peak_intensity` signal whose axis carries the listed peak abscissas (in their
native order — peaks are not re-sorted, so the resulting `SpectralAxis.order`
may be `Descending`, `Ascending`, or `NonMonotonic`).

Supported shape headers (parsed via the per-character field list, with `..`
denoting "many peaks per line"):

| Header                            | Per-peak fields                       | Lines           |
|-----------------------------------|---------------------------------------|-----------------|
| `##PEAK TABLE=(XY..XY)`           | `x`, `y`                              | packed          |
| `##PEAK TABLE=(XYW..XYW)`         | `x`, `y`, `width`                     | packed          |
| `##PEAK TABLE=(XYM..XYM)`         | `x`, `y`, `multiplicity`              | packed          |
| `##PEAK ASSIGNMENTS=(XYA)`        | `x`, `y`, `assignment`                | one-per-line    |
| `##PEAK ASSIGNMENTS=(XYWA)`       | `x`, `y`, `width`, `assignment`       | one-per-line    |
| `##PEAK ASSIGNMENTS=(XYMA)`       | `x`, `y`, `multiplicity`, `assignment`| one-per-line    |
| `##DATA TABLE=(XY..XY), PEAK ...` | as above, kind inferred from value    | as above        |

Rules:

- Assignment text is extracted as the first `<…>` substring on a line; the
  match is non-greedy on the closing `>`. Empty `<>` produces no assignment.
- Any shape containing the `A` (assignment) field is treated as one peak per
  line regardless of `..` syntax, because assignment payloads can contain
  whitespace and punctuation.
- `XFACTOR` is applied to `x` and `width`; `YFACTOR` is applied to `y`.
  `multiplicity` is recorded verbatim.
- If `##NPOINTS=` is present it must equal the decoded peak count; otherwise
  the reader returns `InvalidRecord` with the expected/actual counts.
- A shape that lacks `X` is rejected as malformed.
- A shape that lacks `Y` (e.g. `(XA)`) yields peaks with `y = 0.0` and a
  provenance warning `jcamp_peak_table_missing_y`.
- When both `##PEAK TABLE=` and `##PEAK ASSIGNMENTS=` are present in the same
  block, the ASSIGNMENTS form wins (strictly richer); the other table is
  preserved in metadata under `jcamp_peak_table_dropped` with a
  `jcamp_peak_table_multiple_blocks` warning.

The full peak list is also exported into the record metadata under
`jcamp_peak_table` so downstream code can recover per-peak attributes:

```json
{
  "jcamp_peak_table": {
    "kind": "peak_assignments",
    "shape": "(XYA)",
    "fields": ["x", "y", "assignment"],
    "packed": false,
    "sparse": true,
    "peak_count": 4,
    "peaks": [
      {"x": 3300.0, "y": 0.42, "assignment": "O-H stretch, broad"},
      {"x": 2950.0, "y": 0.18, "assignment": "C-H stretch"},
      {"x": 1650.0, "y": 0.85, "assignment": "C=C stretch"},
      {"x": 1050.0, "y": 0.55, "assignment": "C-O stretch"}
    ]
  }
}
```

Peak-table support is currently limited to top-level blocks; LINK children
carrying peak tables are still rejected (see the LINK section above).

## Fixtures and Reference Checks

Current committed controls:

| File | Encoding | Points | Axis | Value control |
|---|---|---:|---|---|
| `nist_water_ir.jdx` | plain AFFN | 3917 | `388.677 -> 3799.45426 cm-1` | `0.438 -> 0.885` |
| `nist_sucrose_ir.jdx` | two top-level XYDATA blocks | 7153 x 2 | `7498.994 -> 600.88399 cm-1` | reflectance first values `0.422011`, `0.471453` |
| `BRUKSQZ.DX` | SQZ | 16384 | `24038.5 -> 0.0 Hz` | `2259260 -> 1505988` |
| `BRUKDIF.DX` | DIF/DUP | 16384 | `24038.5 -> 0.0 Hz` | `2254931 -> 1513177` |
| `SPECFILE.DX` | mixed SQZ/DIF/DUP | 1801 | `400.0 -> 4000.0 cm-1` | `97.737187 -> 82.830985` |
| `BRUKNTUP.DX` | NTUPLES R/I pages | 16384 x 2 | `24038.5 -> 0.0 Hz` | real `2254931 -> 1513177`, imaginary `-6966283 -> -7303022` |
| `TESTFID.DX` | NTUPLES FID R/I pages | 16384 x 2 | `0.0 -> 0.6815317 s` | real `2979.837825 -> -60241.607962`, imaginary `6214.555864 -> -6063.227393` |
| `OceanOptics_period.jdx` | LINK + XYPOINTS | 3648 x 4 | `176.36 -> 893.69 nm` | computed transmittance `0.0 -> 171.977070` |
| `synthetic_peak_assignments.jdx` | PEAK ASSIGNMENTS `(XYA)` | 4 peaks | `3300 -> 1050 cm-1` | absorbance `0.42 -> 0.55`, sum `2.00` |

The current reader is still narrower than mature JCAMP libraries. It is meant
to cover the high-value NIR/IR `XYDATA` cases first, with warnings where the
legacy format stores extra line checkpoints.

## Remaining Work

- Wider real-world peak-table coverage: shape parsing handles all documented
  JCAMP-DX 5.0 variants today, but only one synthetic fixture is committed.
  Adding NIST WebBook or nzhagen peak-assignment fixtures would harden
  vendor-specific quirks (e.g. embedded `>` in assignment text,
  multi-line assignments, peak groups separated by `;`).
- Broader `LINK` variants beyond same-axis spectral children require a
  per-record fan-out decision in the public API; see the LINK section.
- Reference reports against open JCAMP readers for every committed fixture.
