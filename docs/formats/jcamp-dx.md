# JCAMP-DX

Experimental native Rust reader for common JCAMP-DX `XYDATA` and `NTUPLES`
records.

## Scope Implemented

- Sniffs labeled-data records containing `##JCAMP-DX=` or `##JCAMPDX=`.
- Reads one `XYDATA=(X++(Y..Y))` data table from the first block.
- Supports plain AFFN rows.
- Supports packed ASDF ordinate encodings:
  - PAC-style adjacent signed numbers;
  - SQZ pseudo-digits;
  - DIF difference-coded pseudo-digits;
  - DUP repeat counts.
- Reads NMR `NTUPLES` pages with `VAR_FORM=AFFN,ASDF,ASDF,AFFN` and emits
  real/imaginary channels as separate normalized signals on the same record.
- Applies `YFACTOR` to decoded ordinates.
- Reconstructs the X axis from `FIRSTX` and `DELTAX`, or from `FIRSTX`,
  `LASTX` and `NPOINTS` when `DELTAX` is absent.
- Uses `XUNITS`/`YUNITS` or NTUPLES `UNITS` to map axis kind/unit and signal
  type.

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

## Fixtures and Reference Checks

Current committed controls:

| File | Encoding | Points | Axis | Value control |
|---|---|---:|---|---|
| `nist_water_ir.jdx` | plain AFFN | 3917 | `388.677 -> 3799.45426 cm-1` | `0.438 -> 0.885` |
| `BRUKSQZ.DX` | SQZ | 16384 | `24038.5 -> 0.0 Hz` | `2259260 -> 1505988` |
| `BRUKDIF.DX` | DIF/DUP | 16384 | `24038.5 -> 0.0 Hz` | `2254931 -> 1513177` |
| `SPECFILE.DX` | mixed SQZ/DIF/DUP | 1801 | `400.0 -> 4000.0 cm-1` | `97.737187 -> 82.830985` |
| `BRUKNTUP.DX` | NTUPLES R/I pages | 16384 x 2 | `24038.5 -> 0.0 Hz` | real `2254931 -> 1513177`, imaginary `-6966283 -> -7303022` |
| `TESTFID.DX` | NTUPLES FID R/I pages | 16384 x 2 | `0.0 -> 0.6815317 s` | real `2979.837825 -> -60241.607962`, imaginary `6214.555864 -> -6063.227393` |

The current reader is still narrower than mature JCAMP libraries. It is meant
to cover the high-value NIR/IR `XYDATA` cases first, with warnings where the
legacy format stores extra line checkpoints.

## Remaining Work

- Add `XYPOINTS`, `PEAK TABLE` and multi-block `LINK` files.
- Add stricter line-level X checkpoint verification.
- Add reference reports against open JCAMP readers for every committed fixture.
