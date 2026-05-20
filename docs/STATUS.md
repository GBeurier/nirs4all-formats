# Project Status

Last updated: 2026-05-20.

## Current Checkpoint

Phase 0 is being established:

- format inventory and fixture corpus already exist;
- Rust workspace has been scaffolded;
- Python and R binding skeletons exist;
- reverse-engineering helper package exists;
- GitHub Actions and RTD configuration are being added;
- no production reader has reached Experimental yet.

## Last Green Gate

Green locally on 2026-05-20:

```bash
. "$HOME/.cargo/env"
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python -m pip install -e tools/reverse-lab -e bindings/python
python -m pytest tools/reverse-lab/tests bindings/python/tests
python -m sphinx -b html docs docs/_build/html
cargo run -p nirs4all-io-cli -- probe samples/jcamp_dx/TESTSPEC.DX
```

## Next Agent Prompt

Continue from `/home/delete/nirs4all/nirs4all-io`. Keep Rust as the canonical
core. Do not implement parser logic in Python or R bindings. Advance Phase 1:

1. harden the reader trait and registry;
2. implement bounded file reads and archive policy;
3. add delimited text, Bruker DPT and Avantes ASCII readers;
4. create golden JSON generation and validation;
5. add conformance notes under `docs/formats/`;
6. keep `docs/STATUS.md` and `docs/ROADMAP.md` current after each gate.
