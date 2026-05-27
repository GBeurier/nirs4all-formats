# PerkinElmer Spectrum / IR

> **Status:** Supported (scoped) · **Vendor:** PerkinElmer · **Extensions:** `.sp` (`.fsm` imaging out of v1 scope)

PerkinElmer's Spectrum and Spotlight software write the `PEPE` block container.
nirs4all-formats reads the `.sp` single-spectrum flavour of that container; the `.fsm`
Spotlight imaging flavour shares the family magic but is intentionally left out of
the v1 scope.

## Instruments & software

Produced by PerkinElmer Spectrum software across the FT-IR / FT-NIR ranges and by
Spotlight imaging systems. The committed `.sp` fixture comes from the `specio`
project.

## File structure

A `PEPE` magic at offset 0, a fixed description field, then a root block (id 120)
whose payload is a recursive sequence of typed little-endian blocks. Each block
has a 6-byte header (id + signed payload length); container blocks nest further
block sequences. Data-bearing blocks are tagged: f64 pair (`0x751d`), single f64
(`0x751b`), i32 (`0x752b`), string (`0x7523`) and f64 array (`0x7516`). The
ordinate array, axis bounds, step and point count are read from their specific
block ids and tags.

## What nirs4all-formats extracts

- **Signals** — one `SpectralRecord` per file with the f64 ordinate array. The
  signal type is inferred from the signal label, and falls back to `absorbance`
  when the signal unit is `A`.
- **Axis** — generated from the typed `first_x` / `step` / `point_count` blocks
  (with `last_x` used to infer the step when needed). The axis kind follows the
  declared unit: wavenumber (`cm-1`), wavelength (`nm` / `um`), otherwise index.
- **Metadata** — description, signal min/max, x-step and point count, plus
  string blocks for sample id, instrument, instrument serial, software, detector,
  source type, beam splitter, apodization, measurement / processing / ordinate
  modes, accessory, ratio mode and scan date when present.
- **Provenance & warnings** — a `perkin_elmer_reverse_engineered_blocks` warning,
  source file and SHA-256.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| `.sp` Spectrum single spectrum | Supported | Typed block table, f64 ordinate array, rich instrument metadata. |
| `.fsm` Spotlight imaging | Detected / refused | Recognised by the `PEPE` magic and `.fsm` extension, then refused with a clear unsupported-imaging error rather than misread as a 1D spectrum. |
| PE NIR / Lambda variants | Planned | Need sample-backed validation. |

## Limitations & known gaps

- `.fsm` Spotlight imaging is deliberately out of scope for v1; an image cube is
  never interpreted as a single spectrum.
- The committed `.sp` fixture carries a footer whose scan range disagrees with
  the typed axis blocks; the reader treats the typed blocks as canonical and
  leaves the footer text for future reverse-engineering.
- PE NIR / Lambda variants are not yet sample-backed. A future split of `.sp`
  from `.fsm` would let the `.sp` scope be promoted on its own.

## Reference readers

`specio` reads the same `.sp` container and is the practical cross-check for this
format.

## Samples & validation

The real `specio` fixture `samples/perkin_elmer/spectra.sp` (1 record, `cm-1`,
3301 points, `absorbance` with unit `A`) is golden-backed and exercised by a
direct semantic test. The probe reports `perkin-elmer-sp` at
`Confidence::Definite`, and `perkin-elmer-fsm` (also `Definite`) for the
recognised-but-refused imaging variant.
