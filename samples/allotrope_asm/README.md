# Allotrope ASM (Allotrope Simple Model — JSON)

The Allotrope Foundation defines two formats:

1. **ADF** — binary HDF5 + RDF triplestore. **No public sample exists** (see `allotrope_adf/` for the documented gap).
2. **ASM** — JSON-Schema-based "Simple Model", which is what most third-party tooling actually targets in production. This is the format ingested by [Benchling/allotropy](https://github.com/Benchling-Open-Source/allotropy).

This directory contains **ASM JSON** instances, since they're the practical Allotrope path.

## Samples

All from [`Benchling-Open-Source/allotropy`](https://github.com/Benchling-Open-Source/allotropy) — **MIT**.

| File | Source path | Notes |
|---|---|---|
| `spectrum_emission_data.json` | `tests/parsers/agilent_gen5/testdata/fluorescence/spectrum_emission_data.json` | Agilent Gen5 plate reader → ASM. Fluorescence emission spectrum. |
| `ACSINS_absorbance_spectrum.json` | `tests/parsers/moldev_softmax_pro/testdata/ACSINS_absorbance_timeformat_spectrum.json` | Molecular Devices SoftMax Pro → ASM. Absorbance spectrum. |
| `MD_SMP_absorbance_example.json` | `tests/parsers/moldev_softmax_pro/testdata/MD_SMP_absorbance_endpoint_example01.json` | SoftMax Pro absorbance endpoint readings → ASM. |
| `LICENSE_benchling_open_source.txt` | `LICENSE.txt` | MIT license text from the Benchling allotropy repo. |

## Parser hints

- ASM JSON top-level key is `$asm.manifest` (URL pointing at a `*.manifest` definition in `purl.allotrope.org`).
- Sub-structure: `<instrument-type> aggregate document` → `<instrument-type> document` (list) → `measurement aggregate document` → `measurement document` → `device control document` + `sample document` + the actual `<measurement>`.
- Reference parser: [`Benchling-Open-Source/allotropy`](https://github.com/Benchling-Open-Source/allotropy) — most mature parser ecosystem, covers ~40 instrument-vendor flavours. Use it to convert original vendor files to ASM when available.
- `nirs4all-io` now ships a narrow ASM bridge for plate-reader spectral data cubes and detector-wavelength endpoint readings. It is not a full ASM/ADF implementation.
