# SiWare API JSON / CSV

Status: experimental.

The SiWare API reader covers one-measurement JSON payloads with
`measurement.wavelengths` and `measurement.absorbance` arrays. It is intended
for cloud/API exports from Spectro Inc. NeoSpectra-style workflows. The
companion CSV stream is covered through the generic axis-first
`row-spectral-table` reader.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Targets |
|---|---:|---|---|---|
| `samples/siware_api/synthetic_siware_api.json` | 1 | wavelength, `nm`, 200 points | `absorbance` | `protein`, `moisture` |
| `samples/siware_api/synthetic_siware_api.csv` | 1 | wavelength, `nm`, 200 points | `absorbance` | none |

Instrument vendor/model/serial, measurement id, timestamp, operator and simple
environmental metadata are preserved as record metadata.

## Dispatch Boundaries

The CSV stream is parsed by `row-spectral-table` because it is an axis-first
text export. Comment metadata is preserved in `metadata.notes`. The JSON reader
only claims files with the SiWare-style `measurement.wavelengths` and
`measurement.absorbance` fields.

## Remaining Gaps

Both fixtures are synthetic. To move the row to `fait`, we still need a real
credentialed API response, schema drift examples and a reference comparison for
unit labels, predictions and optional metadata fields.
