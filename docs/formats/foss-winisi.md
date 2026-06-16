# Foss / WinISI

> **Status:** Supported (scoped) · **Vendor:** Foss (NIRSystems) · **Extensions:** native `.cal`, `.nir`; `.txt`, `.csv` (exports); native `.DA`, `.eqa` (not yet sampled)

Foss NIRSystems / WinISI / ISIscan is a long-standing industrial NIR platform.
nirs4all-formats now decodes the native `.cal` / `.nir` binaries directly through a
reverse-engineered reader, and still reads the exported text or CSV spectral
matrix where only an export is available: `Wavelengths:` text blocks through the
[spectral-matrix reader](spectral-matrix.md) and wide CSV exports through the
[delimited-text reader](text-readers-001.md). The remaining native project
formats (`.DA`, `.eqa`) are not yet sampled.

## Native `.cal` / `.nir` reader

The native reader decodes the WinISI / ISIscan binary container directly. It is
routed **by header signature, never by extension**: a file is claimed only when
its binary header carries the version word followed by an ASCII `ISIscan` /
`NIRSystems` marker. This is what resolves the `.nir` extension collision with
BUCHI NIRCal — WinISI never fires on a BUCHI file and BUCHI never fires on a
WinISI file, so each keeps its own `.nir` payloads.

Three probes are exposed, all at `Definite` confidence:

- **`foss-winisi-cal`** — calibration `.cal` files (file-type word `2`) carrying
  per-sample spectra **and** constituent reference values.
- **`foss-winisi-nir`** — spectra-only `.nir` files (file-type word `1`), no
  constituents.
- **`foss-ds-nir`** — the same container emitted by the newer FOSS **DS-series**
  benches (DS2500, DS3 F). These carry no `ISIscan`/`NIRSystems` string; they are
  pinned instead by a `NIRS DS...` instrument model at offset `0x82`. Spectra-only
  (file-type word `1`), decoded identically to `foss-winisi-nir`.

Each sample becomes one `SpectralRecord`:

- **Signal** — an absorbance spectrum (`f32`), axis `Wavelength` in `nm` built
  from the file's segment table of `f32` start/step/end values.
- **Targets** — the constituent reference values (e.g. `CDpcMS`, `NDpcMS`, `K`,
  `P`), with constituent names normalised to lower-case target keys.
- **Metadata** — `sample_number`, `product_code`, `acquired_unix_time`,
  `acquired_utc`, `instrument_model`, `master_number`, `constituent_count`,
  `file_kind`, `version_word`, `record_index`.
- **Provenance** — source file + SHA-256, reader name and version, plus the
  `foss_winisi_reverse_engineered_blocks` warning.

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
| Native `.cal` (calibration) | Supported (native) | `foss-winisi-cal` probe; spectra + constituent targets. Sniffed by header signature. |
| Native `.nir` (spectra-only) | Supported (native) | `foss-winisi-nir` probe; spectra, no constituents. Sniffed by header signature. |
| Native DS-series `.nir` (DS2500, DS3 F) | Supported (native) | `foss-ds-nir` probe; same container, pinned by the `NIRS DS` model at `0x82`, no `ISIscan`/`NIRSystems` string. Spectra-only. |
| `Wavelengths:` text matrix export | Supported | Read via the spectral-matrix reader. |
| Wide CSV export (metadata/targets + numeric headers) | Supported | Read via the delimited-text reader. |
| Native `.DA` / `.eqa` | Not yet sampled | No binary fixture found yet; not reverse-engineered. |
| DS3 / Inframatic property-only reports | Detected / refused | No numeric spectral headers, so explicitly refused. |

## Limitations & known gaps

- The native `.cal` / `.nir` reader is reverse-engineered (hence the
  `foss_winisi_reverse_engineered_blocks` warning); the wavelength axis is
  reconstructed from the file's segment table of `f32` start/step/end values.
- Native sample-block strides are detected from record headers. Most 1050-point
  files use the compact 24-byte post-spectrum gap, while DS3 F 700-point files
  use the 128-byte-aligned variant.
- The real native vendor binaries are **local-only** under
  `samples_local/foss_winisi/` (private, licence TBD); only the synthetic
  `synthetic.cal` fixture is redistributable.
- The remaining native `.DA` / `.eqa` formats are not yet sampled or
  reverse-engineered, so calibration-equation payloads are not extracted.
- The export path does not replace the native reader; it only covers the
  text / CSV interchange where no native binary is available.

## Reference readers

The text and CSV exports are equally readable with `pandas` or R `read.table`;
nirs4all-formats adds axis detection, target promotion and provenance. No open native
reader is available for the binary project formats, so the native `.cal` / `.nir`
reader is validated against the ISIscan `.txt` exports of the same files rather
than against a third-party binary loader.

## Samples & validation

Fixtures under `samples/foss_winisi/` are golden-backed / read-tested:
`synthetic.cal` (native reader; 2 samples, 6-point `nm` axis 1000–1014 nm,
`moisture` / `protein` constituent targets), `synthetic_winisi_export.txt`
(50 records, 200-point `nm` axis, `protein` target),
`foss_xds_barleyground_sensAIfood.csv` (7 records, 1050 points, 400–2498 nm) and
`foss_xds_wheat2_sensAIfood.csv` (2 records, same wide layout). DS3 / Inframatic-style
property-only reports are kept as expected refusals.

The DS-series native path has its own golden-backed fixtures under
`samples/foss_winisi/`: `synthetic_ds2500.nir` (`foss-ds-nir`; 4 samples,
two-segment 100-point `nm` axis, `NIRS DS2500` model) and `synthetic_ds3f.nir`
(3 samples, single-segment 50-point `nm` axis, `NIRS DS3 F` model).

The real native corpus is **local-only** under `samples_local/foss_winisi/`
(private, licence TBD). The ISIscan/NIRSystems files are validated against their
`.txt` exports: `yamtot_2026.cal` (18 records, axis 400–2498 nm / 1050 pts,
spectra matching to `float32` precision ~5e-8, constituents exact),
`ando4_2026.cal` (20 records / 9 constituents), `D2026_20240328.cal` (18 / 5) and
`yamtot_2026.nir` (16 / 0 constituents). The DS-series files have no vendor text
export, so `fileDS2500CRAW.nir` (10 records, 400–2498 nm / 1050 pts) and
`fileDS3FCRAW.nir` (20 records, 1100–2498 nm / 700 pts) are checked structurally
(record count, axis endpoints, instrument model, finite plausible absorbance).
