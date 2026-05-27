# Bruker OPUS

> **Status:** Supported (scoped) · **Vendor:** Bruker · **Extensions:** native numeric (`.0`, `.1`, `.001`, `.0000`, often no fixed extension); `.dpt` ASCII export

OPUS is Bruker's native FT-IR / FT-NIR / Raman file format, written by the OPUS
software that ships with Bruker spectrometers. A single OPUS file is a binary
container that can hold several related blocks (absorbance, reflectance, raw
sample/reference single-beam spectra, interferograms and phase) for one
acquisition. nirs4all-formats reads modern OPUS binaries and the two-column `.dpt`
text export.

## Instruments & software

Produced by Bruker OPUS software across the FT-NIR and FT-IR ranges — e.g. MPA /
MPA II, Tango, Matrix, Vertex and Alpha. The committed corpus includes Bruker
MPA soil fixtures (AfSIS). `.dpt` is the plain ASCII export OPUS can write for
interchange.

## File structure

- **Native binary** — detected by binary magic (`0a 0a fe fe`), never by
  extension; OPUS files commonly use numeric extensions such as `.0` or `.001`.
  The layout is a header + a directory of typed blocks: parameter blocks
  (integer/float/string) and data blocks, with each data block paired to a
  matching data-status parameter block that carries its axis bounds and scaling.
- **`.dpt` export** — two-column ASCII (wavenumber, value); the wavenumber axis
  is in `cm⁻¹`.

## What nirs4all-formats extracts

- **Signals** — one `SpectralRecord` per file with a `signals` map using
  semantic names rather than OPUS abbreviations: `absorbance`, `reflectance`,
  `sample_spectrum`, `reference_spectrum`, `sample_interferogram`,
  `reference_interferogram`, `sample_phase`, `match` and `match_2ch` when
  present. Duplicate block names get stable suffixes (e.g. `absorbance_2`).
- **Axis** — generated from the data-status `FXV`/`LXV` bounds and `NPT` point
  count, with `CSF` scaling and `NPT` trimming applied. `DXU=MIN` axes are typed
  as `AxisKind::Time`.
- **Metadata** — header/directory under `bruker_opus`; per-signal data-status
  parameters under `bruker_opus_signal_params`; other parameter blocks under
  `bruker_opus_params`.
- **Provenance & warnings** — unsupported or unpaired data blocks are preserved
  as provenance warnings rather than dropped.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| OPUS 7/8 native, MPA | Supported | New magic, directory, parameter + 1D data/status blocks. |
| Multi-signal acquisition | Supported | Absorbance, reflectance, sample/reference spectra, interferograms, phase. |
| `.dpt` ASCII export | Supported | Two-column, `cm⁻¹` axis. |
| Bruker Tango / Matrix native | Planned | MPA is covered; dedicated Tango/Matrix fixtures still wanted. |
| OPUS 5/6 legacy (old magic `0a 0a 1a 1a`) | Blocked | The sniff emits a `Possible` candidate so dispatch routes here, then the decoder refuses explicitly with "unsupported or missing Bruker OPUS magic". |
| 3D / time-resolved series, imaging, report tables | Out of scope (v1) | Not decoded yet. |

## Limitations & known gaps

- Old-magic (OPUS 5/6) files are deliberately refused rather than mis-routed.
- 3D/time-resolved data series, image blocks and report/subreport tables are not
  decoded; quantitative report values are not yet promoted into `targets`.
- Full parameter-label expansion and typed promotion of sample properties are
  pending.

## Reference readers

Cross-checked against `brukeropus` (used as the naming/order reference for
duplicate 1D blocks), `opusFC` (directory content and primary arrays),
`brukeropusreader`, `opusreader2` (spectral-cockpit) and SpectroChemPy.

## Samples & validation

The full `samples/bruker_opus/` corpus is golden-backed, with direct semantic
tests over cross-reader fixtures from `opusreader2`, `opusreader`, `brukeropus`,
SpectroChemPy and AfSIS Bruker MPA. Spot-checked control values include
`617262_1TP_C-1_A5.0` (absorbance first X `7497.697861`, first Y `0.552472949`)
and `test_spectra.0` (reflectance first X `7498.291691`, first Y `0.524343193`).
Readers disagree on some older or report-like blocks; those are tracked as
provenance warnings. Full-array external conformance scripts are listed as next
work in `docs/CONFORMANCE.md`.
