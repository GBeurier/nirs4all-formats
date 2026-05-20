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
- Bruker OPUS DPT ASCII export (`.dpt`);
- Avantes AvaSoft ASCII wave tables (`.ttt`, `.trt`, `.tit`, `.tat`) and two-column irradiance export (`.IRR`);
- JCAMP-DX plain AFFN `XYDATA=(X++(Y..Y))`;
- Spectral Evolution SED (`.sed`);
- SVC/GER SIG (`.sig`).

Golden-summary conformance exists for the seven fixtures above under
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
```

## Next Agent Prompt

Continue from `/home/delete/nirs4all/nirs4all-io`. Keep Rust as the canonical
core. Do not implement parser logic in Python or R bindings.

Immediate next work:

1. harden JCAMP beyond plain AFFN: DIF/DUP, SQZ/PAC and NTUPLES;
2. start the binary/open-reader-backed batch: ASD and Galactic SPC;
3. add direct external reference-reader conformance for JCAMP/SED/SIG where practical;
4. replace Python/R subprocess transport with native PyO3/C ABI paths;
5. keep `docs/STATUS.md` and `docs/ROADMAP.md` current after each green gate.
