# Project Status

Last updated: 2026-09-03.

> 2026-09-03 — **0.2.9 dependency security candidate.** The direct XML
> parser and the Excel reader chain now converge on `quick-xml` 0.41.0 through
> `calamine` 0.36.1, resolving `RUSTSEC-2026-0194` and
> `RUSTSEC-2026-0195`. The lock also advances `crossbeam-epoch` to 0.9.20 for
> `RUSTSEC-2026-0204`. Public reader and binding contracts are unchanged.

> 2026-09-02 — **0.2.8 release qualification hardening.** Public source
> archives now exclude local/customer corpus material and are checked by a
> fail-closed inventory gate. Reference-reader conformance also fails when no
> non-skipped case executes. Licensing and Python platform documentation now
> match the package metadata and active workflows. The encrypted local corpus
> remains tracked in Git history pending a separate repository-policy decision;
> archive exclusion does not erase that debt.

> 2026-06-10 — **FOSS DS-series native `.nir` support** added to the `foss_winisi`
> reader. The DS2500 and DS3 F benches emit the same binary container as the
> ISIscan/WinISI files but carry no `ISIscan`/`NIRSystems` string; they are pinned
> by the `NIRS DS` instrument model at `0x82` and decoded through the existing
> parser as a new probe `foss-ds-nir` (spectra-only, version word `1`). Validated
> on the real local-only corpora (`fileDS2500CRAW.nir` 10 samples / 400–2498 nm,
> `fileDS3FCRAW.nir` 20 samples / 1100–2498 nm, customer data CEPICOP/SYNGENTA);
> CI covered by new CC0 synthetic goldens (`foss_ds2500_native_nir`,
> `foss_ds3f_native_nir`). The MicroNIR `.sam` (120 spectra + CSV) and PerkinElmer
> MIR `.sp` corpora were fully integrated into `samples_local/`, which is now also
> committed as the encrypted `samples_local.tar.gz.enc` (key gitignored, see
> `docs/dev/SAMPLES_LOCAL_ARCHIVE.md`). Green gate re-run below (Rust core + Python
> binding + docs locally; wasm/demo rebuild and R/node tests deferred to CI —
> clang/node/Rscript absent on the dev box).

> 2026-06-03 — **Three native NIRS binary readers added/extended** (one agent per
> format, each validated against ground truth). New `foss_winisi` reader decodes
> FOSS NIRSystems / ISIscan / WinISI `.cal` (spectra + constituents) and `.nir`
> (spectra-only) — one record per sample with constituent reference values as
> targets — sniffed by header signature (mot de version + `ISIscan`/`NIRSystems`),
> never by extension, which resolves the `.nir` collision with BUCHI NIRCal;
> validated against the ISIscan `.txt` exports to float32 precision (yamtot 18
> samples, ando4 20/9 constituents, D2026 18/5, yamtot.nir 16/0). New
> `viavi_micronir` reader decodes the Viavi/JDSU MicroNIR `.sam` (`MNIR` .NET
> container) into 125 absorbance channels (interpolated `nm` axis ~908.1-1676.2) +
> 128 raw single-beam pixels, with the `\x04MNIR` Definite sniff resolving its
> former Galactic-SPC false positive. The `perkin_elmer` reader was extended to
> tolerate the trailing audit/CustomColumn blocks that Spectrum 10 FT-MIR `.sp`
> exports append after the root block (31 real files validated vs their CSV
> exports to ~5e-5) and hardened against malformed input (negative point count
> rejected, block recursion depth-capped — no panics). Reviewed by an adversarial
> multi-agent pass (0 bugs on the two new readers; 2 pre-existing PerkinElmer
> robustness bugs found and fixed) and by `codex review`. Real vendor fixtures are
> local-only under `samples_local/` (licence TBD, gitignored); committed CC0
> synthetic goldens (`synthetic.cal`, `synthetic_micronir.sam`,
> `synthetic_trailing.sp`) cover CI. Full green gate re-run below.

