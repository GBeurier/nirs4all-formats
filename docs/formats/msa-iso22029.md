# EMSA/MAS MSA (ISO 22029)

Experimental native Rust reader for EMSA/MAS `.msa` single-spectrum text files,
including ISO 22029:2022-style fixtures.

## Scope Implemented

- Sniffs `.msa` files with `#FORMAT` / `EMSA/MAS` headers.
- Parses header label/value records beginning with a single `#`.
- Preserves repeated header keys, including multi-line `#TITLE` and
  `#COMMENT`, under `metadata.emsa_mas`.
- Supports `#DATATYPE: XY` explicit X/Y pairs.
- Supports `#DATATYPE: Y` ordinate-only arrays with axis reconstruction from
  `#OFFSET`, `#XPERCHAN` and optional `#CHOFFSET`.
- Supports comma-separated and whitespace-separated numeric payloads,
  including scientific notation.
- Applies `#NPOINTS` as a consistency check, truncating extra values with a
  provenance warning.

The reader intentionally does not validate date/time conformance yet; invalid
metadata remains preserved for downstream reference-reader comparison.

## Fixtures and Reference Checks

Current committed controls:

| File | Data type | Points | Axis | Value control |
|---|---|---:|---|---|
| `ISO_22029_2022_compliance.msa` | `XY` | 21 | `520.13 -> 580.50 eV` | `4066 -> 4217` |
| `ISO_22029_2022_compliance_XY_NCOLUMNS2.msa` | `XY` | 21 | `520.13 -> 580.50 eV` | `4066 -> 4217` |
| `example2_NCOLUMNS5.msa` | `Y` | 80 | `0.0 -> 790.0 eV` | `65.820 -> 49.442` |
| `minimum_metadata.msa` | minimal | 1 | `0.0 index` | `1.0` |

Reference reader target: `rsciio.msa` from the HyperSpy/RosettaSciIO stack.

## Remaining Work

- Add full-array conformance reports against `rsciio.msa`.
- Add stricter ISO 22029 metadata validation as warnings, not hard failures.
- Decide whether EDS/ELS signal families deserve dedicated normalized
  `SignalType` variants or should remain `raw_counts`.
