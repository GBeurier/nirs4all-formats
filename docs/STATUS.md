# Project Status

Last updated: 2026-05-20.

## Current Checkpoint

Phase 0 is complete and Phase 1 has started:

- format inventory and fixture corpus already exist;
- Rust workspace is scaffolded and pushed to GitHub;
- Python and R binding skeletons exist;
- reverse-engineering helper package exists;
- GitHub Actions and RTD configuration exist;
- first native Rust readers are implemented and tested on committed samples.

Experimental native readers:

- delimited spectral tables (`.csv`, `.tsv`, headered `.txt`);
- row-oriented spectral tables with an axis in the first column: Si-Ware CSV,
  MODTRAN `.dat`, PP Systems `.SPT/.SPU`, ENVI/ECOSTRESS/IDL spectrum text,
  JASCO text export, Shimadzu TXT, USGS SPECPR ASCII and WiTec TXT fixtures;
- spectral matrix exports with one spectrum per row: Foss/WinISI text, real
  Foss XDS CSV, AuroraNIR handheld CSV, OSSL NeoSpectra wide CSV, Metrohm Vision
  Air CSV and VIAVI MicroNIR CSV fixtures;
- sun photometer channel exports: MFR `.OUT`, Microtops `.TXT`, and the
  committed Microtops MAN NetCDF AOT fixture;
- AnIML spectral XML: spectral `SeriesSet` fixture with wavelength axis,
  absorbance signal and sample target; non-spectral AnIML result documents are
  refused;
- Allotrope ASM JSON: plate-reader spectral data cubes and detector-wavelength
  endpoint readings from committed Benchling allotropy fixtures;
- SiWare API JSON: NeoSpectra-style `measurement.wavelengths` and
  `measurement.absorbance` payloads with predictions mapped to targets;
- Consumer Physics SCiO CSV: plain `band*` developer-app scans and grouped
  `spectrum_*` / `wr_raw_*` / `sample_raw_*` exports at 740-1070 nm;
- NetCDF NIRS datasets: simple `spectra` + `wavelengths` containers using a
  pure-Rust reader, plus a SHA-256-guarded Microtops MAN NetCDF fixture path;
  ANDI/MS gets a dedicated refusal path and weather/PyrNet NetCDF samples are
  schema-refused as non-NIRS;
- generic HDF5 NIRS datasets: root or nested-group `spectra` + `wavelengths`
  containers using a pure-Rust reader; non-spectral HDF5 samples are refused,
  and the committed FGI HDF5 payload is covered while XML sidecar mapping stays
  pending;
- MATLAB MAT datasets: simple MAT v5 and MATLAB v7.3/HDF5 `X` + `wavelengths`
  + optional `y` datasets, plus committed Eigenvector Corn, Eigenvector NIR
  Shootout 2002, SpectroChemPy DSO and SpectroChemPy ALS2004 structured MAT
  fixtures, and prospectr `NIRsoil.RData` RDX3/XZ workspace mapping;
- Excel workbooks: simple `.xlsx/.xlsm` spectral tables, first-cell
  `axis: ... / data: ...` descriptors used by UvA handheld XLSX exports, and
  canonical `spectra`/`metadata`/`references` multi-sheet lab templates with
  numeric wavelength headers; legacy `.xls` remains pending;
- Bruker OPUS DPT ASCII export (`.dpt`);
- Bruker OPUS native binaries, 1D data/status block pairs;
- Avantes AvaSoft ASCII wave tables (`.ttt`, `.trt`, `.tit`, `.tat`) and two-column irradiance export (`.IRR`);
- Avantes AvaSoft legacy binaries (`.TRM`, `.ROH`, `.DRK`, `.REF`) and AvaSoft 8 binaries (`.Raw8`, `.IRR8`);
- ENVI Spectral Library sidecars (`.sli` + `.hdr`), one-band BSQ float32/float64 payloads;
- AVIRIS / ERDAS LAN (`92AV3C.lan`) Indian Pines cube: 145 x 145 x 220 u16
  BIL payload expanded to one raw-count spectrum per pixel, with wavelength
  axis from `.spc` and optional ground-truth class targets from `.GIS`;
