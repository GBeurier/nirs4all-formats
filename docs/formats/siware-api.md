# SiWare API JSON

Status: experimental.

The SiWare API reader covers one-measurement JSON payloads with
`measurement.wavelengths` and `measurement.absorbance` arrays. It is intended
for cloud/API exports from Spectro Inc. NeoSpectra-style workflows.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Targets |
|---|---:|---|---|---|
| `samples/siware_api/synthetic_siware_api.json` | 1 | wavelength, `nm`, 200 points | `absorbance` | `protein`, `moisture` |

Instrument vendor/model/serial, measurement id, timestamp, operator and simple
environmental metadata are preserved as record metadata.

## Dispatch Boundaries

The companion CSV stream is parsed by `row-spectral-table` because it is an
axis-first text export. The JSON reader only claims files with the SiWare-style
`measurement.wavelengths` and `measurement.absorbance` fields.
