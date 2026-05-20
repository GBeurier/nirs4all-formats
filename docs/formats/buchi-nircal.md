# BUCHI NIRCal

Status: experimental.

The BUCHI/Buhler NIRCal reader covers project files that start with
`NIRCAL Project File`. The current implementation parses the section layout used
by the committed `prospectr::read_nircal()` fixture:

- sample identifiers from the `Spectra` selection block;
- wavenumber axis from the `Wavelength Selection` block;
- property names and per-sample property blocks from the `Properties` section;
- per-spectrum NIRCal metadata from the `Spectra Info` block: spectrum GUID,
  timestamps, creator/modifier, scans, resolution, declared wavenumber geometry,
  device, instrument serials, measurement cell, option serial and optional
  gain/temperature diagnostics;
- one double64 spectrum per sample from fixed-size `begin` / `end` blocks.

It emits one `SpectralRecord` per sample. The sample id is stored in metadata.
Project GUID, project-file version and per-sample replicate counters are also
promoted when present, so repeated sample identifiers remain distinguishable in
dataset exports. Spectrum-info fields are promoted as flat snake_case metadata
using names aligned with the `prospectr::read_nircal()` layout where practical;
empty `0/` fields are omitted instead of being emitted as blank strings.
The committed fixture carries property names but its numeric property values are
all zero; those values are exposed as `null` targets to preserve the target
schema while matching `prospectr::read_nircal()` missing-value semantics. A
local-only cannabis transfer file exercises the same target path with non-null
`CBDA` and `THCA` values. In mixed/non-empty property tables, numeric `0.0`
targets are preserved as real values instead of being collapsed to `null`.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Notes |
|---|---:|---|---|---|
| `samples/buchi_nircal/muestras-tejido-foliar_transfer.nir` | 20 | wavenumber, `cm-1`, 1501 points | `absorbance` | Real `prospectr` fixture; 20 property targets are present but null; validates spectrum GUID, device/serial, scans/resolution and gain/temperature metadata |
| `samples_local/buchi_nircal/transpec_DEMO_cannabis.nir` | 105 | wavenumber, `cm-1`, 1501 points | `absorbance` | Local-only transfer fixture; non-null `CBDA` and `THCA` targets plus 3 replicate spectra per sample validate the target, replicate, comment/timestamp and device/serial metadata paths |

## Dispatch Boundaries

NIRCal `.nir` must be distinguished from Foss/WinISI `.NIR` by the header, not
by extension. This reader only accepts the `NIRCAL Project File` signature.

Non-zero reference properties are validated locally, but still need a small
redistributable fixture before the target path can be claimed publicly.
Calibration-only `.cal` files, JCAMP-DX exports and broader NIRMaster/NIRFlex
firmware variants are still untreated. The `Spectra Info` parser follows the
observed NIRCal 2.23 token layout, so new firmware fixtures should be compared
against vendor exports or `prospectr` before widening the compatibility claim.
