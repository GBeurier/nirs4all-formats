# Shimadzu UVProbe

Status: experimental partial.

The supported v1 path is Shimadzu UVProbe text export. Native Shimadzu `.spc`
is a proprietary container that shares an extension with Galactic/Thermo GRAMS
SPC but does not share the same binary layout, so dispatch must never be based
on `.spc` alone.

## Supported Fixtures

| Fixture | Records | Axis | Signals | Notes |
|---|---:|---|---|---|
| `samples/shimadzu/synthetic_uvprobe.txt` | 1 | 200 wavelengths, `1100..2500 nm` | `sample_s000` | Synthetic quoted CSV-style UVProbe export |

The signal type remains `unknown` because the export header only identifies a
sample column, not absorbance, transmittance or reflectance. The `"Spectrum
Data"` title row is preserved as a note.

## Dispatch Boundaries

The row-oriented spectral table reader accepts UVProbe text exports when it can
see a wavelength column and numeric sample data. For native `.spc` files,
`nirs4all-io` only reports an extension-level candidate unless the binary
matches a known Galactic/Thermo SPC header.

## Remaining Gaps

- real redistributable Shimadzu UVProbe text export;
- native Shimadzu `.spc` fixture with a clear redistribution license;
- comparison against `pyfasma-spc` or Shimadzu's own export for native `.spc`;
- typed signal role detection once real UVProbe exports expose measurement mode
  metadata.
