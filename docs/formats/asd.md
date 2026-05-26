# ASD FieldSpec

> **Status:** Supported (scoped) · **Vendor:** ASD / Malvern Panalytical · **Extensions:** `.asd` (also extension-less numeric files such as `.000`)

ASD FieldSpec is the native binary format of ASD (now Malvern Panalytical)
portable field spectrometers — FieldSpec, LabSpec and HandHeld instruments used
for reflectance, radiance and raw-counts measurements across the VNIR/SWIR
range. A single `.asd` file is a fixed header followed by the primary spectrum
and, in newer revisions, several embedded internal blocks (reference,
classifier, dependent variables, calibration, audit log, signature).
nirs4all-io decodes the primary spectrum of revisions 1, 6, 7 and 8 and
inventories the remaining blocks.

## Instruments & software

Written by ASD's RS³ / ViewSpec / Indico software for FieldSpec full-range,
FieldSpec Pro, LabSpec and HandHeld spectrometers. The instrument model is read
from the header and mapped to labels such as `fieldspec_full_range`,
`labspec_pro` or `handheld`.

## File structure

- **File-version magic** — the first three bytes carry an ASCII revision prefix:
  `ASD` (revision 1), then `as2`…`as8`. The format is detected by magic, so
  files without an `.asd` extension (e.g. `.000`) are still recognised.
- **Fixed header (484 bytes)** — comments, acquisition time, program/file
  version, dark/reference timestamps, data type, data format, first wavelength
  (`channel1`), wavelength step, channel count, integration time, foreoptic,
  detector gains/offsets, splice wavelengths, instrument and calibration
  identifiers, and sample/reference/dark counts.
- **Primary spectrum** — `channels` values in `float32` (data format 0),
  `int32` (1) or `float64` (2), immediately after the header.
- **Internal blocks** (revision-gated) — reference header + spectrum (rev ≥ 2),
  classifier data and dependent variables (rev ≥ 6), calibration header +
  spectra (rev ≥ 7), audit log and signature (rev 8), followed by an optional
  `ff fe fd` footer marker and zero padding.

## What nirs4all-io extracts

- **Signals** — one `SpectralRecord` carrying the primary spectrum as a single
  signal, named and typed from the header `data_type`: `reflectance`,
  `radiance`, `irradiance`, `transmittance`, `absolute_reflectance` or `raw`.
- **Axis** — a wavelength axis in `nm`, generated from `channel1 +
  wavelength_step × index`.
- **Metadata** — the full decoded fixed header under `metadata.asd`
  (acquisition time, versions, integration time, gains/offsets, splice
  wavelengths, display range, instrument/calibration labels, smart-detector
  type). An inventory of internal blocks is exposed under
  `metadata.asd.secondary_blocks`, with byte accounting in
  `trailing_block_bytes`, `decoded_trailing_block_bytes` and
  `undecoded_trailing_block_bytes`.
- **Provenance & warnings** — `asd_secondary_spectra_not_emitted` is raised when
  reference/calibration spectra are present but not emitted as records;
  `trailing_asd_blocks_not_decoded` is raised when bytes remain outside the
  known block inventory.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Revision 1 (`ASD`) primary spectrum | Supported | `float32` legacy `.000` covered. |
| Revisions 6 / 7 / 8 primary spectrum | Supported | `float64` reflectance, radiance and raw-counts fixtures covered. |
| `int32` / `float64` payload encodings | Supported | Selected by the header `data_format` field. |
| Embedded reference / calibration / classifier / dependent-variable blocks | Detected / refused | Inventoried with byte offsets and counts, but not decoded into additional signals or targets. |
| v8 audit log / digital signature blocks | Detected / refused | Summarised for diagnostics only. |
| Revisions 3 / 4 / 5 | Blocked | Recognised by magic but no open fixture to validate. |
| Separate `.ILL` / `.REF` / `.RAW` calibration companions | Blocked | No redistributable sample set yet (matrix: *non viable*). |

## Limitations & known gaps

- Embedded secondary spectra (reference, calibration) are counted and typed but
  their numeric payloads are not exposed as signals.
- Classifier and dependent-variable blocks are summarised for diagnostics, not
  promoted to calibrated quantitative `targets`.
- Revisions 3–5 and files exercising the full internal-block set still need open
  samples; the companion `.ILL`/`.REF`/`.RAW` calibration files have no
  redistributable fixture.

## Reference readers

`pyASDReader` (revision 6/7/8 metadata and spectrum checks) and
`prospectr::readASD()` (legacy `.000` coverage). `asdreader`, `specdal` and
`spectrolab` remain reference candidates for deeper conformance once the R
reference path is automated.

## Samples & validation

Six fixtures under `samples/asd/` are golden-backed in
`crates/nirs4all-io/tests/goldens/` with direct semantic tests:
`3L9257.000` (rev 1, `float32`, reflectance), `v6sample00000.asd` (rev 6, raw
counts), `v7_field_44231B009.asd` (rev 7, reflectance), `v7sample00000.asd`
(rev 7, radiance), `soil.asd` and `v8sample00001.asd` (rev 8, raw counts). The
probe reports format `asd-fieldspec` at `Confidence::Definite`.
