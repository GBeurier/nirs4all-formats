# BUCHI NIRCal

Status: experimental.

The BUCHI/Buhler NIRCal reader covers project files that start with
`NIRCAL Project File`. The current implementation parses the section layout used
by the committed `prospectr::read_nircal()` fixture:

- sample identifiers from the `Spectra` selection block;
- wavenumber axis from the `Wavelength Selection` block;
- one double64 spectrum per sample from fixed-size `begin` / `end` blocks.

It emits one `SpectralRecord` per sample. The sample id is stored in metadata.
The committed fixture carries property names but its numeric property values are
all zero, so no targets are emitted yet.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Notes |
|---|---:|---|---|---|
| `samples/buchi_nircal/muestras-tejido-foliar_transfer.nir` | 20 | wavenumber, `cm-1`, 1501 points | `absorbance` | Real `prospectr` fixture |

## Dispatch Boundaries

NIRCal `.nir` must be distinguished from Foss/WinISI `.NIR` by the header, not
by extension. This reader only accepts the `NIRCAL Project File` signature.

Reference-property extraction remains pending until a fixture with non-zero
properties is available. When implemented, those values must become
`targets`, not metadata.
