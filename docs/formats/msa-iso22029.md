# EMSA/MAS MSA (ISO 22029)

> **Status:** Supported (scoped) · **Vendor:** EMSA / MAS / ISO · **Extensions:** `.msa`

EMSA/MAS `.msa` is the single-spectrum ASCII interchange format from the
Microscopy Society of America / European Microbeam Analysis Society, standardised
as ISO 22029. It is used mainly for EDS/EELS microanalysis spectra, where the X
axis is typically energy in `eV`. nirs4all-io reads it as an adjacent, narrow
text format so these files can be probed and disambiguated.

## Instruments & software

Written by electron-microscopy and microanalysis software (EDS / EELS / WDS
acquisition packages) following the EMSA/MAS specification and ISO 22029:2022.
Committed fixtures include ISO 22029:2022 compliance examples and the classic
EMSA `example1`/`example2` spectra.

## File structure

Plain ASCII with two parts:

- **Header** — label/value records each beginning with a single `#`, e.g.
  `#FORMAT`, `#TITLE`, `#NPOINTS`, `#XUNITS`, `#YUNITS`, `#XPERCHAN`, `#OFFSET`,
  `#CHOFFSET`, `#DATATYPE`. Repeated keys (including multi-line `#TITLE` and
  `#COMMENT`) are kept. The data section begins after `#SPECTRUM` and ends at
  `#ENDOFDATA`.
- **Data** — either `#DATATYPE: XY` explicit X/Y pairs, or `#DATATYPE: Y`
  ordinate-only arrays. Numeric payloads may be comma-, semicolon-, tab- or
  whitespace-separated, including scientific notation.

The reader sniffs files with an `.msa` extension or an `EMSA/MAS` marker that also
contain `#FORMAT`.

## What nirs4all-io extracts

- **Signal** — one `SpectralRecord` with a single signal named from `#YLABEL`
  (default `signal`). Signal type is inferred from the Y label/units; an
  `intensity` label with no recognised optical type maps to `RawCounts`,
  otherwise `Unknown`.
- **Axis** — for `XY` data the X column is the axis; for `Y` data the axis is
  reconstructed as `OFFSET + (index + CHOFFSET) * XPERCHAN`. The kind/unit follow
  `#XUNITS`: energy `eV`, wavenumber `cm-1`, wavelength `nm`, else `index`.
- **Metadata** — the full header (with repeated keys preserved) is stored under
  `metadata.emsa_mas`.
- **Provenance & warnings** — source file + SHA-256, reader name/version; a
  `#NPOINTS` consistency check truncates extra values with a provenance warning.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| `#DATATYPE: XY` (incl. `NCOLUMNS 2`) | Supported | Explicit energy/value pairs. |
| `#DATATYPE: Y` ordinate-only | Supported | Axis from `OFFSET`/`XPERCHAN`/`CHOFFSET`. |
| Scientific-notation payloads | Supported | Parsed in either data type. |
| Multi-line `#TITLE` / `#COMMENT` | Supported | Repeated header keys preserved. |
| Invalid date/time, empty fields | Supported (preserved) | Kept verbatim for reference comparison; not yet validated. |

## Limitations & known gaps

- Date/time and other ISO 22029 metadata fields are not yet validated; invalid
  metadata is preserved rather than flagged. Stricter validation would be emitted
  as warnings, not hard failures.
- EDS/EELS signal families are currently typed as `RawCounts`/`Unknown`; whether
  they deserve dedicated `SignalType` variants is undecided.
- Only single-spectrum `.msa` files are handled (the format itself is
  single-spectrum).

## Reference readers

`rsciio.msa` from the HyperSpy / RosettaSciIO stack is the reference loader.
Full-array conformance reports against it are still to be added.

## Samples & validation

Fixtures live under `samples/` and are covered by golden summaries in
`crates/nirs4all-io/tests/goldens/`; the probe reports format `emsa-mas-msa` at
`Confidence::Definite`. Committed control values:

| File | Data type | Points | Axis | Value control |
|---|---|---:|---|---|
| `ISO_22029_2022_compliance.msa` | `XY` | 21 | energy `520.13 → 580.50 eV` | `4066 → 4217` |
| `ISO_22029_2022_compliance_XY_NCOLUMNS2.msa` | `XY` | 21 | energy `520.13 → 580.50 eV` | `4066 → 4217` |
| `ISO_22029_2022_compliance_scientific_notation.msa` | `XY` | 21 | scientific notation | golden summary |
| `ISO_22029_2022_compliance_title_multiple_line.msa` | `XY` | 21 | multi-line title metadata | golden summary |
| `example1.msa`, `example1_with_seconds.msa`, `example2.msa` | examples | variable | metadata/date variants | golden summaries |
| `example1_wrong_date.msa`, `example1_wrong_date_empty_field.msa` | `XY` | 20 | invalid date/empty fields preserved; extra payload row truncated with warning | golden summaries |
| `example2_NCOLUMNS5.msa` | `Y` | 80 | energy `0.0 → 790.0 eV` | `65.820 → 49.442` |
| `minimum_metadata.msa` | minimal | 1 | `0.0 index` | `1.0` |
