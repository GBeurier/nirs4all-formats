# Metrohm Vision / Vision Air

> **Status:** Supported (scoped) · **Vendor:** Metrohm · **Extensions:** `.csv` (export); native project database (blocked)

Metrohm Vision / Vision Air drives Metrohm NIR analyzers. The native project
database is a closed binary store, but Vision Air can export results as a
spectral matrix. nirs4all-io reads that CSV export through the generic
[spectral-matrix reader](row-spectral-table.md); the native project DB is not
decoded.

## Instruments & software

Produced by Metrohm Vision / Vision Air (and the related OMNIS NIR workflow) for
Metrohm FT-NIR / NIR instruments. The export is a wide table where each row is
one sample. The committed fixture is a synthetic Vision Air CSV; a real
license-cleared customer export is still wanted.

## File structure

The supported path is a one-spectrum-per-row matrix: an optional metadata /
title preamble, then a header row whose numeric columns are the wavelength axis,
preceded by sample-identifier and property columns. The delimiter is
auto-detected.

## What nirs4all-io extracts

- **Signal** — one `absorbance` signal per row, axis in `nm`.
- **Targets** — early numeric property columns (e.g. `protein`, `moisture`,
  `fat`) become per-record targets.
- **Metadata** — the export title is preserved under `metadata.vendor.title`,
  the sample identifier under `metadata.sample_id`, and the source row under
  `metadata.row_index`.
- **Provenance** — source file + SHA-256, reader name and version.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Vision Air CSV matrix export | Supported (scoped) | Read via the spectral-matrix reader; numeric wavelength columns required. |
| Native Vision project database | Blocked | Closed binary store; no public schema or redistributable fixture. |
| OMNIS NIR export | Planned | Wanted alongside real Vision Air exports. |

## Limitations & known gaps

- Target-only reports and native project stores are not promoted to records:
  without a spectral axis they expose no `SpectralRecord`.
- Vendor metadata beyond the title is not yet normalized into typed instrument /
  method / product / project fields.
- No reference comparison against a trusted Metrohm import workflow yet.

## Reference readers

Vision Air CSV exports are equally readable with `pandas` or a plain text
parser; nirs4all-io adds axis detection, signal typing, target promotion and
provenance. No open reader exists for the native project database.

## Samples & validation

`samples/metrohm/synthetic_visionair.csv` is golden-backed: 50 records, a
200-point `nm` axis (1100–2500 nm), an `absorbance` signal, and `protein` /
`moisture` / `fat` targets. The probe reports `spectral-matrix` at
`Confidence::Likely`.
