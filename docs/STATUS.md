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
- spectral matrix exports with one spectrum per row: Foss/WinISI text,
  Metrohm Vision Air CSV and VIAVI MicroNIR CSV fixtures;
- sun photometer channel exports: MFR `.OUT` and Microtops `.TXT` fixtures;
- AnIML spectral XML: spectral `SeriesSet` fixture with wavelength axis,
  absorbance signal and sample target; non-spectral AnIML result documents are
  refused;
- Allotrope ASM JSON: plate-reader spectral data cubes and detector-wavelength
  endpoint readings from committed Benchling allotropy fixtures;
- SiWare API JSON: NeoSpectra-style `measurement.wavelengths` and
  `measurement.absorbance` payloads with predictions mapped to targets;
- NetCDF NIRS datasets: simple `spectra` + `wavelengths` containers using a
  pure-Rust reader; ANDI/MS and weather NetCDF samples are refused as non-NIRS;
- generic HDF5 NIRS datasets: root or nested-group `spectra` + `wavelengths`
  containers using a pure-Rust reader; non-spectral HDF5 samples are refused,
  and the committed FGI HDF5 payload is covered while XML sidecar mapping stays
  pending;
- MATLAB MAT datasets: simple MAT v5 and MATLAB v7.3/HDF5 `X` + `wavelengths`
  + optional `y` datasets; object/DSO `.mat` files and `.RData` are still
  pending dedicated schema mappers;
- Excel workbooks: simple `.xlsx/.xlsm` spectral tables with numeric wavelength
  headers; legacy `.xls` and multi-sheet lab templates remain pending;
- Bruker OPUS DPT ASCII export (`.dpt`);
- Bruker OPUS native binaries, 1D data/status block pairs;
- Avantes AvaSoft ASCII wave tables (`.ttt`, `.trt`, `.tit`, `.tat`) and two-column irradiance export (`.IRR`);
- Avantes AvaSoft legacy binaries (`.TRM`, `.ROH`, `.DRK`, `.REF`) and AvaSoft 8 binaries (`.Raw8`, `.IRR8`);
- ENVI Spectral Library sidecars (`.sli` + `.hdr`), one-band BSQ float32/float64 payloads;
- Ocean Optics / Ocean Insight exports (`.txt`, `.csv`, `.jaz`, `.JazIrrad`, `.Master.Transmission`)
  and `.ProcSpec` ZIP/XML archives; the committed Ocean Optics `.spc` sample is
  covered by the Galactic SPC reader;
- JCAMP-DX `XYDATA=(X++(Y..Y))` with plain AFFN plus PAC/SQZ/DIF/DUP ASDF decoding,
  NMR `NTUPLES` real/imaginary pages, and Ocean Optics `LINK`/`XYPOINTS` blocks;
- EMSA/MAS `.msa` (ISO 22029-style) `XY` and `Y` single-spectrum text files;
- Spectral Evolution SED (`.sed`);
- SVC/GER SIG (`.sig`).
- ASD FieldSpec (`.asd` and ASD binaries with numeric extensions), revisions 1/6/7/8.
- Thermo / Galactic GRAMS SPC (`.spc`, `.SPC`), new little-endian generated-X,
  explicit-X, multi common-X and `-XYXY` directory layouts; old little-endian
  support is limited.
- Thermo Nicolet OMNIC (`.SPA`, `.spg`, `.srs`) single spectra and grouped
  spectra via the reverse-engineered key table, plus TGA/GC `.srs` time-series
  matrices as 2D `y,x` records; rapid-scan/high-speed `.srs` and `.srsx` remain
  pending;
- Perkin Elmer Spectrum / IR (`.sp`) single spectra via the `PEPE` block
  container; `.fsm` Spotlight imaging is detected but out of scope for v1.
- BUCHI NIRCal (`.nir`) `NIRCAL Project File` spectra and wavenumber sections
  for the committed foliar-transfer fixture; non-zero reference properties are
  still pending `targets` extraction.
- JASCO JWS (`.jws`) OLE2 `DataInfo` + `Y-Data` spectra for committed
  single-channel and multi-channel fixtures; semantic channel labels are still
  pending.

Golden-summary conformance exists for the fixtures above under
`crates/nirs4all-io/tests/goldens/`.

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
python -m pip install -e tools/reverse-lab -e "bindings/python[numpy,pandas]"
python -m pytest tools/reverse-lab/tests bindings/python/tests
R CMD INSTALL bindings/r/nirs4allio
Rscript -e 'Sys.setenv(NIRS4ALL_IO_REPO=getwd()); library(nirs4allio); testthat::test_dir("bindings/r/nirs4allio/tests/testthat")'
python -m sphinx -b html docs docs/_build/html
```

## Next Agent Prompt

Continue from `/home/delete/nirs4all/nirs4all-io`. Keep Rust as the canonical
core. Do not implement parser logic in Python or R bindings.

Immediate next work:

1. add dedicated MATLAB struct/DSO and `.RData` dataset mappers for Eigenvector
   Corn, NIR Shootout, SpectroChemPy DSO and prospectr NIRsoil;
2. continue the open-reader-backed binary batch in this order: remaining
   Nicolet OMNIC `.srs/.srsx` variants, BUCHI NIRCal property targets and
   JASCO `.jws` semantic channel labels;
3. add remaining lab-transfer containers and adjacent spectroscopy readers:
   Excel multi-sheet templates, Renishaw WDF and Horiba / JobinYvon LabSpec
   XML/text;
4. harden JCAMP beyond current coverage: `PEAK TABLE`, incompatible-axis `LINK`
   files and stricter checkpoint validation;
5. add direct external reference-reader conformance for OPUS/SPC/JCAMP/SED/SIG/ASM/HDF5 where practical;
6. replace Python/R subprocess transport with native PyO3/C ABI paths;
7. keep `docs/STATUS.md` and `docs/ROADMAP.md` current after each green gate.