> 2026-05-27 — **Renamed `nirs4all-io` → `nirs4all-formats`** (Phase 0 of the
> formats/io redesign, distinct from this repo's internal roadmap phases). All
> crates, bindings (Python/R/WASM), the C ABI (`n4fmt_*`, `NIRS4ALL_FORMATS_H`),
> env vars (`NIRS4ALL_FORMATS_*`), provenance literals, the 231 goldens (re-blessed),
> CI/release workflows, and docs were renamed. Full green gate re-run green below.
> The freed `nirs4all-io` name is reused by the new dataset-bridge library (separate repo).

> 2026-05-26 — Documentation pass: the per-format pages
> (`docs/formats/*.md`) were rewritten into a single publishable template, a
> public [`SUPPORTED_FORMATS.md`](SUPPORTED_FORMATS.md) catalogue and
> getting-started / installation / usage guides were added, all binding docs
> (Python/R/WASM/C ABI) were refreshed, and GitHub issue forms +
> `CONTRIBUTING.md` were added. Re-run `sphinx-build -W` as part of the next
> green gate.

## Current Checkpoint

V1 RC status: package manifests are at the unpublished `0.2.9` security
candidate, with Python, Rust, WASM, R,
C ABI and source/provenance release workflows wired. PyPI, crates.io and npm
publish on non-prerelease tags; R source tarballs attach to GitHub Releases and
R-universe can lag until its from-Git rebuild catches up. The reader matrix below
is the current implementation surface, not the old Phase 0/Phase 1 scaffold.

Experimental native readers:

- delimited spectral tables (`.csv`, `.tsv`, headered `.txt`);
- row-oriented spectral tables with an axis in the first column: Si-Ware CSV,
  MODTRAN `.dat`, PP Systems `.SPT/.SPU`, ENVI/ECOSTRESS/IDL spectrum text,
  JASCO text export, Shimadzu TXT, USGS SPECPR ASCII and WiTec TXT fixtures;
- spectral matrix exports with one spectrum per row: Foss/WinISI text, real
  Foss XDS CSV, AuroraNIR handheld CSV, OSSL NeoSpectra wide CSV, Metrohm Vision
  Air CSV and VIAVI MicroNIR CSV fixtures;
- native vendor binaries: FOSS NIRSystems / ISIscan / WinISI `.cal`/`.nir`
  (`foss_winisi`, one record per sample with constituent targets, sniffed by
  header signature to avoid the BUCHI `.nir` collision), Viavi/JDSU MicroNIR
  `.sam` (`viavi_micronir`, 125 absorbance channels + 128 raw single-beam pixels)
  and PerkinElmer Spectrum 10 FT-MIR `.sp` with trailing audit blocks
  (`perkin_elmer`); real fixtures are local-only, CI uses committed CC0 synthetic
  goldens;
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
  with a generic `DataLayout::Contiguous` fallback for the MSM114/2 NetCDF4
  shared-attribute layout; ANDI/MS gets a dedicated
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
  payloads, and ENVI Standard cubes expanded by pixel, rectangular ROI or
  caller-ordered sparse `(row, col)` mask, with parsed `map info`;
- AVIRIS / ERDAS LAN (`92AV3C.lan`) Indian Pines cube: 145 x 145 x 220 u16
  BIL payload expanded to one raw-count spectrum per pixel, rectangular ROI or
  caller-ordered sparse `(row, col)` mask, with wavelength axis from `.spc` and
  optional ground-truth class targets from `.GIS`;
- Ocean Optics / Ocean Insight exports (`.txt`, `.csv`, `.jaz`, `.JazIrrad`,
  `.Master.Transmission`) and `.ProcSpec` ZIP/XML archives with XML-driven
  transmittance/reflectance typing; all committed Ocean fixtures are
  golden-backed, Ocean JCAMP is routed through JCAMP-DX and the committed Ocean
  Optics `.spc` sample is covered by the Galactic SPC reader;
- JCAMP-DX `XYDATA=(X++(Y..Y))` with plain AFFN plus PAC/SQZ/DIF/DUP ASDF
  decoding, top-level multi-block XYDATA files as multiple records, NMR
  `NTUPLES` real/imaginary pages with frequency/time axes, Ocean Optics
  `LINK`/`XYPOINTS` blocks, and top-level sparse `PEAK TABLE` /
  `PEAK ASSIGNMENTS` records. Same-axis LINK still collapses into one
  composite record (Ocean Optics flow); heterogeneous LINK (different
  child axes) now fans out one record per child (M3, 2026-05-23) with
  `link_parent_id`, `link_index`, `link_total` and `link_relation`
  metadata. Top-level multi-block files carry the same metadata. Short
  `NPOINTS` payloads fail strictly. `XYDATA` line-start X checkpoints are
  verified against the reconstructed axis; mismatches surface a
  structured `jcamp_xydata_x_checkpoint_drift` warning carrying absolute
  and relative deltas;
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
- ASD FieldSpec (`.asd` and ASD binaries with numeric extensions), revisions
  1/6/7/8, with internal secondary/reference/classifier/dependent/calibration/
  audit/signature block inventory.
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
  for the committed foliar-transfer fixture, including project GUID/version,
  sample replicate counters, per-spectrum `Spectra Info` metadata and property
  target schema extraction with zero values mapped to null targets. A
  local-only cannabis fixture validates non-null `CBDA` and `THCA` targets plus
  3 replicate spectra per sample through the same path.
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
  experimentally into 4410 valid raw-count spectra with a Raman-shift axis
  derived from `ExcitationWaveLength` and physical map coordinates derived from
  the TDGraph space transformation; legacy `WIT^` and unknown `WIT_PR06`
  layouts are refused explicitly.
  The committed WiTec TXT export remains covered by the row-oriented spectral
  table reader.
- mzML mass-spectrometry XML is detected and refused with a pointer to
  `pyteomics`, `pymzML` or `pyOpenMS`; committed mzML spectrum/chromatogram
  fixtures cover the refusal path.
- ANDI/MS NetCDF chromatography containers are detected from standard
  acquisition-time, mass and intensity variables and refused with a pointer to
  `pyteomics.openms.ANDIMS`, `PyMassSpec` or `pyOpenMS`.

Golden-summary conformance exists for the fixtures above under
`crates/nirs4all-formats/tests/goldens/`.

Untreated formats, missing fixture blockers, unknown binary layouts and
deliberate refusal paths are now documented in `docs/FORMAT_GAPS.md`, with
`MISSING_SAMPLES.md` retained as the exact fixture checklist.

Auto-discovery walker:

- `walk_path()` / `WalkOptions` in `nirs4all-formats` recursively traverse a
  directory, probe each file by head bytes and either decode it through the
  registry or mark it `Unsupported` / `Error`. The CLI exposes the same path as
  `nirs4all-formats scan PATH [--max-depth N] [--include-unsupported] [--json]`.

In-memory reads:

- `open_bytes()` / `open_bytes_with_options()` and `Reader::read_bytes` dispatch
  decoding directly from an in-memory byte slice without touching the
  filesystem. Every single-file reader (CSV, JCAMP, ASD, SED, SIG, SPC, OPUS,
  OMNIC, BUCHI, PerkinElmer, AvantesAscii/Binary, MSA, OceanOptics, JASCO JWS,
  Horiba, Renishaw, TriVista, DigitalSurf, Hamamatsu, WiTec, NumPy, Excel,
  AnIML, AllotropeASM, SiWareAPI, SCiO, USGS, spectral matrix/table, sun
  photometer, mzML, Bruker DPT, ...) implements `read_bytes` directly.
- `open_with_sidecars()` / `open_with_sidecars_and_options()` (M1,
  2026-05-22) decode sidecar-bearing formats from a pure in-memory map:
  ENVI SLI, ENVI Standard, AVIRIS/ERDAS LAN, FGI HDF5+XML, generic HDF5
  (with HDF5 external-file/external-link routing wired through the
  resolver), MATLAB v7.3 and MATLAB Indian Pines (`indian_pines_gt.mat`
  sidecar), ARM MFRSR NetCDF (`<stem>.yaml` QC sidecar) and Allotrope
  ADF all support the new flow. See `docs/dev/SIDECAR_RESOLVER.md` for
  the API surface.
- `open_bytes` keeps refusing sidecar-bearing formats explicitly with
  `Error::UnsupportedSidecar` instead of a generic "does not support
  in-memory reads" string; bindings detect the refusal and route through
  `open_with_sidecars` (PyO3, R extendr, WASM `openWithSidecars`, the CLI
  `--sidecar key=path` flag). The WASM build now enables `fmt-hdf5` by
  default (the upstream `read_exact_at` wasm fix landed in
  `roteiro-gis/netcdf-rust` PR #46, pulled via a pinned `[patch.crates-io]`
  in `bindings/wasm/Cargo.toml` until `hdf5-reader` 0.5.1 ships), so the
  HDF5-backed sidecar formats (FGI XML+HDF5, NetCDF MFRSR) decode under
  WASM too. `fmt-matlab` and `fmt-parquet` are now enabled in the default
  WASM build.

Python bridge — native PyO3 extension `nirs4all_formats._native` built with
maturin (mixed `python/` + `src/` layout). Falls back to the CLI subprocess
when the native module is not available:

- raw access — `probe_path`, `open_records`, `open_bytes`,
  `open_with_sidecars`, `walk_path` directly via PyO3 (no JSON roundtrip);
- lossless object model — `open_recordset` -> `SpectralRecordSet`, a faithful
  N-dimensional mirror of the Rust `SpectralRecord` (shape/dims/coords,
  metadata, provenance) with `to_xarray()` on each array;
- explicit projections on `SpectralRecordSet` — `to_numpy`, `to_pandas` /
  `to_pandas_long`, `to_sklearn`, `to_torch` and `to_spectrodataset` (each
  signal -> a nirs4all source, provenance preserved in reserved
  `nirs4all_formats.*` metadata columns). Heterogeneous feature axes raise a strict
  projection error; missing signals are NaN-filled.

R bridge — `nirs4allformats_*` functions try a native extendr-api static library
shipped under `bindings/r/nirs4allformats/src/rust/` (built at install time by
`R CMD INSTALL` when Cargo is present) and fall back to the `nirs4all-formats`
CLI when the native symbols are absent:

- `nirs4allformats_open_records`, `nirs4allformats_open_dataset`,
  `nirs4allformats_open_bytes`, `nirs4allformats_open_with_sidecars`,
  `nirs4allformats_probe_path`, `nirs4allformats_walk_path`;
- `matrix`, `data.frame` and optional tibble conversion.

JS / WebAssembly bridge — `bindings/wasm/` crate built with `wasm-pack`
for `target web` / `target nodejs`. Compiles `nirs4all-formats` with `fmt-hdf5`,
`fmt-matlab` and `fmt-parquet` on by default, and exposes:

- `version()`, `features()` (reports `{ hdf5: true, matlab: true,
  parquet: true }`);
- `probeBytes(filename, Uint8Array)` returning the ordered candidate readers;
- `openBytes(filename, Uint8Array)` returning the decoded `SpectralRecord`
  list for every single-file reader, including single-file HDF5/NetCDF.
  Sidecar formats (ENVI Standard, AVIRIS ERDAS LAN) return
  `UnsupportedSidecar`; use `openWithSidecars` below.
- `openWithSidecars(filename, Uint8Array, Record<string, Uint8Array>)` decodes
  ENVI SLI / ENVI Standard / AVIRIS LAN and the HDF5-backed FGI XML+HDF5 and
  NetCDF MFRSR formats under WASM from a pure in-memory payload+sidecars map.

## Last Green Gate

Green locally on 2026-09-03 for the 0.2.9 dependency-security lot: fmt clean,
**292 Rust tests** across 21 suites and workspace clippy
`-D warnings` clean, with **10 focused Excel/AnIML/XML/ProcSpec parser tests**
green. `cargo audit` reports zero
vulnerabilities after removing the three newly detected RustSec findings; four
non-vulnerability warnings remain explicit for transitive `paste`, `anyhow`,
`lru` and `memmap2`. Binding, docs and cross-platform jobs remain delegated to
the candidate CI before publication.

**Deferred to CI** (toolchain absent on this dev box): the R `testthat` suite
(`Rscript`) and the wasm-pack Node smoke/sidecar tests (`clang` is missing, so
`zstd-sys` cannot build for WASM). The Rust WASM target itself and the WASM crate
clippy gate are green.

```bash
. "$HOME/.cargo/env"
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p nirs4all-formats --no-default-features
cargo build -p nirs4all-formats --no-default-features --target wasm32-unknown-unknown
(cd bindings/wasm && cargo clippy --all-targets -- -D warnings)
(cd bindings/python && cargo clippy --all-targets -- -D warnings)
unset CONDA_DEFAULT_ENV CONDA_PREFIX CONDA_PYTHON_EXE
. .venv/bin/activate
(cd bindings/python && maturin develop --release --quiet)
python -m pytest bindings/python/tests/ tools/reverse-lab/tests
Rscript -e 'Sys.setenv(NIRS4ALL_FORMATS_REPO=getwd()); source("bindings/r/nirs4allformats/R/version.R"); source("bindings/r/nirs4allformats/R/io.R"); source("bindings/r/nirs4allformats/R/native.R"); testthat::test_dir("bindings/r/nirs4allformats/tests/testthat")'
(cd bindings/wasm && wasm-pack build --target nodejs --release --out-dir pkg-node)
node bindings/wasm/tests/smoke.js
node bindings/wasm/tests/sidecars.test.js
uv run sphinx-build -W -b html docs docs/_build/html
git diff --check
```

Local-only sample sweep (not in CI): `samples_local/` now also carries the full
MicroNIR `.sam` corpus (120 spectra + CSV), the PerkinElmer MIR `.sp` corpus and the
two FOSS DS-series `.nir` files, all committed encrypted as `samples_local.tar.gz.enc`.

## Next Agent Prompt

Continue from `/home/delete/nirs4all/nirs4all-formats`. Keep Rust as the canonical
core. Do not implement parser logic in Python or R bindings.

Immediate next work:

1. decide whether AVIRIS/Indian Pines sample redistribution terms allow keeping
   the committed `.lan/.spc/.GIS` fixtures in public release artifacts, and
   keep the EHU MATLAB cube path local-only unless redistribution terms change;
2. source large NEON/Specim/HySpex/Headwall scenes so the rectangular ROI and
   sparse `(row, col)` mask paths can be exercised on production-scale cubes;
3. continue the open-reader-backed binary batch in this order: OMNIC `.srsx`
   and high-speed variants beyond local SpectroChemPy samples, redistributable
   BUCHI NIRCal non-null target fixtures and `.cal`/NIRMaster variants;
   (**2026-06-03 DONE for this batch**: native FOSS WinISI `.cal`/`.nir`, Viavi
   MicroNIR `.sam`, and PerkinElmer Spectrum 10 FT-MIR `.sp` trailing blocks —
   real fixtures local-only, CC0 synthetic goldens in CI; still want
   redistributable native fixtures and the FOSS `.DA`/`.eqa` + MicroNIR `.pri`
   variants);
4. **DONE (M2, 2026-05-23)** — external reference-reader conformance
   harness under `tests/conformance/`: `brukeropus` for OPUS,
   `spc-spectra` for SPC, `jcamp` for JCAMP-DX, `spectrolab` (R,
   subprocess-isolated) for SED/SIG, canonical ASM JSON for Allotrope,
   `h5py` for HDF5. Initial run on the committed corpus passes 67 /
   skips 16 / fails 0. Tolerances per format documented in
   `docs/CONFORMANCE.md` and `tests/conformance/tolerances.toml`. The
   `.github/workflows/conformance.yml` placeholder is replaced with a
   real weekly + workflow_dispatch job;
5. **DONE (M1, 2026-05-22)** — sidecar resolver wired into
   `Reader::read_bytes` via the new `SidecarResolver` trait (core) plus
   `FsSidecars`/`InMemorySidecars`/`NoSidecars` implementations. ENVI SLI,
   ENVI Standard, AVIRIS/ERDAS LAN, FGI HDF5+XML, generic HDF5 (incl. HDF5
   external file/link routing), MATLAB v7.3, MATLAB Indian Pines, ARM
   MFRSR NetCDF (QC YAML) and Allotrope ADF all decode from in-memory
   `Map<filename, bytes>` payloads. PyO3, R extendr, WASM (ENVI/ERDAS
   plus HDF5-backed FGI XML+HDF5 and NetCDF MFRSR) and the CLI
   `--sidecar key=path` flag expose the new entry point. Follow-up
   2026-05-23: ParquetReader now overrides `read_bytes` via
   `bytes::Bytes` (commit P1) so the committed `synthetic_nirs.parquet`
   fixture decodes in-memory through `open_bytes`. Follow-up 2026-05-25:
   `fmt-hdf5` is now on by default in the WASM crate — the upstream
   `read_exact_at` wasm fix merged as `roteiro-gis/netcdf-rust` PR #46
   and is pulled through a pinned `[patch.crates-io]` until
   `hdf5-reader` 0.5.1 ships. See `docs/dev/SIDECAR_RESOLVER.md`.
6. **DONE (M3, 2026-05-23)** — heterogeneous JCAMP `LINK` fan-out API
   shipped: one record per child plus `link_parent_id` / `link_index` /
   `link_total` / `link_relation` metadata. Top-level multi-block files
   carry the same metadata. The `XYDATA` line-start X checkpoint
   warning is now the structured
   `jcamp_xydata_x_checkpoint_drift` carrying absolute and relative
   deltas. Sourcing real PEAK TABLE / PEAK ASSIGNMENTS fixtures remains
   open (synthetic-only path retained as the contract).
7. **DONE (M4, 2026-05-23; matrix revised for the V1 RC)** — Phase 6 release
   pipeline shipped. `.github/workflows/release.yml` currently builds Python
   wheels via `cibuildwheel` for manylinux2014 x86_64 and Windows AMD64
   (CPython 3.10-3.13), plus a `maturin sdist` used for macOS and Linux
   aarch64 source installs. It also builds per-OS C ABI
   archives (`nirs4all-formats-capi-<target>.{tar.gz,zip}` with the
   `cbindgen`-generated `nirs4all_formats.h` and both project license
   texts). Tagged releases publish to PyPI via OIDC trusted publishing;
   the separate `release-r.yml` builds and attaches the R source tarballs.
   `workflow_dispatch` provides a dry-run path. See `docs/RELEASE.md` for the tag flow,
   per-wheel capability matrix and rollback procedure. Locally
   validated: the C smoke
   `crates/nirs4all-formats-capi/examples/probe_version.c` builds against
   the release library and prints `0.1.0`.

9. keep `docs/STATUS.md` and `docs/ROADMAP.md` current after each green gate.

10. keep the refreshed root `README.md`, `FORMAT_MATRIX.md` and
    `IMPLEMENTATION_DASHBOARD.md` current after each material format change,
    and continue auditing individual `docs/formats/` pages for
    description, implemented behavior, missing behavior and validation
    status.
