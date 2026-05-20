# Roadmap

`nirs4all-io` is developed in gated phases. Each phase must leave tests,
documentation and review notes behind.

## Phase 0: Foundation

Status: done.

Deliverables:

- Rust workspace with core model, registry, CLI and C ABI scaffold.
- Python and R binding skeletons.
- Reverse-engineering lab.
- CI for Rust, Python helper packages and docs.
- Read the Docs configuration.
- GitHub repository with initial commit.

Gate:

```bash
. "$HOME/.cargo/env"
cargo test --workspace
python -m pip install -e tools/reverse-lab -e bindings/python
python -m pytest tools/reverse-lab/tests bindings/python/tests
```

## Phase 1: Reader Contract and Simple Text Readers

Status: in progress. The registry now dispatches real native readers and the
first simple text readers are covered by sample tests.

Deliverables:

- strict `Reader` trait and registry behavior; done;
- bounded readers and archive policy;
- delimited text reader; experimental;
- row-oriented axis-first spectral table reader; experimental for Si-Ware,
  MODTRAN, PP Systems, ENVI/ECOSTRESS text, Shimadzu text, USGS SPECPR ASCII
  and WiTec ASCII fixtures; comment-prefixed IDL/ENVI headers and JASCO
  `XYDATA` text exports are also covered;
- spectral matrix reader; experimental for Foss/WinISI, Metrohm Vision Air and
  VIAVI MicroNIR exports;
- sun photometer reader; experimental for MFR and Microtops text exports;
- AnIML spectral XML reader; experimental for spectral `SeriesSet` fixtures
  with explicit values or uniform `AutoIncrementedValueSet` axes, plus refusal
  of non-spectral AnIML results;
- Allotrope ASM JSON bridge; experimental for plate-reader spectral data cubes
  and detector-wavelength endpoint readings;
- Allotrope ADF reader; experimental local-only subset for numeric HDF5
  `/data-cubes`, with minimal RDF component mapping but without full
  ontology/SDK validation yet;
- SiWare API JSON reader and CSV companion coverage; experimental for
  one-measurement NeoSpectra-style JSON payloads and axis-first CSV streams;
- NetCDF NIRS reader; experimental for simple `spectra` + `wavelengths`
  datasets, dedicated ANDI/MS refusal and schema refusal of adjacent non-NIRS
  NetCDF containers;
- generic HDF5 NIRS reader; experimental for root and nested-group `spectra` +
  `wavelengths` datasets, including the committed synthetic FGI HDF5 payload;
- MATLAB MAT reader; experimental for simple MAT v5 and MATLAB v7.3/HDF5
  `X` + `wavelengths` + optional `y` datasets, plus committed Eigenvector
  Corn, Eigenvector NIR Shootout 2002, SpectroChemPy DSO and SpectroChemPy
  ALS2004 structured MAT fixtures and prospectr `NIRsoil.RData`;
- Excel workbook reader; experimental for simple `.xlsx/.xlsm` spectral tables
  and canonical `spectra`/`metadata`/`references` multi-sheet lab templates
  with numeric wavelength headers;
- Thermo Nicolet OMNIC reader; experimental for `.SPA` single spectra,
  `.spg` grouped spectra, committed TGA/GC `.srs` time-series matrices and
  local-only rapid-scan raw/reprocessed `.srs`; `.srsx` and broader high-speed
  variants still pending;
- Perkin Elmer Spectrum / IR reader; experimental for single-spectrum `.sp`
  `PEPE` block files, with `.fsm` imaging refused for v1;
- BUCHI NIRCal reader; experimental for the committed `NIRCAL Project File`
  spectra, wavenumber, project identity, replicate metadata and property-target
  sections, with a redistributable non-zero target fixture still pending;
- JASCO JWS reader; experimental for OLE2 `DataInfo` + `Y-Data`
  FT/IR transmittance, fluorescence and CD/HT/Abs multi-channel fixtures, with
  semantic channel labels inferred from metadata;
