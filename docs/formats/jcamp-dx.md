# JCAMP-DX

Experimental native Rust reader for single-block JCAMP-DX `XYDATA`.

## Scope Implemented

- Sniffs labeled-data records containing `##JCAMP-DX=` or `##JCAMPDX=`.
- Reads one `XYDATA=(X++(Y..Y))` data table from the first block.
- Supports plain AFFN rows.
- Supports packed ASDF ordinate encodings:
  - PAC-style adjacent signed numbers;
  - SQZ pseudo-digits;
  - DIF difference-coded pseudo-digits;
  - DUP repeat counts.
- Applies `YFACTOR` to decoded ordinates.
- Reconstructs the X axis from `FIRSTX` and `DELTAX`, or from `FIRSTX`,
  `LASTX` and `NPOINTS` when `DELTAX` is absent.
- Uses `XUNITS` and `YUNITS` to map axis kind/unit and signal type.

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

## Fixtures and Reference Checks

Current committed controls:

| File | Encoding | Points | Axis | Value control |
|---|---|---:|---|---|
| `nist_water_ir.jdx` | plain AFFN | 3917 | `388.677 -> 3799.45426 cm-1` | `0.438 -> 0.885` |
| `BRUKSQZ.DX` | SQZ | 16384 | `24038.5 -> 0.0 Hz` | `2259260 -> 1505988` |
| `BRUKDIF.DX` | DIF/DUP | 16384 | `24038.5 -> 0.0 Hz` | `2254931 -> 1513177` |
| `SPECFILE.DX` | mixed SQZ/DIF/DUP | 1801 | `400.0 -> 4000.0 cm-1` | `97.737187 -> 82.830985` |

The current reader is still narrower than mature JCAMP libraries. It is meant
to cover the high-value NIR/IR `XYDATA` cases first, with warnings where the
legacy format stores extra line checkpoints.

## Remaining Work

- Add `NTUPLES` support and multi-signal page handling.
- Add `XYPOINTS`, `PEAK TABLE` and multi-block `LINK` files.
- Add stricter line-level X checkpoint verification.
- Add reference reports against open JCAMP readers for every committed fixture.