- Ocean Optics / Ocean Insight exports (`.txt`, `.csv`, `.jaz`, `.JazIrrad`, `.Master.Transmission`)
  and `.ProcSpec` ZIP/XML archives; the committed Ocean Optics `.spc` sample is
  covered by the Galactic SPC reader;
- JCAMP-DX `XYDATA=(X++(Y..Y))` with plain AFFN plus PAC/SQZ/DIF/DUP ASDF decoding,
  NMR `NTUPLES` real/imaginary pages, and Ocean Optics `LINK`/`XYPOINTS` blocks;
  `PEAK TABLE` inputs are explicitly refused, incompatible-axis `LINK` children
  are rejected and short `NPOINTS` payloads fail strictly;
- EMSA/MAS `.msa` (ISO 22029-style) `XY` and `Y` single-spectrum text files;
- Spectral Evolution SED (`.sed`);
- SVC/GER SIG (`.sig`).
- ASD FieldSpec (`.asd` and ASD binaries with numeric extensions), revisions 1/6/7/8.
- Thermo / Galactic GRAMS SPC (`.spc`, `.SPC`), new little-endian generated-X,
  explicit-X, multi common-X and `-XYXY` directory layouts; old little-endian
  support is limited.
- Thermo Nicolet OMNIC (`.SPA`, `.spg`, `.srs`) single spectra and grouped
  spectra via the reverse-engineered key table, plus TGA/GC `.srs` time-series
  matrices as 2D `y,x` records; non-TGA/GC `.srs/.srsx` series variants are
  classified and refused explicitly until a real fixture/reference export is
  available;
- Perkin Elmer Spectrum / IR (`.sp`) single spectra via the `PEPE` block
  container; `.fsm` Spotlight imaging is detected but out of scope for v1.
- BUCHI NIRCal (`.nir`) `NIRCAL Project File` spectra and wavenumber sections
  for the committed foliar-transfer fixture, including property target schema
  extraction with zero values mapped to null targets.
- JASCO JWS (`.jws`) OLE2 `DataInfo` + `Y-Data` spectra for committed
  FT/IR transmittance, fluorescence and CD/HT/Abs multi-channel fixtures, with
  metadata-driven semantic channel labels.
- Horiba LabSpec / JobinYvon XML/text exports for committed single-spectrum,
  range, linescan, map, two-column, series-row and map-row Raman fixtures, plus
  an experimental LabSpec6 `.l6m` binary map decoder validated against the
  paired Gd2O3/AlN text export. Text exports without explicit axis units are
  inferred as `cm-1`; XML `eV` axes are preserved with an energy-axis fallback
  warning.
- Renishaw WDF (`.wdf`) spectral payloads via `WDF1`, `DATA`, `XLST` and
  `YLST` chunks plus `ORGN`/`WMAP` navigation metadata. Map, line, depth,
  FocusTrack, time-series and interrupted acquisitions emit one record per
  stored spectrum with normalized spatial, elapsed-time and map-index metadata;
  `WHTL` JPEG white-light image metadata and `MAP ` PSET analysis-block
  inventories are preserved without decoding derived images as spectra.
- Princeton TriVista TVF (`.tvf`) XML frame payloads for committed single
  spectra, line scans, maps, time-series and Step-and-Glue fixtures. The reader
  emits one record per frame and preserves Step-and-Glue child windows.
- DigitalSurf MountainsMap (`.sur`, `.pro`) spectral/profile/surface payloads
  via fixed headers and zlib-stream compression. Single spectra, multi-spectrum
  profiles and hyperspectral maps emit one record per spectrum or XY point;
  plain surfaces emit one spatial-profile record per row with a warning.
- Hamamatsu HPD-TA streak-camera `.img` files as adjacent 2D `y,x` signals.
  The reader covers committed focus, operate, photon-counting, shading and
  uncalibrated-axis fixtures, preserving the secondary time/CCD axis in
  metadata and warning that the signal is not a point-sample NIR spectrum.
