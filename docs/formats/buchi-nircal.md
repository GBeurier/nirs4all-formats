# BUCHI NIRCal

Status: experimental.

The BUCHI/Buhler NIRCal reader covers project files that start with
`NIRCAL Project File`. The current implementation parses the section layout used
by the committed `prospectr::read_nircal()` fixture:

- sample identifiers from the `Spectra` selection block;
- wavenumber axis from the `Wavelength Selection` block;
- property names and per-sample property blocks from the `Properties` section;
- one double64 spectrum per sample from fixed-size `begin` / `end` blocks.

It emits one `SpectralRecord` per sample. The sample id is stored in metadata.
The committed fixture carries property names but its numeric property values are
all zero; those values are exposed as `null` targets to preserve the target
schema while matching `prospectr::read_nircal()` missing-value semantics.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Notes |
|---|---:|---|---|---|
| `samples/buchi_nircal/muestras-tejido-foliar_transfer.nir` | 20 | wavenumber, `cm-1`, 1501 points | `absorbance` | Real `prospectr` fixture; 20 property targets are present but null |

## Dispatch Boundaries

NIRCal `.nir` must be distinguished from Foss/WinISI `.NIR` by the header, not
by extension. This reader only accepts the `NIRCAL Project File` signature.

Non-zero reference properties still need an additional fixture. They will use
the same `targets` path already exercised by the committed zero-valued sample.
