# Allotrope ASM JSON

Status: experimental.

The Allotrope reader covers the JSON Simple Model path, not ADF. Current
support is focused on plate-reader ASM documents with spectral data cubes or
single-wavelength endpoint readings.

## Supported Fixtures

| Fixture | Records | Axis | Signal |
|---|---:|---|---|
| `samples/allotrope_asm/ACSINS_absorbance_spectrum.json` | 360 | wavelength, `nm`, 51 points | `absorbance`, `mAU` |
| `samples/allotrope_asm/spectrum_emission_data.json` | 1 | wavelength, `nm`, 3 points | `absorbance`, `mAU` |
| `samples/allotrope_asm/MD_SMP_absorbance_example.json` | 192 | detector wavelength, `nm`, one point | `absorbance`, `mAU` |

The reader emits one `SpectralRecord` per `measurement document`. Sample
identifier, well/location, detection type, measurement time, manifest URL,
converter metadata and selected device-control settings are preserved as
metadata.

## Dispatch Boundaries

This is a narrow ASM bridge for committed spectroscopy-like plate-reader
fixtures. It does not attempt full ASM schema coverage, ADF/HDF5 parsing or
vendor conversion. When original vendor files must be converted to ASM first,
the reference tool remains `Benchling-Open-Source/allotropy`.
