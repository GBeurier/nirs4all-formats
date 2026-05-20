# Roadmap

`nirs4all-io` is developed in gated phases. Each phase must leave tests,
documentation and review notes behind.

## Phase 0: Foundation

Status: in progress.

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

Deliverables:

- strict `Reader` trait and registry behavior;
- bounded readers and archive policy;
- delimited text reader;
- Bruker DPT reader;
- Avantes ASCII exports;
- golden JSON writer/validator;
- docs for accepted fixture metadata.

Gate:

- no extension-only false positive on known collision fixtures;
- golden JSON includes axis, signal, metadata, provenance and warnings;
- Python `to_numpy_matrix()` and `to_pandas_frame()` work for these records.

## Phase 2: JCAMP-DX

Deliverables:

- JCAMP AFFN and XYDATA;
- then DIF/DUP and NTUPLES;
- conformance against open JCAMP readers where possible;
- adversarial tests for malformed label-data records and compressed archives.

## Phase 3: Field Spectrometers

Deliverables:

- Spectral Evolution `.sed`;
- SVC/GER `.sig`;
- ASD `.asd`;
- first full metadata/PII redaction policy implementation.

## Phase 4: First Bindings

Starts after three readers reach `Done`.

Deliverables:

- native Python binding backed by Rust;
- numpy and pandas exports;
- sklearn-compatible dataset provider;
- torch dataset adapter;
- R package backed by Rust/C ABI;
- R matrix/data.frame/tibble-compatible exports;
- cross-binding fixtures for every `Done` reader.

## Phase 5: Binary Reader Batch

Deliverables:

- Galactic SPC;
- Bruker OPUS;
- Avantes 6/7 and AvaSoft 8;
- ENVI SLI sidecar handling;
- performance dashboards for large file and many-small-file scenarios.

## Phase 6: Packaging and Deployment

Deliverables:

- PyPI wheels through GitHub Actions;
- local R build/release procedure;
- C ABI headers and release archives;
- docs for deployment matrix and supported platforms.

## Phase 7+: Continuous Format Expansion

Add one family at a time:

- Nicolet OMNIC, Perkin Elmer, BUCHI NIRCal, OceanView ProcSpec, JASCO;
- Foss/Metrohm/Perten native formats as fixtures and reverse-engineering
  evidence become available;
- adjacent formats only when they help disambiguation or user workflows.

Every new format repeats the same lifecycle: Experimental -> Beta -> Done.
