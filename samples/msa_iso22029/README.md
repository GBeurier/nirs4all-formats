# EMSA / MAS `.msa` (ISO 22029)

Standard ASCII format for storing single spectra, **codified as ISO 22029:2022**. Originally for X-ray microanalysis (EDS / WDS) but format is generic enough that any 1-D spectrum can be encoded.

## Samples

All from [`hyperspy/rosettasciio@main/rsciio/tests/data/msa/`](https://github.com/hyperspy/rosettasciio/tree/main/rsciio/tests/data/msa) — GPL-3.0.

| File | Notes |
|---|---|
| `ISO_22029_2022_compliance.msa` | Reference ISO-compliant spectrum. |
| `ISO_22029_2022_compliance_XY_NCOLUMNS2.msa` | 2-column XY data variant. |
| `ISO_22029_2022_compliance_scientific_notation.msa` | Exercises scientific-notation values. |
| `ISO_22029_2022_compliance_title_multiple_line.msa` | Multi-line `##TITLE` test. |
| `example1.msa`, `example2.msa` | Generic examples. |
| `example1_with_seconds.msa` | With sub-second timestamp. |
| `example1_wrong_date.msa`, `example1_wrong_date_empty_field.msa` | Invalid date metadata examples kept for future warning-level ISO validation. The current reader preserves the metadata instead of rejecting the spectrum. |
| `example2_NCOLUMNS5.msa` | 5-column variant. |
| `minimum_metadata.msa` | Minimum-required-metadata baseline. |

## Parser hints

- Header keys look very much like JCAMP-DX (`#TITLE`, `#XPERCHAN`, `#YOFFSET`, `#XUNITS`, `#YUNITS`, …) — but use a single `#` rather than `##`. Don't conflate with JCAMP-DX.
- Reference reader: [`rsciio.msa`](https://hyperspy.org/rosettasciio/).
- Useful as a **standards-track reference** for the loader's text-format unit handling — the ISO 22029 specification is explicit about unit semantics and decimal locale.
