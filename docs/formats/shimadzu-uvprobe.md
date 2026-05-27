# Shimadzu UVProbe

> **Status:** Supported (scoped) · **Vendor:** Shimadzu · **Extensions:** `.txt` (export); native `.spc` (planned)

UVProbe is Shimadzu's UV-Vis / NIR acquisition software. nirs4all-formats reads its
ASCII `.txt` export through the
[row-spectral-table reader](row-spectral-table.md). The native Shimadzu `.spc`
container is proprietary: it shares an extension with Galactic / Thermo GRAMS SPC
but not its binary layout, so dispatch must never be based on `.spc` alone.

## Instruments & software

Produced by Shimadzu UVProbe for Shimadzu UV-Vis / NIR spectrophotometers. The
text export is an axis-first table (often quoted CSV-style). The committed
fixture is a synthetic UVProbe export; a real redistributable export and a
licensed native `.spc` fixture are still wanted.

## File structure

A `"Spectrum Data"` title row followed by an axis-first table: a wavelength
column and one or more sample columns. The reader auto-detects the delimiter and
preserves the declared axis order.

## What nirs4all-formats extracts

- **Signal** — one signal per sample column (e.g. `sample_s000`), axis in `nm`.
  The signal type stays `Unknown` because the export header identifies only a
  sample column, not absorbance / transmittance / reflectance.
- **Metadata** — the `"Spectrum Data"` title row is preserved as a note.
- **Provenance** — source file + SHA-256, reader name and version.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| UVProbe `.txt` text export | Supported (scoped) | Read via the row-spectral-table reader; synthetic fixture only so far. |
| Native Shimadzu `.spc` | Planned | Recognised only at extension level; never claimed by `.spc` alone. |

## Limitations & known gaps

- The native `.spc` is not decoded: nirs4all-formats reports only an extension-level
  candidate unless the binary matches a known Galactic / Thermo SPC header.
- Typed signal-role detection is pending a real UVProbe export that exposes a
  measurement-mode field.

## Reference readers

The `.txt` export is readable with `pandas` or R `read.table`; nirs4all-formats adds
axis detection and provenance. For the native `.spc`, candidate references such
as `pyfasma-spc` or Shimadzu's own export converter are noted but no
clearly-licensed fixture exists yet.

## Samples & validation

`samples/shimadzu/synthetic_uvprobe.txt` is golden-backed: 1 record, 200-point
`nm` axis (1100–2500 nm), `sample_s000` signal, `"Spectrum Data"` title. The
registry test also confirms that `.spc` is not claimed by extension alone.