- Horiba LabSpec / JobinYvon reader; experimental for LSX XML single spectra,
  range exports, linescans and maps, plus LabSpec two-column, series-row and
  map-row text exports;
- Renishaw WDF reader; experimental for `WDF1` spectral payloads using `DATA`,
  `XLST`, `YLST`, `ORGN` and `WMAP`; maps/lines/depth/time-series records are
  emitted one stored spectrum at a time with spatial, elapsed-time and map-index
  metadata, plus conservative `WHTL` JPEG image metadata and golden-backed
  `MAP ` PSET `dataRange` analysis metadata for observed map/depth fixtures;
- Princeton TriVista TVF reader; experimental for XML frame payloads,
  `xDim/Calibration` spectral axes, X/Y navigation axes, time-series
  timestamps, Step-and-Glue child windows and detector/spectrometer metadata;
- DigitalSurf MountainsMap reader; experimental for fixed-header `.sur/.pro`
  spectra, multi-spectrum profiles, hyperspectral maps, surface profiles and
  `DSCOMPRESSED` zlib payloads;
- Hamamatsu HPD-TA `.img` reader; experimental adjacent-format support for
  2D streak-camera `y,x` raw-count signals with time/CCD secondary-axis
  metadata;
- WiTec WIP/WID detector/refusal path plus a narrow `WIT_PR06` TDGraph decoder
  for the committed `Sa4.wip` fixture, including Raman-shift axis and map
  coordinate derivation; legacy `WIT^` and unknown `WIT_PR06` layouts are
  refused with explicit WiTec Project/FIVE ASCII-export guidance;
- mzML detector/refusal path; committed MS fixtures are recognized and rejected
  with explicit `pyteomics` / `pymzML` guidance instead of being coerced into
  optical spectra;
- ANDI/MS NetCDF detector/refusal path; committed chromatography/MS fixture is
  recognized from standard variables and rejected with explicit
  `pyteomics.openms.ANDIMS` / `PyMassSpec` / `pyOpenMS` guidance;
- Bruker DPT reader; experimental;
- Avantes ASCII exports; experimental;
- golden JSON writer/validator; golden summaries implemented;
- docs for accepted fixture metadata.

Gate:

- no extension-only false positive on known collision fixtures;
- golden JSON includes axis, signal, metadata, provenance and warnings;
- Python `to_numpy_matrix()` and `to_pandas_frame()` work for these records.

## Phase 2: JCAMP-DX

Status: in progress. `XYDATA` single-block and top-level multi-block files,
ASDF PAC/SQZ/DIF/DUP ordinate encodings, NMR `NTUPLES` real/imaginary pages and Ocean Optics
`LINK`/`XYPOINTS` blocks are experimental. `PEAK TABLE` is now explicitly
refused and short `NPOINTS` payloads fail strictly; real peak-list support and
broader multi-block `LINK` files remain pending.

Deliverables:

- JCAMP AFFN and `XYDATA`; experimental;
- PAC/SQZ/DIF/DUP ASDF decoding; experimental;
- NMR `NTUPLES` real/imaginary pages; experimental;
- Ocean Optics `LINK`/`XYPOINTS` sample-dark-reference blocks; experimental;
- explicit `PEAK TABLE` refusal until a sparse peak-list export model exists;
- strict short-`NPOINTS` rejection and incompatible-axis `LINK` rejection;
- then real `PEAK TABLE` support and broader multi-block `LINK`;
- conformance against open JCAMP readers where possible;
- adversarial tests for malformed label-data records and compressed archives.

## Phase 3: Field Spectrometers

Status: in progress for SED and SVC/GER SIG ASCII fixtures.

Deliverables:

- Spectral Evolution `.sed`; experimental;
- SVC/GER `.sig`; experimental;
- ASD `.asd`; experimental primary-spectrum reader for revisions 1/6/7/8;
- first full metadata/PII redaction policy implementation.

