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
- Bruker DPT reader; experimental;
- Avantes ASCII exports; experimental;
- golden JSON writer/validator; golden summaries implemented;
- docs for accepted fixture metadata.

Gate:

- no extension-only false positive on known collision fixtures;
- golden JSON includes axis, signal, metadata, provenance and warnings;
- Python `to_numpy_matrix()` and `to_pandas_frame()` work for these records.

## Phase 2: JCAMP-DX

Status: in progress for plain AFFN `XYDATA`; compressed encodings are pending.

Deliverables:

- JCAMP AFFN and XYDATA; plain rows experimental;
- then DIF/DUP and NTUPLES;
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

Status: in progress. Galactic SPC, Bruker OPUS native, Avantes binaries and
ENVI SLI now have experimental readers for their first high-value subsets.

Deliverables:

- Galactic SPC; experimental;
- Bruker OPUS; experimental 1D data/status block pairs;
- Avantes 6/7 and AvaSoft 8; experimental for committed legacy and AVS8 fixtures;
- ENVI SLI sidecar handling; experimental for one-band BSQ spectral libraries;
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
