# DigitalSurf SUR / PRO

> **Status:** Supported · **Vendor:** Digital Surf · **Extensions:** `.sur`, `.pro`

`.sur` and `.pro` are the Digital Surf MountainsMap container formats, used for
surface metrology, profilometry and AFM/Raman exports. nirs4all-io reads the
spectral and surface objects: spectra and hyperspectral maps become spectral
records, while height surfaces become spatial profile records. These files are
adjacent to the core NIRS point-spectrum scope.

## Instruments & software

Written by Digital Surf MountainsMap and instruments that export through it
(profilometers, AFM, branded AFM-Raman). The committed fixtures come from
RosettaSciIO.

## File structure

The reader sniffs the 12-byte signature `DIGITAL SURF` (uncompressed) or
`DSCOMPRESSED` (zlib) and reads a fixed 512-byte little-endian object header.
Multi-object files concatenate objects; the first header declares
`number_of_objects` and `p_size`. Decoded header fields include object type,
point size, X/Y dimensions, optional W size, axis names and units, offsets,
spacings, scaling parameters, comment size, private-zone size and compressed
payload size.

Compressed payloads are not RLE: a small directory (`stream_count`, then
per-stream raw/zlib lengths) precedes the zlib streams. Raw points are signed
16-bit or 32-bit integers; for spectral/profile payloads the decoded value is
`(raw_int - z_min) * (z_spacing / z_unit_ratio) + z_offset`.

## What nirs4all-io extracts

- **Signals** — `_SPECTRUM` objects emit one record (single) or one record per
  spectrum (multi); `_HYPCARD` hyperspectral maps emit one `SpectralRecord` per
  XY point; `_SURFACE` height maps emit one profile record per row.
- **Axis** — wavelength axes stored in `mm` by MountainsMap are normalised to
  `nm` in the `SpectralAxis`; the original DigitalSurf axis name and unit are
  preserved as `signal_axis_name` / `signal_axis_original_unit`. Surface rows use
  a spatial-index axis.
- **Metadata** — maps add `map_x_index`, `map_y_index`, dimensions and
  `map_axis_order = y_slowest_x_fastest`; surfaces add `spatial_y_index`, X/Y
  spatial units and `surface_axis_order = row_profiles_y_slowest_x_fastest`.
- **Provenance & warnings** — surface objects carry an explicit warning because
  their axis is spatial rather than spectral.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| `_SPECTRUM` single / multi-spectrum profile | Supported | Multi-spectrum carries line-position metadata. |
| `_HYPCARD` hyperspectral map | Supported | One record per XY point; uncompressed and zlib payloads. |
| `_SURFACE` height map | Supported | One spatial profile per row; flagged as non-spectral. |
| `DSCOMPRESSED` zlib payloads | Supported | Directory-plus-zlib-stream decoding. |

## Limitations & known gaps

- Object/comment metadata is decoded only at a basic level; richer object and
  comment fields are still pending.
- Automated full-array conformance against `rsciio.digitalsurf` is not yet wired
  up.
- The scope for MountainsMap variants outside this corpus, including branded
  AFM-Raman exports, is still undecided.

## Reference readers

Layout and fixture values cross-checked against `rsciio.digitalsurf` 0.13.0.
RosettaSciIO is GPL-3.0; it is used only as an external conformance reference and
is never imported or linked by the MIT runtime.

## Samples & validation

The RosettaSciIO fixtures under `samples/digitalsurf/` are fully golden-backed:
a single spectrum and a 65-spectrum profile (`.pro`), a 12-by-10 hyperspectral
map and its zlib-compressed counterpart (120 records each, `.sur`), and a
128-row height surface exported as spatial profiles. No blocking samples are
known; the open items are conformance and scope decisions.