- WiTec WIP/WID binary project files are detected from `.wip/.wid` plus `WIT^`
  or `WIT_PR06` magic. The committed `Sa4.wip` `WIT_PR06` TDGraph map decodes
  experimentally into 4410 valid raw-count spectra with a polynomial wavelength
  axis; legacy `WIT^` and unknown `WIT_PR06` layouts are refused explicitly.
  The committed WiTec TXT export remains covered by the row-oriented spectral
  table reader.
- mzML mass-spectrometry XML is detected and refused with a pointer to
  `pyteomics`, `pymzML` or `pyOpenMS`; committed mzML spectrum/chromatogram
  fixtures cover the refusal path.
- ANDI/MS NetCDF chromatography containers are detected from standard
  acquisition-time, mass and intensity variables and refused with a pointer to
  `pyteomics.openms.ANDIMS`, `PyMassSpec` or `pyOpenMS`.

Golden-summary conformance exists for the fixtures above under
`crates/nirs4all-io/tests/goldens/`.

Untreated formats, missing fixture blockers, unknown binary layouts and
deliberate refusal paths are now documented in `docs/FORMAT_GAPS.md`, with
`MISSING_SAMPLES.md` retained as the exact fixture checklist.

Python bridge surfaces now call the Rust CLI rather than implementing parser
logic in Python:

- raw record access;
- tabular `NirsDataset`;
- numpy, pandas and sklearn-style exports;
- torch dataset adapter;
- `nirs4all.data.SpectroDataset` adapter.

R bridge surfaces also call the Rust CLI and expose:

- raw record access;
- `nirs4allio_dataset`;
- `matrix`, `data.frame` and optional tibble conversion.

## Last Green Gate

Green locally on 2026-05-20:

```bash
. "$HOME/.cargo/env"
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
uv run --python 3.11 --with-editable ./tools/reverse-lab --with-editable ./bindings/python --with-editable /home/delete/nirs4all/nirs4all --with pytest pytest tools/reverse-lab/tests bindings/python/tests
Rscript -e 'Sys.setenv(NIRS4ALL_IO_REPO=getwd()); library(nirs4allio); records <- nirs4allio_open_records("samples/csv_tsv/synthetic_nirs.csv"); dataset <- nirs4allio_open_dataset("samples/csv_tsv/synthetic_nirs.csv"); stopifnot(length(records) == 50L, all(dim(as.matrix(dataset)) == c(50L, 200L)), nrow(as.data.frame(dataset)) == 50L)'
uv run sphinx-build -W -b html docs docs/_build/html
```

## Next Agent Prompt

Continue from `/home/delete/nirs4all/nirs4all-io`. Keep Rust as the canonical
core. Do not implement parser logic in Python or R bindings.

Immediate next work:

1. decide whether AVIRIS/Indian Pines sample redistribution terms allow keeping
   the committed `.lan/.spc/.GIS` fixtures in public release artifacts;
2. add ROI/mask extraction for hyperspectral cubes so large NEON/Specim/HySpex/
   Headwall scenes do not require whole-cube expansion;
3. continue the open-reader-backed binary batch in this order: remaining
   Nicolet OMNIC `.srs/.srsx` variants and a non-zero BUCHI NIRCal target
   fixture when available;
4. add direct external reference-reader conformance for OPUS/SPC/JCAMP/SED/SIG/ASM/HDF5 where practical;
5. replace Python/R subprocess transport with native PyO3/C ABI paths;
6. harden JCAMP line-level X checkpoint validation and implement `PEAK TABLE`
   only after the shared model can represent sparse peak lists;
7. keep `docs/STATUS.md` and `docs/ROADMAP.md` current after each green gate.
8. owner-requested documentation tail work: rewrite the root `README.md`,
   add implementation visualizations for format/probe-confidence/maturity/
   missing-fixture status, and audit every `docs/formats/` page for
   description, implemented behavior, missing behavior and validation status.