## Phase 4: First Bindings

Starts after three readers reach `Done`. A temporary Python bridge already
routes through the Rust CLI so downstream integration work can start without
duplicating parsers.

Deliverables:

- native Python binding backed by Rust; CLI bridge experimental, native extension pending;
- numpy and pandas exports; experimental;
- sklearn-compatible dataset provider; experimental;
- torch dataset adapter; experimental;
- nirs4all `SpectroDataset` adapter; experimental;
- R package backed by Rust/C ABI; CLI bridge experimental, C ABI pending;
- R matrix/data.frame/tibble-compatible exports; experimental;
- cross-binding fixtures for every `Done` reader.

## Phase 5: Binary Reader Batch

Status: in progress. Galactic SPC, Bruker OPUS native, Avantes binaries, ENVI
SLI and Ocean Optics text/ProcSpec/SPC now have experimental readers for their
first high-value subsets.

Deliverables:

- Galactic SPC; experimental;
- Bruker OPUS; experimental 1D data/status block pairs;
- Avantes 6/7 and AvaSoft 8; experimental for committed legacy and AVS8 fixtures;
- ENVI SLI sidecar handling; experimental for one-band BSQ spectral libraries;
- Ocean Optics text and ProcSpec; experimental for SpectraSuite, OceanView,
  Jaz, CRAIC, two-column CSV exports, OceanView ZIP/XML `.ProcSpec` and the
  committed Galactic-layout Ocean Optics `.spc` sample;
- EMSA/MAS `.msa`; experimental for ISO 22029-style `XY` and `Y` fixtures;
- row-oriented text exports; experimental for several vendor and scientific
  fixtures where the first column is the spectral axis;
- one-spectrum-per-row matrix exports and sun photometer channel tables;
  experimental for committed text fixtures;
- AnIML XML and Allotrope ASM JSON structured exports; experimental for
  committed spectroscopy-like fixtures;
- generic HDF5 structured containers; experimental for simple NIRS datasets
  and nested FGI-style groups, with XML sidecar mapping still pending;
- performance dashboards for large file and many-small-file scenarios.

## Phase 6: Packaging and Deployment

Deliverables:

- PyPI wheels through GitHub Actions;
- local R build/release procedure;
- C ABI headers and release archives;
- docs for deployment matrix and supported platforms.

## Phase 7+: Continuous Format Expansion

Add one family at a time:

- remaining Nicolet OMNIC `.srs/.srsx` variants and a non-zero BUCHI NIRCal
  target fixture, each validated against an open reference reader when possible;
- WiTec native WIP/WID parsing once a redistributable or private-test binary
  fixture exists; DigitalSurf richer comment/metadata parsing, TriVista
  objective/hardware-branch metadata and later Renishaw WDF `MAP ` layout
  variants if additional derived analysis maps become part of the export model;
- harden AnIML XML and Allotrope ASM JSON beyond the initial spectral fixtures;
- Foss/Metrohm/Perten native formats as fixtures and reverse-engineering
  evidence become available;
- adjacent formats only when they help disambiguation or user workflows.

Every new format repeats the same lifecycle: Experimental -> Beta -> Done.

## Owner-Requested Documentation Backlog

Keep these items at the bottom of the roadmap until they are scheduled and
closed:

- rewrite the root `README.md` so it states the Rust-first architecture,
  binding strategy, current maturity and contribution path clearly;
- add implementation visualizations for format coverage, probe confidence,
  maturity level and missing fixture/reference-reader gaps;
- keep `docs/FORMAT_GAPS.md` current whenever an unsupported, sample-blocked,
  unknown or deliberately refused format is discovered;
- audit every format page under `docs/formats/` so each one describes the file
  format, implemented behavior, missing behavior, validation fixtures,
  reference libraries and conformance status.
