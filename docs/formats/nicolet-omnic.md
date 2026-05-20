# Thermo Nicolet OMNIC

Status: experimental.

The OMNIC reader covers the reverse-engineered key-table layout used by legacy
Thermo Nicolet spectral data files:

- `.SPA` single-spectrum files;
- `.SPG` grouped spectra, emitted as one `SpectralRecord` per sub-spectrum.

The reader extracts the spectral header (`nx`, axis unit, signal unit, first and
last x values), the float32 intensity block and the OMNIC title/timestamp fields
when present. Wavenumber axes are emitted in native descending `cm-1` order.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Notes |
|---|---:|---|---|---|
| `samples/nicolet_omnic/2-BaSO4_0.SPA` | 1 | wavenumber, `cm-1`, 11098 points | `absorbance` | Real SpectroChemPy fixture |
| `samples/nicolet_omnic/not_opus.spa` | 1 | wavenumber, `cm-1`, 5549 points | `absorbance` | Regression fixture for `.spa` vs Bruker OPUS disambiguation |
| `samples/nicolet_omnic/wodger.spg` | 2 | wavenumber, `cm-1`, 5549 points | `absorbance` | SpectroChemPy documentation fixture |

## Dispatch Boundaries

`.srs` time-series files are sniffed as `nicolet-omnic-srs` but are not decoded
yet. Their series variants need separate validation because even SpectroChemPy
documents hard-case `.srs` files. The native reader therefore refuses `.srs`
reads until the series data offsets and time axis are covered by fixtures.

The implementation follows the same public reverse-engineering model used by
SpectroChemPy: key `02` points to the spectral header, key `03` points to the
float32 intensity block and key `6B` carries group spectrum titles and OMNIC
timestamps.
