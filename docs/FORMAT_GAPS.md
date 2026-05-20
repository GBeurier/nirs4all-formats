# Untreated, Blocked and Unknown Formats

This page documents formats that are **not decoded natively yet**, even when
they are listed in [`FORMATS.md`](FORMATS.md). It complements
`MISSING_SAMPLES.md`, which is the exact fixture inventory.

## Policy

`nirs4all-io` does not promote a native loader without at least one of:

- a redistributable real fixture;
- a private fixture that can be used in CI-like validation by the owner;
- a public specification precise enough to build adversarial tests;
- a reference reader plus a sample/export pair that proves the decoded values.

When none of those exist, the correct behavior is detection plus explicit
refusal, or a documented text/CSV export path. Extension-only routing is not
accepted for collision-prone families such as `.spc`, `.nir`, `.dat`, `.txt`,
`.spa` and `.srs`.

## No Sample At All

| Format | Current behavior | Why blocked | What unlocks it |
|---|---|---|---|
| Allotrope ADF `.adf` | Not implemented; not a v1 target. | Membership-gated HDF5/RDF stack; no public fixture. | A redistributable ADF or an Allotrope-member validation fixture plus schema notes. |

## Native Binary Blocked By Missing Fixtures

These are real spectroscopy formats, but native decoding is blocked because no
small redistributable binary fixture is available or the format is too
undocumented to test safely.

| Format | Current behavior | Why blocked | Preferred interim path |
|---|---|---|---|
| ASD `.ILL` / `.REF` / `.RAW` calibration companions | Not decoded. `.asd` primary spectra are supported. | Vendor/SDK-distributed companions; no open fixture. | Use `.asd` primary spectra; add calibration decoding only with a complete `.asd + .ILL/.REF/.RAW` set. |
| Foss NIRSystems / WinISI `.NIR`, `.DA`, `.cal`, `.eqa` | Native binary not decoded. WinISI text matrix exports are supported. | Closed binary format; no reliable open reader. | Export WinISI/DS/Vision data to text or CSV. |
| Perten DA / Inframatic native binaries | Not decoded. Target-only CSV reports are intentionally refused as spectra. | Customer-only binary files; no spectral-axis fixture. | Export spectral CSV/Excel from vendor software. |
| Metrohm Vision / Vision Air project database | Native project DB not decoded. Vision Air CSV spectral matrices are supported. | Closed project format; public workflows use CSV/Excel export. | Export spectral matrices or lab templates. |
| Shimadzu UVProbe proprietary `.spc` | Native binary not decoded; Shimadzu text export is covered. | Same extension as Galactic SPC but different magic/layout; no real fixture. | Use UVProbe TXT export until a real `.spc` is contributed. |
| VIAVI MicroNIR `.pri` project container | Not decoded; MicroNIR CSV matrices are supported. | Customer-only project format. | Export CSV. |
| WiTec `.wip` / `.wid` variants outside `WIT_PR06` TDGraph | The committed `Sa4.wip` layout decodes experimentally; signed `WIT^` and unknown `WIT_PR06` layouts are refused. WiTec ASCII export is supported. | Only one redistributable binary map layout is available. | Export from WiTec Project/FIVE as ASCII text; broaden native decode only with matching `.wip` plus reference export. |
| Horiba LabSpec `.l6s` / `.l6m` binary variants | One LabSpec6 `.l6m` Gd2O3/AlN map layout decodes experimentally; `.l6s` and other binary layouts are not decoded. LabSpec XML/text exports are supported. | Only one open `.l6m` map fixture is available. | Export XML/text spectrum, line scan or map; broaden native decode with matching binary and text/reference export. |

## Export Path Exists But Real Samples Are Missing

These are not blocked architecturally: the text/CSV/JSON shape is implemented
or synthetic-tested, but a real customer export is still needed before the
format can be promoted.

| Format/export | Current behavior | Missing evidence |
|---|---|---|
| NeoSpectra Scanner CSV | Synthetic NeoSpectra-style CSV is parsed by row/table readers. | Real customer export with instrument metadata. |
| SiWare / Spectro Inc. API JSON or CSV stream | Synthetic one-measurement JSON is parsed. | Real credentialed API response. |
| Solar Light MFR `.OUT` and Microtops legacy `.TXT` | Synthetic channel exports are parsed by `sun_photometer`; committed Microtops MAN NetCDF is parsed through a guarded fixture path. | Real legacy Microtops `.TXT` export and a generic MAN NetCDF/HDF5 metadata path. |
| PP Systems UniSpec `.SPT` / `.SPU` | Synthetic SC/DC axis-first exports are parsed. | Real field acquisition. |
| MODTRAN albedo `.dat` | Synthetic albedo table is parsed. | Redistributable licensed MODTRAN output. |
| FGI HDF5 + XML pairing | Synthetic nested HDF5 payload and XML sidecar are parsed; real schema coverage is unknown. | Real FGI HDF5/XML pair. |
| Shimadzu UVProbe `.txt` | Synthetic UVProbe text export is parsed. | Real customer text export. |
| JASCO Raman `.txt` | JASCO text export path is parsed. | Real NRS-series Raman text export. |

## Partial Real Coverage

These families already have real fixtures and native readers, but important
variants remain untreated until a sample or reference comparison is available.

