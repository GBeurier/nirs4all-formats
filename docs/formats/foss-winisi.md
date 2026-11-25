# Foss / WinISI

> **Status:** Supported (scoped) · **Vendor:** Foss (NIRSystems) · **Extensions:** native `.cal`, `.nir`; `.txt`, `.csv` (exports); legacy `.DA` through the separate NSAS reader; `.eqa` unsampled

Foss NIRSystems / WinISI / ISIscan is a long-standing industrial NIR platform.
nirs4all-formats now decodes the native `.cal` / `.nir` binaries directly through a
reverse-engineered reader, and still reads the exported text or CSV spectral
matrix where only an export is available: `Wavelengths:` text blocks through the
[spectral-matrix reader](spectral-matrix.md) and wide CSV exports through the
[delimited-text reader](text-readers-001.md). The remaining native project
format `.eqa` is not yet sampled. The observed legacy Rapitec/NIRSystems 6500
`.DA` layout is decoded by the separate [FOSS NSAS DA reader](foss-nsas-da.md).

## Native `.cal` / `.nir` reader

The native reader decodes the WinISI / ISIscan binary container directly. It is
routed **by header signature, never by extension**: a file is claimed only when
its binary header carries the version word followed by an ASCII `ISIscan` /
`NIRSystems` marker. This is what resolves the `.nir` extension collision with
BUCHI NIRCal — WinISI never fires on a BUCHI file and BUCHI never fires on a
WinISI file, so each keeps its own `.nir` payloads.

WinISI writes neither marker for spectra of other instruments (model `Unknown`).
Those files are claimed with `Likely` confidence when the whole header is
consistent: file date, equal spacing mode, segment table and point count.

Three more specific probes are exposed at `Definite` confidence:

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

### Header and sample layout

Little-endian. The meaning of most fields was confirmed by `wai.spectralio` (MIT,
University of Waikato), by the ISIscan text exports and by static reading of the
Vision importer.

| Offset | Type | Field |
|---|---|---|
| `0x00` | u16 | 1 = spectra, 2 = spectra and constituents |
| `0x02`, `0x04` | u16 | live and deleted samples |
| `0x06`, `0x08` | u16 | points, constituents |
| `0x0a`, `0x0c` | u16, u32 | DOS date and Unix time of the file |
| `0x12` | 71 chars | file ID |
| `0x59` | 9 chars | master serial number |
| `0x62` | u16 | moisture basis: 0 dry matter, 1 as received (confirmed by text exports) |
| `0x64` | nine u16 | math treatment; Vision refuses non-zero values |
| `0x80` | u16 | instrument type: 9 = 6500, 11 = 5000 and "Unknown", 29 = DS |
| `0x82`, `0x97` | 21, 9 chars | model, serial number |
| `0xa0`, `0xa2` | u16, u16[20] | segments, points per segment |
| `0xca` | u16 | spacing mode, 1 = equal |
| `0xcc`, `0xe8`, `0x104` | f32[7] | segment starts, steps, ends |
| `0x180` | 32 x 16 chars | constituent names |
| `0x380` | blocks | samples |

Each sample block holds a 256-byte header, the spectrum, zero padding to a multiple
of 128 bytes and 32 f32 constituents, where 0 means missing. The sample header holds
the sample number (13 chars), sequence (u16 at 13), deleted flag (u8 at 15), DOS date
(u16 at 16), product code (u16 at 18), client (9 chars at 20), three sample IDs
(50, 50 and 51 chars at 29), operator (32 chars at 180) and Unix time (u32 at 214).
After the last block, one 16-byte entry per block repeats sample number, sequence
and deleted flag. Deleted samples are not read.

The reader still reports constituents of 0 as they are stored.

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
| Legacy NSAS / Rapitec `.DA` | Experimental, observed layout | Separate `foss-nsas-da` reader, 6500/1050-point records, validated against paired SPC. |
| Native `.eqa`, other `.DA` variants | Not yet sampled | Equation files and broader legacy variants remain unverified. |
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
- Calibration-equation payloads are not extracted. The separate NSAS `.DA`
  reader recovers the observed legacy spectra; `.eqa` and other DA variants
  remain unverified.
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
