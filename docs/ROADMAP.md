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
- AnIML spectral XML reader; experimental for explicit spectral `SeriesSet`
  fixtures and refusal of non-spectral AnIML results;
- Allotrope ASM JSON bridge; experimental for plate-reader spectral data cubes
  and detector-wavelength endpoint readings;
- SiWare API JSON reader; experimental for one-measurement NeoSpectra-style
  payloads;
- NetCDF NIRS reader; experimental for simple `spectra` + `wavelengths`
  datasets and refusal of adjacent non-NIRS NetCDF containers;
- generic HDF5 NIRS reader; experimental for root and nested-group `spectra` +
  `wavelengths` datasets, including the committed synthetic FGI HDF5 payload;
- MATLAB MAT reader; experimental for simple MAT v5 and MATLAB v7.3/HDF5
  `X` + `wavelengths` + optional `y` datasets;
- Bruker DPT reader; experimental;
- Avantes ASCII exports; experimental;
- golden JSON writer/validator; golden summaries implemented;
- docs for accepted fixture metadata.

Gate:

- no extension-only false positive on known collision fixtures;
- golden JSON includes axis, signal, metadata, provenance and warnings;
- Python `to_numpy_matrix()` and `to_pandas_frame()` work for these records.

## Phase 2: JCAMP-DX

Status: in progress. Single-block `XYDATA`, ASDF PAC/SQZ/DIF/DUP ordinate
encodings, NMR `NTUPLES` real/imaginary pages and Ocean Optics
`LINK`/`XYPOINTS` blocks are experimental; `PEAK TABLE` and broader multi-block
files remain pending.

Deliverables:

- JCAMP AFFN and `XYDATA`; experimental;
- PAC/SQZ/DIF/DUP ASDF decoding; experimental;
- NMR `NTUPLES` real/imaginary pages; experimental;
- Ocean Optics `LINK`/`XYPOINTS` sample-dark-reference blocks; experimental;
- then `PEAK TABLE` and broader multi-block `LINK`;
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

- MATLAB struct/DSO and `.RData` academic dataset mappers after the simple
  MAT v5/v7.3 matrix path;
- Nicolet OMNIC `.spa/.spg/.srs`, Perkin Elmer `.sp`, BUCHI NIRCal `.nir` and
  JASCO `.jws`, each validated against an open reference reader when possible;
- Excel `.xlsx`, Renishaw WDF and Horiba / JobinYvon LabSpec XML/text for
  lab-transfer and adjacent spectroscopy workflows;
- harden AnIML XML and Allotrope ASM JSON beyond the initial spectral fixtures;
- Foss/Metrohm/Perten native formats as fixtures and reverse-engineering
  evidence become available;
- adjacent formats only when they help disambiguation or user workflows.

Every new format repeats the same lifecycle: Experimental -> Beta -> Done.
