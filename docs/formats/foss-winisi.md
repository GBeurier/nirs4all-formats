# Foss / WinISI Exports

> **Status:** Supported (scoped) · **Vendor:** Foss (NIRSystems) · **Extensions:** `.txt`, `.csv` (exports); native `.NIR`, `.DA`, `.cal`, `.eqa` (blocked)

Foss NIRSystems / WinISI / ISIscan is a long-standing industrial NIR platform.
Its native project files are vendor-closed with no reliable open reader, so
nirs4all-formats reads the exported text or CSV spectral matrix instead: `Wavelengths:`
text blocks through the [spectral-matrix reader](spectral-matrix.md) and wide CSV
exports through the [delimited-text reader](text-readers-001.md). The native
binary formats are not decoded.

## Instruments & software

Produced by WinISI / ISIscan for Foss NIRSystems instruments (e.g. XDS, NIRSystems
5000) when exporting calibration or sample data. Committed fixtures include a
synthetic WinISI-style matrix export and two real Foss XDS CSV exports from a
University of Cordoba (sensAIfood) dataset.

## File structure

- **`Wavelengths:` text export** — a labelled wavelength block followed by
  one-spectrum-per-row data; read as a spectral matrix.
- **Wide CSV export** — leading metadata / target columns (`ID`, properties)
  followed by numeric wavelength headers, one sample per row; read as delimited
  text. The delimiter is auto-detected.

## What nirs4all-formats extracts

- **Signal** — one spectral signal per sample row, axis in `nm`.
- **Metadata** — `ID` is promoted to `metadata.sample_id`.
- **Targets** — numeric property columns (e.g. `Moisture`, `Protein`, `Year`)
  become per-record targets.
- **Provenance** — source file + SHA-256, reader name and version.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| `Wavelengths:` text matrix export | Supported | Read via the spectral-matrix reader. |
| Wide CSV export (metadata/targets + numeric headers) | Supported | Read via the delimited-text reader. |
| Native `.NIR` / `.DA` / `.cal` / `.eqa` | Blocked | Closed formats; no reliable open reader or redistributable binary fixture. |
| DS3 / Inframatic property-only reports | Detected / refused | No numeric spectral headers, so explicitly refused. |

## Limitations & known gaps

- The native `.NIR` / `.DA` / `.cal` / `.eqa` formats are not reverse-engineered
  (no public binary fixture has been found), so calibration / equation payloads
  are not extracted.
- The export path does not replace the native Foss reader; it only covers the
  text / CSV interchange.
- No comparison against a vendor or community native binary reader exists,
  because none is currently available.

## Reference readers

The text and CSV exports are equally readable with `pandas` or R `read.table`;
nirs4all-formats adds axis detection, target promotion and provenance. No open native
reader is available for the binary project formats.

## Samples & validation

Fixtures under `samples/foss_winisi/` are golden-backed / read-tested:
`synthetic_winisi_export.txt` (50 records, 200-point `nm` axis, `protein`
target), `foss_xds_barleyground_sensAIfood.csv` (7 records, 1050 points,
400–2498 nm) and `foss_xds_wheat2_sensAIfood.csv` (2 records, same wide layout).
DS3 / Inframatic-style property-only reports are kept as expected refusals.
