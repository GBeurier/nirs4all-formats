# nirs4all-io

Rust-first low-level readers for NIRS and spectroscopy files, with stable
language bindings and conformance checks against existing reference loaders.

> **Status**: foundation phase. The fixture corpus and format inventory are
> present; the executable Rust workspace, binding skeletons, CI and roadmap are
> now in place. Format readers are added one by one behind conformance gates.

## Direction

`nirs4all-io` follows the same product shape as `pls4all`:

- a single low-level core, implemented in Rust;
- a narrow stable interface for bindings and future C ABI use;
- thin language bindings for Python and R first;
- later bindings for JavaScript/WASM, MATLAB/Octave, Android/JNI, Java, C#,
  Julia, Go, Rust, Ruby, Lua and related targets;
- parity/conformance tests against reference readers when they exist.

The Rust core is the source of truth. Python and R do not reimplement parsers;
they expose native records as idiomatic numpy/pandas/sklearn/torch and R data
structures.

## Repository Layout

```text
crates/
  nirs4all-io-core/   # data model, errors, sniffing contracts
  nirs4all-io/        # Rust facade and reader registry
  nirs4all-io-capi/   # additive C ABI scaffold
  nirs4all-io-cli/    # probe/validation CLI
bindings/
  python/             # Python package skeleton and compatibility helpers
  r/                  # R package skeleton
tools/
  reverse-lab/        # clean-room reverse-engineering helpers
samples/              # fixture corpus and per-format provenance
docs/                 # architecture decisions, roadmap, RTD documentation
tests/                # future cross-crate conformance/adversarial tests
```

## Format Scope

The priority set is documented in [`docs/FORMATS.md`](docs/FORMATS.md). Tier A
starts with CSV/TSV, JCAMP-DX, ASD, SVC/GER, Spectral Evolution, Bruker OPUS,
Galactic SPC, ENVI SLI, Avantes and Excel. Tier B/C formats follow once the
core validation and reverse-engineering workflow is stable.

The operational status lives in
[`docs/FORMAT_MATRIX.md`](docs/FORMAT_MATRIX.md): it tracks variant counts,
validated/partial/planned/blocked states, NIRS coverage, missing impact,
popularity and the exact files still needed from instrument networks. A compact
visual summary is maintained in
[`docs/IMPLEMENTATION_DASHBOARD.md`](docs/IMPLEMENTATION_DASHBOARD.md).

Current matrix snapshot, from the 2026-05-20 documentation gate:

| Scope | Count |
|---|---:|
| Format families tracked | 58 |
| Known variants tracked | 238 |
| Validated variants | 145 |
| Partial variants | 19 |
| Planned variants | 16 |
| Blocked variants | 58 |
| Broadly diffusable format families | 25 |
| Targeted diffusable format families | 14 |
| Non-viable format families until samples/specs arrive | 7 |

Current implementation highlights:

- ASD FieldSpec `.asd` covers revisions 1/6/7/8 for primary spectra and now
  inventories embedded reference, classifier, dependent-variable, calibration,
  audit/signature, footer and padding blocks so remaining reverse-engineering
  work is explicit.
- JCAMP-DX now covers dense `XYDATA`/ASDF, NMR `NTUPLES`, top-level
  multi-block records, Ocean Optics `LINK`/`XYPOINTS`, and top-level sparse
  `PEAK TABLE` / `PEAK ASSIGNMENTS` records, with `XYDATA` line-start X
  checkpoint warnings for malformed blocks.
- Generic HDF5 covers simple spectral schemas, multi-signal groups sharing one
  axis, nested groups, common dataset aliases (`spectra`, `absorbance`,
  `reflectance`, `data`) and unambiguous transposed matrices.
- BUCHI NIRCal `.nir` files expose spectra, wavenumber axes, property targets,
  project identity, replicate metadata and per-spectrum `Spectra Info`
  metadata (GUID, scans/resolution, timestamps, device/serials, cell/option and
  gain/temperature diagnostics when present), with local validation on non-null
  cannabis `CBDA`/`THCA` targets.
- Microtops MAN NetCDF reads the real PANGAEA MSM114/2 AOT fixture through
  schema discovery plus a generic `DataLayout::Contiguous` fallback when the
  current pure-Rust HDF5 stack cannot resolve NetCDF4 shared attributes.
- Spectral Evolution `.sed` keeps DN-only files loadable while typing DN,
  percent/fraction reflectance and promoting instrument/GPS/acquisition
  metadata.
- SVC/GER `.sig` covers the committed PDA, laptop, matched-overlap and
  resampled field fixtures with promoted instrument, foreoptic, detector,
  factor and overlap metadata.
- ENVI Standard and AVIRIS/ERDAS LAN cube readers can emit full pixel spectra,
  rectangular row/column ROI windows, or caller-ordered sparse `(row, col)`
  pixel masks from the Rust API and CLI.
- WiTec `WIT_PR06` TDGraph maps are decoded experimentally for the committed
  `Sa4.wip` fixture, including Raman-shift axis derivation and physical map
  coordinate metadata; unknown WiTec project layouts are still refused
  explicitly.
- Avantes AvaSoft legacy and AvaSoft 8 binaries promote `measurement_mode`,
  `point_count`, pixel range, integration time/averages, instrument serial,
  operator, detector temperature (legacy) and acquisition date/time (AvaSoft
  8) at the record top level while preserving the raw vendor block under
  `metadata.avantes`; IRR8 mode now exposes the per-pixel calibration vector
  as `irradiance_calibration` and a mode/extension mismatch warning is raised
  when a `.IRR8`/`.Raw8` file disagrees with its `measure_mode`.

## Adding Or Validating A Format

Each new reader or fixture should update the same public trail:

1. add or place the sample under `samples/` when redistributable, or
   `samples_local/` when private/local-only;
2. document source, license and parser hints in the relevant sample README;
3. add or update the format page under `docs/formats/`;
4. update `docs/FORMAT_MATRIX.md` and, when the status changes materially,
   `docs/IMPLEMENTATION_DASHBOARD.md`;
5. add probe/read/golden tests and any reference-reader comparison that is
   legally usable;
6. run the green gate listed in `docs/STATUS.md` before committing.

## Development

```bash
. "$HOME/.cargo/env"
cargo test --workspace
cargo run -p nirs4all-io-cli -- probe samples/jcamp_dx/TESTSPEC.DX
python -m pip install -e tools/reverse-lab -e bindings/python
python -m pytest tools/reverse-lab/tests bindings/python/tests
```

The canonical plan is [`docs/ROADMAP.md`](docs/ROADMAP.md). Current progress is
tracked in [`docs/STATUS.md`](docs/STATUS.md).

## Non-goals

- no chemometrics or modelling algorithms here;
- no GUI;
- no parser implementation inside language bindings;
- no direct import of GPL reference readers into the MIT runtime path.

## License

MIT. Fixture licenses are documented per format under `samples/`. GPL reference
readers can be used for conformance through isolated subprocesses, not linked
or imported into the runtime core.
