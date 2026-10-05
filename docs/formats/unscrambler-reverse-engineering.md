---
orphan: true
---

# Unscrambler revision 35 and labelled MAT export investigation

## Evidence and provenance

The user supplied a `.00D` and a `.mat` export of the same dataset on
2026-10-05. No redistribution license was supplied: both originals are
local-only, never release assets. Analysis used the bytes, Python `struct` /
NumPy and SciPy `loadmat`; no vendor implementation or GPL reference reader
was imported into runtime code. Synthetic public fixtures contain invented data.

The native file has 4,660,332 bytes and the export 4,845,752 bytes.
Their SHA-256 hashes are respectively
`e10222c646017e1a413763b9d873f85e183dcb3374cd174b29888b1e362a06b0`
and `6a92cc1145aafffed234b0f201f34884a0fd076f7d84a64f4cd08003e4208447`.

## Observed native layout

The ASCII header identifies non-design data and revision `35`; the binary
revision word at offset `0x5c` agrees. Offset `0xf8` contains dataset count 1;
offset `0xfc` points to the dataset at `0x300`. These fields are validated,
and the decoder follows the offset rather than scanning for payload patterns.

Each dataset block has a 16-byte header of little-endian uint32
`[type, id, count, stride]`, followed by exactly `count * stride` bytes.

| ID | Type | Stride | Observed offset | Meaning |
|---|---:|---:|---|---|
| 1000 | 11 | 276 | `0x300` | Dataset header: UTF-16LE name at payload +4, columns at +172, rows at +176. |
| 1001 | 30 | 112 | `0x424` | Dataset settings; undocumented words retained. |
| 1002 | 13 | 128 | `0x4a4` | Row descriptors; one descriptor retained per record. |
| 1003 | 14 | 92 | `0x11b4` | Column descriptors; retained on the first record. |
| 1004 | 26 | 600 | `0x1ca5ec` | Six sample groups. |
| 1005 | 26 | 600 | `0x1cb40c` | Sixteen variable groups. |
| 1006 | 12 | 32 | `0x1cd99c` | Fixed-width UTF-16LE variable labels. |
| 1007 | 12 | 32 | `0x26ca6c` | Fixed-width UTF-16LE object labels. |
| 1008 | 12 | 32 | `0x26cdbc` | Dataset label. |
| 1009 | 1 | 4 | `0x26cdec` | 529,308 float32 values, column-major; payload begins `0x26cdfc`. |

Group entries store a UTF-16LE name in bytes 0–79, two vendor flag words at
80/84, and a NUL-terminated ASCII selection in bytes 88–343. Selections use
one-based inclusive ranges and comma-separated indices, such as `1-6,21-26`.
Remaining words are preserved without assigning undocumented semantics.

## Numerical and schema checks

SciPy identifies 26 rows, 20,358 columns, a `VarLabels0` character matrix,
`ObjLabels`, and one numeric matrix. The numeric matrix name is arbitrary;
matching label dimensions selects it. MATLAB column-major character matrices
are decoded by row, including UTF-16 surrogate pairs. Both byte orders and
compressed MAT elements have synthetic test coverage.

Exactly 151 native words have bytes `be 2f 53 e7`, the float32 sentinel
`-9.973e23`; each maps to a NaN at the same position in the export. Every other
value agrees within `1e-6` relative / `1e-9` absolute tolerance. The export has
decimal rounding: maximum absolute difference is `0.004453125000509317` and
maximum difference scaled by `max(1, abs(export))` is
`4.994248188533474e-7`. Bit equality with exported doubles is inappropriate.

The first 66 labelled columns become targets. The remaining columns have `*`
labels. Native leaf selections partition these into six signals with lengths
4,096 / 1,050 / 4,096 / 1,050 / 5,000 / 5,000. Parent selections are kept as
metadata without duplicating signals. The MAT export contains no group table:
all 20,292 unlabelled columns remain one signal, preserving column order.

No wavelength/time calibration or physical signal unit can be verified from
these files. Index axes and `unknown` signal types are therefore deliberate,
with provenance warnings. Named column-to-target mapping is a documented
convention: reference variables and explanatory variables cannot be
distinguished reliably. Duplicate object labels do not merge records.

## Acceptance and boundaries

Rust tests compare every value across the private pair when available;
SciPy conformance independently compares every signal/target against the
export. Public generated fixtures are golden-backed. Error tests cover
truncation, counts, offsets, revisions, selection bounds and overlapping groups.
All source values are represented by signals or targets, and undocumented
header/settings/descriptor words are preserved in vendor metadata.

Only the observed single-dataset, Unicode non-design revision 35 is qualified.
Other revisions, design/model files, multiple datasets, ambiguous MAT matrices
and physical calibration require additional evidence. This investigation does
not establish the semantics of every vendor flag or claim universal Unscrambler
support.
