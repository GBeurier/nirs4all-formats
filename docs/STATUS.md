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
- sun photometer channel exports: MFR `.OUT`, local ARM MFRSR b1 NetCDF with
  optional ARM QC YAML sidecar ranges, Microtops `.TXT`, the committed
  Microtops MAN NetCDF AOT fixture with typed aerosol-optical-thickness
  signals, and
  local-only AERONET MAN ASCII `.lev10/.lev15/.lev20` exports;
- AnIML spectral XML: spectral `SeriesSet` fixtures with explicit values or
  uniform `AutoIncrementedValueSet` wavelength axes, absorbance signal and
  sample target; non-spectral AnIML result documents are refused;
- Allotrope ASM JSON: plate-reader spectral data cubes and detector-wavelength
  endpoint readings from committed Benchling allotropy fixtures;
- Allotrope ADF: local-only `adfsee` HDF5 fixture with numeric `/data-cubes`
  extracted as an experimental subset, plus minimal RDF component mapping for
  cube titles, typed seconds axes, secondary nm scales and absorbance units; full
  ontology resolution and SDK validation remain pending;
- SiWare API JSON / CSV companion: NeoSpectra-style
  `measurement.wavelengths` and `measurement.absorbance` JSON payloads with
  predictions mapped to targets, plus the synthetic CSV stream through
  `row-spectral-table`;
- Consumer Physics SCiO CSV: plain `band*` developer-app scans and grouped
  `spectrum_*` / `wr_raw_*` / `sample_raw_*` exports at 740-1070 nm;
- NetCDF NIRS datasets: simple `spectra` + `wavelengths` containers using a
  pure-Rust reader, a local ARM MFRSR b1 7-filter time-series path with
  optional QC YAML sidecar mapping, local ARM
  SURFSPECALB derived albedo, plus Microtops MAN NetCDF `aot_<nm>` discovery
  with a SHA-256-guarded MSM114/2 payload fallback; ANDI/MS gets a dedicated
  refusal path and weather/PyrNet/AOSMET NetCDF samples are schema-refused as
  non-NIRS;
- generic HDF5 NIRS datasets: root or nested spectral groups using a pure-Rust
  reader, with common spectra/axis aliases, multi-signal groups sharing one
  axis and unambiguous transposed matrices; non-spectral HDF5 samples are
  refused, and the committed FGI HDF5+XML
  synthetic pair is mapped with both payload and metadata sidecar in provenance;
- MATLAB MAT datasets: simple MAT v5 and MATLAB v7.3/HDF5 `X` + `wavelengths`
  + optional `y` datasets, plus committed Eigenvector Corn, Eigenvector NIR
  Shootout 2002, SpectroChemPy DSO and SpectroChemPy ALS2004 structured MAT
  fixtures, prospectr `NIRsoil.RData` RDX3/XZ workspace mapping, and the
  local-only Indian Pines MATLAB v5 cube mapped to one raw-count spectrum per
  pixel with optional `_gt.mat` class targets;
- Excel workbooks: simple `.xlsx` spectral tables, `.xlsm` through the same
  fixture-backed OOXML code path, first-cell
  `axis: ... / data: ...` descriptors used by UvA handheld XLSX exports, and
  canonical `spectra`/`metadata`/`references` multi-sheet lab templates with
  numeric wavelength headers; legacy `.xls` remains pending;
- Bruker OPUS DPT ASCII export (`.dpt`), including the real `RS-1.dpt`
  fixture from `lightr`;
- Bruker OPUS native binaries, 1D data/status block pairs and cross-reader
  fixtures with semantic tests over the committed OPUS corpus;
- Avantes AvaSoft ASCII wave tables (`.ttt`, `.trt`, `.tit`, `.tat`) and two-column irradiance export (`.IRR`);
- Avantes AvaSoft legacy binaries (`.TRM`, `.ROH`, `.DRK`, `.REF`) and
  AvaSoft 8 binaries (`.Raw8`, `.IRR8`) with decoded AVS8 SPC date/time;
- ENVI Spectral Library sidecars (`.sli` + `.hdr`), one-band BSQ float32/float64
  payloads, and ENVI Standard cubes expanded by pixel with parsed `map info`;
- AVIRIS / ERDAS LAN (`92AV3C.lan`) Indian Pines cube: 145 x 145 x 220 u16
  BIL payload expanded to one raw-count spectrum per pixel, with wavelength
  axis from `.spc` and optional ground-truth class targets from `.GIS`;
- Ocean Optics / Ocean Insight exports (`.txt`, `.csv`, `.jaz`, `.JazIrrad`,
  `.Master.Transmission`) and `.ProcSpec` ZIP/XML archives with XML-driven
  transmittance/reflectance typing; all committed Ocean fixtures are
  golden-backed, Ocean JCAMP is routed through JCAMP-DX and the committed Ocean
  Optics `.spc` sample is covered by the Galactic SPC reader;
- JCAMP-DX `XYDATA=(X++(Y..Y))` with plain AFFN plus PAC/SQZ/DIF/DUP ASDF
  decoding, top-level multi-block XYDATA files as multiple records, NMR
  `NTUPLES` real/imaginary pages with frequency/time axes, Ocean Optics
  `LINK`/`XYPOINTS` blocks, and top-level sparse `PEAK TABLE` /
  `PEAK ASSIGNMENTS` records; incompatible-axis `LINK` children are rejected
  and short `NPOINTS` payloads fail strictly;
- EMSA/MAS `.msa` (ISO 22029-style) `XY` and `Y` single-spectrum text files
  with typed energy axes for `eV` data;
