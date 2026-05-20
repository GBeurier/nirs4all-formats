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

1. continue with remaining structured-container samples and adjacency:
   MATLAB/HDF5/NetCDF, then additional JSON/XML exports as fixtures appear;
2. continue the binary/open-reader-backed batch with remaining SPC/JCAMP variants;
3. harden JCAMP beyond current coverage: `PEAK TABLE`, incompatible-axis `LINK` files and stricter checkpoint validation;
4. add direct external reference-reader conformance for OPUS/SPC/JCAMP/SED/SIG/ASM where practical;
5. replace Python/R subprocess transport with native PyO3/C ABI paths;
6. keep `docs/STATUS.md` and `docs/ROADMAP.md` current after each green gate.