| Format family | Implemented subset | Untreated variants |
|---|---|---|
| ASD `.asd` | Revisions 1, 6, 7 and 8 primary spectra. | Legacy v3/v4/v5 and calibration companion workflows. |
| Bruker OPUS | Modern OPUS native 1D spectral/status blocks and DPT exports. | OPUS 5/6 archives, Tango-specific metadata blocks, additional 2D/imaging blocks. |
| Galactic / Thermo SPC | New little-endian generated-X, explicit-X, common-X multi and `-XYXY`; old LSB limited. | New big-endian `0x4C`, more old-header layouts, real multi-channel instrument fixtures. |
| Thermo Nicolet OMNIC | `.spa`, `.spg`, TGA/GC `.srs` time-series matrices. | `.srsx`, rapid-scan/high-speed `.srs`, additional OMNIC release layouts. |
| Perkin Elmer | `.sp` single spectra. | `.fsm` Spotlight imaging is intentionally out of v1; PE Lambda NIR-specific variants need fixtures. |
| BUCHI NIRCal | One `.nir` transfer file with spectra/wavenumbers/property schema. | Non-zero property targets, `.cal` calibration-only files, NIRMaster variants. |
| Avantes AvaSoft | Legacy `.TRM/.ROH/.DRK/.REF/.ABS`, AvaSoft 8 `.Raw8/.IRR8`, ASCII exports. | Legacy `.IRR/.RMN`, AvaSoft 8 `.RWD8/.ABS8/.TRM8/.RFL8/.RIR8/.RMN8/.RMD8` fixtures. |
| Ocean Optics / Ocean Insight | SpectraSuite/OceanView/Jaz/CRAIC text, `.ProcSpec`, Ocean Optics-flavoured SPC. | QE Pro, Maya and Apex firmware-specific exports. |
| ENVI / hyperspectral | ENVI SLI including USGS splib06a/splib07, ENVI Standard `.img/.dat + .hdr` cubes, AVIRIS 92AV3C ERDAS `.lan/.spc/.GIS`, and the local-only Indian Pines MATLAB v5 cube expanded to one spectrum per pixel. | Generic ERDAS LAN, NEON/Specim/HySpex/Headwall/HDF5 cubes and mask/ROI extraction workflows. |
| Consumer Physics SCiO CSV | Developer-app `band*`, grouped spectrum/raw CSV and axis-first calibration CSV fixtures. | Native/mobile project containers and additional firmware exports. |
| JASCO JWS | FT/IR, fluorescence and CD/HT/Abs OLE2 payloads. | NIR-specific V-780 blocks and NRS-series Raman binary flavor. |
| Renishaw WDF | Spectra, maps/lines/depth/time metadata, WHTL JPEG metadata, MAP inventory. | Full derived `MAP ` dataRange decoding and per-model fixtures. |

## Refused By Design

These are recognized or listed to avoid false positives, but they are not NIRS
point-spectroscopy loaders.

| Format | Current behavior | Reason |
|---|---|---|
| mzML / mzMLb | mzML XML is detected and refused with MS-library guidance. | Mass spectrometry container, not NIRS molecular spectroscopy. |
| ANDI/MS NetCDF `.cdf` | Detected from standard chromatography/MS variables and refused. | Chromatography/MS standard, not NIRS. |
| Hyperspectral image cubes outside ENVI Standard sidecars | Refused or unsupported until a schema-specific reader exists. | Primary payload is an image cube; large ROI/mask workflows need an explicit extraction API. |
| Perkin Elmer `.fsm` | Detected/refused as imaging. | Spotlight imaging format; large fixtures and different data model. |
| fNIRS neuroscience files (`SNIRF`, NIRx `.nirs/.wl1/.wl2/.hdr`) | Out of scope. | Physiological time-series domain; use MNE-NIRS/SNIRF tools. |
| Non-spectral HDF5/NetCDF/AnIML | Schema-refused. | Standard container does not imply NIRS spectra. |

## Unknown Or Unsafe To Claim

Some vendor names and extensions appear in user data, manuals or issue reports
without enough evidence to write even a refusal sniffer. They stay in the
inventory as unknown until we have a signature, fixture or documented export.

| Family | Risk | Required evidence |
|---|---|---|
| Unknown `.spc` files | Extension collision across Galactic, Ocean Optics, Shimadzu and other vendors. | Magic/header bytes and one real file. |
| Unknown `.nir` files | Collision between BUCHI NIRCal, Foss NIRSystems and vendor exports. | Header signature plus expected payload type. |
| Unknown `.dat` files | Collision between MODTRAN text, ENVI raw cubes and arbitrary tables. | Sidecar/header or axis-bearing text evidence. |
| Unknown `.txt/.csv` exports | Could be spectral table, target report, metadata-only report or unrelated lab output. | Axis column, numeric spectral headers, or vendor metadata identifying spectra. |
| Unknown `.srs/.srsx` files | OMNIC series layouts vary by release and acquisition mode. | Header plus at least one decoded reference from OMNIC/SpectroChemPy/vendor export. |

## Documentation Rule For New Gaps

When a new unsupported format is discovered, update:

1. `MISSING_SAMPLES.md` with exact sample/version evidence;
2. this page with the reason it is untreated and the intended current behavior;
3. the relevant `docs/formats/*.md` page if a detector, refusal path or export
   parser exists;
4. `docs/STATUS.md` only after the behavior is implemented and tested.