- Spectral Evolution SED (`.sed`), including DN-only broken-but-valid files
  flagged when no reflectance signal is present; DN, percent/fraction
  reflectance, source-signal labels/units and parseable instrument/GPS/date/time
  headers are promoted to canonical metadata;
- SVC/GER SIG (`.sig`), including semantic assertions over the committed
  PDA/laptop/white-reference/matched-overlap fixtures and declared bad
  fixtures flagged in quality metadata.
- USGS spectral-library text: SPECPR-style `.asc` wavelength/reflectance
  tables, ECOSTRESS/ASTER `.spectrum.txt` exports and single-column AREF dumps
  with generated index axes.
- ASD FieldSpec (`.asd` and ASD binaries with numeric extensions), revisions 1/6/7/8.
- Thermo / Galactic GRAMS SPC (`.spc`, `.SPC`), new little-endian generated-X,
  explicit-X, multi common-X and `-XYXY` directory layouts; old little-endian
  support is limited.
- Thermo Nicolet OMNIC (`.SPA`, `.spg`, `.srs`) single spectra and grouped
  spectra via the reverse-engineered key table, plus TGA/GC and local
  rapid-scan `.srs` matrices as 2D `y,x` records; unsupported `.srs` anchor
  patterns and `.srsx` remain explicit pending variants;
- Perkin Elmer Spectrum / IR (`.sp`) single spectra via the `PEPE` block
  container; `.fsm` Spotlight imaging is detected but out of scope for v1.
- BUCHI NIRCal (`.nir`) `NIRCAL Project File` spectra and wavenumber sections
  for the committed foliar-transfer fixture, including property target schema
  extraction with zero values mapped to null targets. A local-only cannabis
  fixture validates non-null `CBDA` and `THCA` targets through the same path.
- JASCO JWS (`.jws`) OLE2 `DataInfo` + `Y-Data` spectra for committed
  FT/IR transmittance, fluorescence and CD/HT/Abs multi-channel fixtures, with
  metadata-driven semantic channel labels.
- Horiba LabSpec / JobinYvon XML/text exports for committed single-spectrum,
  range, linescan, map, two-column, series-row and map-row Raman fixtures, plus
  an experimental LabSpec6 `.l6m` binary map decoder validated against the
  paired Gd2O3/AlN text export. Text exports without explicit axis units are
  inferred as `cm-1`; XML `eV` axes are typed as energy axes.
- Renishaw WDF (`.wdf`) spectral payloads via `WDF1`, `DATA`, `XLST` and
  `YLST` chunks plus `ORGN`/`WMAP` navigation metadata. Map, line, depth,
  FocusTrack, time-series and interrupted acquisitions emit one record per
  stored spectrum with normalized spatial, elapsed-time and map-index metadata;
  `WHTL` JPEG white-light image metadata is preserved, and observed `MAP `
  PSET `dataRange` tails are decoded as per-record derived metadata when their
  length matches the stored spectrum count. The map/depth `dataRange` fixtures
  are golden-backed.
- Princeton TriVista TVF (`.tvf`) XML frame payloads for committed single
  spectra, line scans, maps, time-series and Step-and-Glue fixtures. The reader
  emits one record per frame, validates `xDim` lengths, preserves
  Step-and-Glue child windows, and promotes detector plus numbered
  spectrometer metadata. Spatial navigation units are no longer guessed when
  absent in the source metadata.
- DigitalSurf MountainsMap (`.sur`, `.pro`) spectral/profile/surface payloads
  via fixed headers and zlib-stream compression. Single spectra, multi-spectrum
  profiles and hyperspectral maps emit one record per spectrum or XY point;
  plain surfaces emit one spatial-profile record per row with a warning.
  Hyperspectral maps and surfaces expose normalized spatial indices, dimensions
  and explicit axis-order metadata.
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
git diff --check
```

Local-only sample sweep (not in CI): `samples_local/` now has 15 successful
reads, 5 expected refusals and 0 unexpected refusals.

## Next Agent Prompt

Continue from `/home/delete/nirs4all/nirs4all-io`. Keep Rust as the canonical
core. Do not implement parser logic in Python or R bindings.

Immediate next work:

1. decide whether AVIRIS/Indian Pines sample redistribution terms allow keeping
   the committed `.lan/.spc/.GIS` fixtures in public release artifacts, and
   keep the EHU MATLAB cube path local-only unless redistribution terms change;
2. add ROI/mask extraction for hyperspectral cubes so large NEON/Specim/HySpex/
   Headwall scenes do not require whole-cube expansion;
3. continue the open-reader-backed binary batch in this order: OMNIC `.srsx`
   and high-speed variants beyond local SpectroChemPy samples, redistributable
   BUCHI NIRCal non-null target fixtures and `.cal`/NIRMaster variants;
4. add direct external reference-reader conformance for OPUS/SPC/JCAMP/SED/SIG/ASM/HDF5 where practical;
5. replace Python/R subprocess transport with native PyO3/C ABI paths;
6. harden JCAMP line-level X checkpoint validation, source real PEAK
   TABLE/ASSIGNMENTS fixtures for conformance and decide the public API shape
   for heterogeneous `LINK` fan-out;
7. keep `docs/STATUS.md` and `docs/ROADMAP.md` current after each green gate.
8. owner-requested documentation tail work: rewrite the root `README.md`,
   add implementation visualizations for format/probe-confidence/maturity/
   missing-fixture status, and audit every `docs/formats/` page for
   description, implemented behavior, missing behavior and validation status.
