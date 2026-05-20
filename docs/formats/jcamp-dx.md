# JCAMP-DX

Experimental native Rust reader for common JCAMP-DX `XYDATA` and `NTUPLES`
records.

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
- Applies `YFACTOR` to decoded ordinates.
- Reconstructs the X axis from `FIRSTX` and `DELTAX`, or from `FIRSTX`,
  `LASTX` and `NPOINTS` when `DELTAX` is absent.
- Uses `XUNITS`/`YUNITS` or NTUPLES `UNITS` to map axis kind/unit and signal
  type.
- Refuses `PEAK TABLE` blocks explicitly until that table model is implemented.

`XFACTOR` is preserved in metadata but is not applied to the reconstructed axis.
In the committed Bruker fixtures, `FIRSTX` and `DELTAX` are already in physical
units while line-level X checkpoints are stored in scaled integer form.

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
current generic `index` axis kind because the core schema does not yet expose a
dedicated time-axis enum value.

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

The current reader is still narrower than mature JCAMP libraries. It is meant
to cover the high-value NIR/IR `XYDATA` cases first, with warnings where the
legacy format stores extra line checkpoints.

## Remaining Work

- Implement `PEAK TABLE` as a real sparse peak-list representation once the
  shared model can represent it.
- Add broader `LINK` variants beyond same-axis spectral children.
- Add stricter line-level X checkpoint verification.
- Add reference reports against open JCAMP readers for every committed fixture.
