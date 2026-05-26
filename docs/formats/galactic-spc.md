# Thermo / Galactic GRAMS SPC

> **Status:** Supported (scoped) · **Vendor:** Thermo / Galactic (GRAMS) · **Extensions:** `.spc`

SPC is the Galactic / Thermo GRAMS spectroscopy container, one of the most widely
exchanged formats across IR, NIR, Raman and UV-Vis. A single `.spc` file can hold
one spectrum or many subfiles, with a shared or per-subfile X axis.
nirs4all-io reads it by the file-version byte, never by extension, and emits one
normalized `SpectralRecord` per readable subfile.

## Instruments & software

GRAMS SPC is produced by Galactic / Thermo software and exported by a broad range
of instruments and conversion tools. Because `.spc` is reused by unrelated Ocean
Optics, Shimadzu, Renishaw and hyperspectral fixtures, the reader dispatches on
the header byte rather than the extension.

## File structure

A 512-byte new-header (or 256-byte old-header) followed by per-subfile headers and
Y arrays, with optional log and directory blocks. The reader decodes the header
flag byte (`TSPREC`, `TCGRAM`, `TMULTI`, `TRANDM`, `TORDRD`, `TALABS`, `TXYXYS`,
`TXVALS`) to choose the X-axis source and Y encoding. X arrays are either
generated evenly from `ffirst` / `flast` / `fnpts` or read explicitly when
`TXVALS` is set; independent-X `-XYXY` files carry per-subfile directory offsets.
Y arrays are decoded as fixed-point 32-bit, fixed-point 16-bit (`TSPREC`) or IEEE
float32.

## What nirs4all-io extracts

- **Signals** — one `SpectralRecord` per readable subfile. Multi-subfile common-X
  files keep all subfiles usable by the tabular adapters; independent-X `-XYXY`
  files are exposed as raw records and intentionally rejected by `open_dataset()`
  until a resampling / ragged-array policy exists.
- **Axis** — axis and signal labels come from the SPC enumerations, with `TALABS`
  custom labels overriding the enum labels when present. SPC time-axis labels
  (`Seconds`, `Minutes`, hours, days, years and sub-second units) are mapped to
  `AxisKind::Time`.
- **Metadata** — the decoded global header under `galactic_spc`, the subfile
  header under `galactic_spc_subfile`, the optional `galactic_spc_log` key/value
  block, a `data_layout` field for single/common/independent-X layouts, and a
  top-level `sample_id` (from `SUBFILE<n>` log labels when available, otherwise
  `subfile_<n>`).
- **Provenance** — source file, SHA-256 and warnings (e.g. invalid integer
  exponents preserved, old-header limited-mode notes).

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| New little-endian (`FVERSN = 0x4B`) | Supported | Generated-X, explicit-X, multi common-X and independent-X `TXYXYS`. |
| Float32 / fixed-point 16- and 32-bit Y | Supported | Including `TSPREC` 16-bit and IEEE float32. |
| Time-axis labels | Supported | Mapped to `AxisKind::Time`. |
| Old little-endian (`FVERSN = 0x4D`) | Partial | Generated-X and old word-swapped Y decoded; old XY / log variants incomplete. |
| New big-endian (`FVERSN = 0x4C`) | Detected / refused | Recognised by sniff (`Confidence::Possible`) then refused with a clear not-implemented error. |
| NMR / FID (adjacent) | Supported (adjacent) | Readable as raw counts over a seconds time axis; not a core NIRS scope guarantee. |

## Limitations & known gaps

- New big-endian (`0x4C`) is recognised but not decoded; full old-format
  multi/ordered-Z semantics, binary log payloads and promotion of quantitative
  calibration into `targets` are not implemented.
- Independent-X `-XYXY` files are not assembled into a dataset until a resampling
  or ragged-array policy is added.
- `spc_spectra` does not implement new big-endian and is unreliable for at least
  one old ordered-Z fixture, so old-format promotion needs an additional
  independent review. Adversarial truncation tests and full-array reference
  conformance for representative new-LSB fixtures remain to be added.

## Reference readers

Cross-checked against the local `spc_spectra` Python reader for the new-LSB
fixtures; `rohanisaac/spc`, `specio`, SpectroChemPy, `xylib` and `spc-parser` are
further references for the family.

## Samples & validation

A broad corpus under `samples/galactic_spc/` is golden-backed with direct
semantic tests across IR / Raman / UV-Vis / NIR and an adjacent NMR-FID control,
including multi-subfile generated-X, directory-backed `TXYXYS`, limited old
ordered-Z, and minute/second time axes (`s_xy.spc`, `NMR_FID.SPC`). Important
controls include `BENZENE.SPC` (1842 absorbance points, first Y `0.1015599817`,
sum `189.390214`), `s_xy.spc` (512 points, minute time axis, first X
`1.0866667032`, first Y `45333`, sum `30065112`), `OceanOptics.spc` (first X
`176.3604126`, last X `893.6943359`, last Y `119.4251709`), `nir.spc` (20 records,
700 points each), `m_xyxy.spc` (512 records, first subfile 8 points, sum `45327`)
and `DRUG_SAMPLE.SPC` (400 directory-backed `TXYXYS` records over a descending
`m/z` axis, first record sum `245071`). The probe reports `galactic-spc` at
`Confidence::Definite` for new-LSB, `Likely` for old-LSB and `Possible` for the
recognised-but-refused big-endian header; `PK`-prefixed ZIP archives are excluded
to disambiguate from other containers.
