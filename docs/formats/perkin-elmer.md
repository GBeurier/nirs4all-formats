# Perkin Elmer Spectrum / IR

Status: experimental.

The Perkin Elmer reader covers the `PEPE` block container used by Spectrum /
Spotlight `.sp` single-spectrum files. It reads the root block table, typed
little-endian payloads and the f64 ordinate array.

Extracted fields include:

- wavenumber axis limits, step and point count;
- signal min/max and f64 values;
- axis unit and signal unit;
- sample id, instrument, serial, software, detector, source, beam splitter,
  apodization and accessory metadata when present.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Notes |
|---|---:|---|---|---|
| `samples/perkin_elmer/spectra.sp` | 1 | wavenumber, `cm-1`, 3301 points | `absorbance`, unit `A` | Real `specio` fixture |

## Dispatch Boundaries

`.fsm` Spotlight imaging files share the `PEPE` family but are intentionally out
of scope for v1. The reader recognizes `.fsm` headers and returns a clear
unsupported-imaging error instead of interpreting an image cube as a 1D
spectrum.

The committed `.sp` fixture contains footer metadata whose scan range disagrees
with the typed axis blocks. The native reader treats the typed blocks as
canonical and leaves the footer text for future reverse-engineering work.
