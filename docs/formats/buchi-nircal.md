# BUCHI NIRCal

> **Status:** Supported (scoped) · **Vendor:** BUCHI / Bühler · **Extensions:** `.nir`

NIRCal is the project file written by BUCHI/Bühler NIRCal software for FT-NIR
calibration work. A `.nir` project bundles many sample spectra, a shared
wavenumber axis, per-sample property values and per-spectrum acquisition
metadata. nirs4all-formats reads the project file emitting one `SpectralRecord` per
sample.

## Instruments & software

Produced by BUCHI NIRCal software (NIRMaster / NIRFlex instrument families). The
committed fixture is the `prospectr::read_nircal()` reference file; a richer
local-only cannabis transfer file exercises non-null property targets and
replicate spectra.

## File structure

A text-keyed container that starts with `NIRCAL Project File`. Sections are
located by ASCII markers rather than fixed offsets: the `Spectra` selection block
holds sample identifiers, a `Wavelength Selection` block holds the wavenumber
axis, a `Properties` section holds property names and per-sample property blocks,
and a `Spectra Info` block holds per-spectrum metadata. Each spectrum is one
double64 array delimited by fixed-size `begin` / `end` markers; the axis is
emitted as wavenumber in `cm-1`. The token layout follows the observed
NIRCal 2.23 ordering.

## What nirs4all-formats extracts

- **Signals** — one `absorbance` `SpectralRecord` per sample; the sample id is
  stored in metadata. Duplicate sample ids remain distinguishable through
  promoted replicate counters.
- **Axis** — the shared wavenumber axis (`cm-1`), validated to match the declared
  spectrum length.
- **Targets** — property names and per-sample values become `targets`. When every
  value in the property table is zero, those values are exposed as `null` to
  match `prospectr::read_nircal()` missing-value semantics; in mixed tables, a
  real `0.0` is preserved. Duplicate property names are normalised with a warning.
- **Metadata** — project GUID, project-file version, project title, replicate
  index/count and per-spectrum `Spectra Info` fields promoted as flat snake_case:
  spectrum GUID, comment/description, scans, resolution, declared wavenumber
  geometry, device, instrument serials, software/instrument version, measurement
  cell, option serial, reference substance, creator/modifier (with logins),
  timestamps, computer name, and gain / instrument-temperature /
  sample-temperature diagnostics when present.
- **Provenance & warnings** — `buchi_nircal_reverse_engineered_sections` plus any
  target / duplicate-name warnings, source file and SHA-256.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| `.nir` project (spectra + axis + properties) | Supported | One record per sample; targets, replicates and spectrum metadata. |
| `.nir` with non-zero property targets | Supported (local-only) | Validated on a local cannabis transfer file; needs a redistributable fixture. |
| `.cal` calibration-only | Planned | Not yet parsed. |
| JCAMP-DX export | Planned | Routed to the JCAMP reader once fixtures exist. |
| NIRMaster / NIRFlex firmware variants | Planned | `Spectra Info` parser follows NIRCal 2.23; new firmware needs comparison against vendor exports. |

## Limitations & known gaps

- Non-zero reference properties are validated only locally; a small
  redistributable fixture is still needed before the target path is claimed
  publicly.
- Calibration-only `.cal` files, JCAMP-DX exports and broader NIRMaster / NIRFlex
  firmware variants are not yet handled.
- The `Spectra Info` parser tracks the observed NIRCal 2.23 token layout, so new
  firmware fixtures should be compared against vendor exports or `prospectr`
  before the compatibility claim is widened.

## Reference readers

`prospectr::read_nircal()` (R) is the naming and missing-value reference for this
format.

## Samples & validation

`samples/buchi_nircal/muestras-tejido-foliar_transfer.nir` (20 records, `cm-1`,
1501 points, `absorbance`; 20 property targets present but null) is golden-backed
and validates spectrum GUID, device/serial, scans/resolution and gain/temperature
metadata. The local-only `transpec_DEMO_cannabis.nir` (105 records, 3 replicate
spectra per sample) validates the non-null `CBDA` / `THCA` target path plus the
replicate, comment/timestamp and device/serial metadata. NIRCal `.nir` is
distinguished from Foss/WinISI `.NIR` by the `NIRCAL Project File` header, never
by extension.
