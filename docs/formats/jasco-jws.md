# JASCO JWS

> **Status:** Supported (scoped) · **Vendor:** JASCO · **Extensions:** `.jws`

JWS is the native binary format written by JASCO Spectra Manager for FT-IR,
UV-Vis, fluorescence and circular-dichroism instruments. A `.jws` file is an
OLE2 compound document; nirs4all-formats reads the reverse-engineered stream pair seen
in the committed fixtures and labels channels conservatively from the embedded
instrument metadata.

## Instruments & software

Produced by JASCO Spectra Manager across the FT/IR, V-series UV-Vis, FP-series
fluorescence and CD-1500 / J-1500 circular-dichroism lines. Committed fixtures
cover an FT/IR-4100, an FP-8300 and a CD-1500/J-1500 acquisition.

## File structure

An OLE2 compound document (magic `D0 CF 11 E0 …`) with named streams:

- `DataInfo` — channel count, point count and the spectral axis endpoints;
- `Y-Data` — float32 ordinate values for all channels;
- `BaseInfo` — original source path (when present);
- `ModuleInfo`, `SampleInfo`, `UserInfo`, `MeasParam` — instrument, sample and
  measurement hint strings used to choose semantic channel labels.

The axis unit is inferred from the endpoint range: `nm` (wavelength) when the
bounds sit within roughly 150-2500, otherwise `cm-1` (wavenumber).

## What nirs4all-formats extracts

- **Signals** — one `SpectralRecord` per file. Single-channel files emit one
  signal; multi-channel files emit one signal per channel. Channel semantics are
  inferred from the metadata hints:
  - FT/IR single-channel percent-scale spectra → `transmittance` (`%T`);
  - FP-series fluorescence → `fluorescence`;
  - CD-1500 / J-1500 → `cd` (`mdeg`), `ht` (`V`), `absorbance` (`dOD`);
  - otherwise `signal` or `channel_N`.
- **Axis** — generated from the `DataInfo` endpoints and point count; kind from
  the inferred unit.
- **Metadata** — channel count, channel labels, point count, source path,
  instrument model and module list, sample label, operator/organization,
  measurement parameters and the inferred measurement mode.
- **Provenance & warnings** — `jasco_jws_reverse_engineered_data_info`, plus
  `jasco_jws_semantic_channels_inferred` when a semantic mode was chosen, with
  source file and SHA-256.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| FT/IR transmittance (single channel) | Supported | Percent-scale ordinates typed as `transmittance`. |
| Fluorescence (FP-series) | Supported | Single `fluorescence` channel. |
| CD / HT / Abs (CD-1500 / J-1500) | Supported | Three labelled channels. |
| V-series NIR / NRS Raman variants | Planned | Distinct stream layouts; no fixtures yet. |
| `Data` / `Header` / `XdataValue` stream layouts | Planned | Described by other reverse-engineering projects; pending fixtures. |

## Limitations & known gaps

- Channel typing is conservative and metadata-driven; layouts whose hints are not
  specific enough fall back to generic `signal` / `channel_N` names.
- V-series NIR, NRS Raman and the alternative `Data` / `Header` / `XdataValue`
  stream layouts remain pending until fixtures are available.
- JASCO text exports are handled by the
  [row-oriented spectral table reader](row-spectral-table.md), not here.

## Reference readers

The reverse-engineering follows public JWS projects such as `jws2txt` and
`jwsProcessor`.

## Samples & validation

Fixtures under `samples/jasco/` are golden-backed with direct semantic tests:
`243.jws` (FT/IR-4100, `cm-1`, 7729 points, `transmittance`),
`sample_fluorescence.jws` (FP-8300, `nm`, 301 points, `fluorescence`) and
`sample_CD_HT_Abs.jws` (CD-1500/J-1500, `nm`, 1501 points, `cd` / `ht` /
`absorbance`). The probe requires both a `.jws` extension and the OLE2 header,
reporting `jasco-jws` at `Confidence::Likely`.
