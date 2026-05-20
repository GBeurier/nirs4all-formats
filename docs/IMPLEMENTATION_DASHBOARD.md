# Implementation Dashboard

Last updated: 2026-05-20.

This page is the compact, visual companion to `FORMAT_MATRIX.md`. The matrix is
still the source of truth; this page makes the implementation maturity and
remaining sourcing work easier to scan.

## Variant Maturity

The matrix currently tracks 58 format families and 238 known variants.

| State | Count | Share | Bar |
|---|---:|---:|---|
| Validated | 145 | 61% | `##############################` |
| Blocked | 58 | 24% | `############` |
| Partial | 19 | 8% | `####` |
| Planned | 16 | 7% | `###` |

Interpretation:

- `Validated` means parsing and metadata are sufficient for that variant, with
  sample/test/documentation coverage.
- `Partial` means the parser is useful but knowingly incomplete.
- `Planned` means the variant is identified and actionable, but not coded yet.
- `Blocked` means a sample, specification, key generator, reference export or
  license clearance is missing.

## Format Coverage

| Coverage class | Families | Bar |
|---|---:|---|
| Diffusable | 25 | `#########################` |
| Diffusable cible | 14 | `##############` |
| Non viable | 7 | `#######` |
| Adjacent diffusable | 4 | `####` |
| Adjacent | 3 | `###` |
| Utile incomplet | 2 | `##` |
| Adjacent cible | 2 | `##` |
| Hors-scope | 1 | `#` |

`Diffusable` means the main active variants are covered enough to communicate a
clear public scope. `Diffusable cible` means the reader is useful if the
supported subset is stated explicitly. `Non viable` marks formats where coding
more without original files or specs would be mostly speculative.

## Missing Impact

| Impact | Families | Practical meaning |
|---|---:|---|
| Moyen | 20 | Useful now, but important variants or metadata remain. |
| Aucun | 11 | No known blocker for the current scope. |
| Mineur | 9 | Remaining work should not block diffusion. |
| Grave | 7 | A significant active variant or data path is missing. |
| Bloquant | 6 | Cannot reasonably claim the native format yet. |
| Hors perimetre | 5 | Adjacent or intentionally out of scope. |

## P0/P1 Action Board

These are the rows that should drive the next coding and sample-sourcing work.

| Format | Coverage | Impact | Next action |
|---|---|---|---|
| Foss NIRSystems / WinISI natif | non viable | bloquant | Source `.NIR/.DA/.cal/.eqa` natives before coding. |
| Perten DA / Inframatic | non viable | bloquant | Source native spectral export or wavelength-bearing CSV/XLSX. |
| ASD calibration | non viable | bloquant | Source `.asd + .ILL/.REF/.RAW` calibration sets. |
| PP Systems UniSpec DC | non viable | bloquant | Source real two-channel `.SPU` field acquisition. |
| PP Systems UniSpec SC | non viable | bloquant | Source real `.SPT` field acquisition. |
| Avantes AvaSoft 8 binaire | diffusable cible | grave | Source missing AVS8 suffixes and complete irradiance calibration variants. |
| Metrohm Vision / Vision Air | diffusable cible | grave | Source real Vision Air export and native project evidence. |
| Spectro Inc. SiWare API | utile incomplet | grave | Source real API JSON/CSV response. |
| ASD FieldSpec | diffusable | moyen | Decode or validate internal calibration/reference blocks when fixtures arrive. |
| Avantes AvaSoft 6/7 binaire | diffusable cible | moyen | Source binary `.ABS` and remaining legacy modes. |
| BUCHI NIRCal | diffusable cible | moyen | Source redistributable non-null target `.nir`, `.cal`, JCAMP-DX export and NIRMaster/NIRFlex variants. |
| JCAMP-DX | diffusable | moyen | Source real heterogeneous `LINK` and peak-table fixtures. |
| HDF5 NIRS generique | diffusable cible | moyen | Source richer real schemas with targets, metadata and nested groups. |
| Si-Ware NeoSpectra | diffusable | mineur | Source single-measurement Scanner export. |
| Spectral Evolution / PSR | diffusable | mineur | Source SR-3500/SR-6500 variants and reference comparisons. |
| SVC / GER SIG | diffusable | mineur | Add HR-1024i >=3.0 and byte-level reference comparisons. |
| VIAVI MicroNIR | diffusable | mineur | Source native `.pri`; CSV/XLSX path is already useful. |

## Probe Confidence

| Confidence | Current meaning | Examples |
|---|---|---|
| High | Signature, magic bytes, container schema or dedicated probe tests prevent extension-only routing. | ASD, BUCHI, OPUS, SPC, OMNIC, JWS, WDF, WIP, NetCDF/HDF5 schema refusals. |
| Medium | Text/CSV/Excel shape is validated by content and goldens, but vendor identity can depend on export conventions. | Spectral matrices, row-oriented tables, Vision Air CSV, Foss text/CSV exports, SiWare/NeoSpectra CSV/XLSX. |
| Guarded | A real payload is parsed, but the implementation is intentionally narrow or local-only until more fixtures arrive. | Microtops MAN NetCDF fallback, ARM MFRSR local NetCDF, Allotrope ADF local, BUCHI cannabis local, Horiba `.l6m`, WiTec `WIT_PR06`. |
| Blocked | No reliable native sample/spec/reference pair exists. | Foss native, Perten native, ASD companions, PP Systems raw, Metrohm native, VIAVI `.pri`. |

## Files To Source First

The exhaustive list remains in `FORMAT_MATRIX.md` under "Fichiers a sourcer
pour continuer". The shortest high-impact list is:

1. Foss native `.NIR/.DA/.cal/.eqa` plus text/CSV export of the same scans.
2. Perten DA/Inframatic native spectral files plus wavelength-bearing export.
3. ASD `.asd + .ILL/.REF/.RAW` calibration companion sets.
4. PP Systems raw `.SPT` and `.SPU` field acquisitions.
5. Avantes missing AVS8 and legacy binary suffixes, especially `.ABS` and
   irradiance-calibration cases.
6. BUCHI `.nir` with redistributable non-null targets, `.cal`, JCAMP-DX export
   and recent NIRMaster/NIRFlex variants.
7. Real Vision Air/OMNIS NIR exports and native project evidence.
8. Real Spectro Inc. SiWare API responses.

Update this dashboard whenever `FORMAT_MATRIX.md` changes materially.
