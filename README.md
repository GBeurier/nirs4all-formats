# nirs4all-io

Rust-first, low-level readers for **NIRS and spectroscopy file formats**, with
stable Python / R / WebAssembly / C bindings and conformance checks against
reference loaders.

> **Status:** active alpha (`0.1.0-alpha.0`). Over 40 native readers across ~58
> format families are implemented and tested on a committed fixture corpus;
> APIs are stable in shape but may still change before 1.0.

`nirs4all-io` turns the messy zoo of vendor spectroscopy files into one clean,
provenance-tracked data model. It does **no** chemometrics or modelling — it
produces the records that a modelling library such as
[`nirs4all`](https://github.com/GBeurier/nirs4all) consumes.

## Why

- **One API, any format.** `open_path` / `open_bytes` sniff each file by content
  (magic bytes, container schema, text shape) and route it to the right reader —
  you never pick one by hand.
- **Lossless, canonical records.** Every reader emits `SpectralRecord`s with
  per-signal axes, metadata, provenance (source SHA-256, reader version) and
  explicit quality flags. Nothing is resampled or silently merged.
- **The Rust core is the single source of truth.** Bindings decode through the
  same registry and only convert the result, so a file reads identically from
  Rust, Python, R, the CLI or the browser.

## Quick start

```bash
# Command line (binary: nirs4all-io)
nirs4all-io probe     samples/jcamp_dx/TESTSPEC.DX   # which reader, and why?
nirs4all-io read-json samples/jcamp_dx/TESTSPEC.DX   # decode to JSON records
nirs4all-io scan      samples/ --json                # walk a directory
```

```python
import nirs4all_io as nio

records = nio.open_recordset("spectrum.sed")     # lossless object model
X, axis = records.to_numpy(signal="reflectance") # modelling-ready matrix
```

See [Getting started](docs/getting_started.md) and the
[usage guide](docs/usage.md) for more.

## Supported formats

Over 40 readers cover delimited/Excel tables, Bruker OPUS, Thermo SPC & Nicolet
OMNIC, JCAMP-DX, ASD FieldSpec, Spectral Evolution, SVC/GER, Avantes, Ocean
Optics, ENVI/AVIRIS hyperspectral cubes, HDF5/NetCDF/MATLAB, Allotrope, AnIML,
Raman formats (Renishaw, Horiba, WiTec, TriVista) and more.

- **[Supported-format catalogue](docs/SUPPORTED_FORMATS.md)** — the public list
  with per-format pages, vendors, extensions and support status.
- [`docs/FORMAT_MATRIX.md`](docs/FORMAT_MATRIX.md) /
  [`docs/IMPLEMENTATION_DASHBOARD.md`](docs/IMPLEMENTATION_DASHBOARD.md) — the
  internal, variant-by-variant tracking.

## Bindings

| Binding | Package | Notes |
|---|---|---|
| Python | `nirs4all-io` (import `nirs4all_io`) | numpy / pandas / polars / sklearn / torch / xarray / `SpectroDataset` projections. [Docs](docs/bindings/python.md) |
| R | `nirs4allio` | matrix, `data.frame`, tibble; native extendr or CLI fallback. [Docs](docs/bindings/r.md) |
| WebAssembly / JS | `nirs4all-io-wasm` | in-browser sniffing + decoding (`fmt-hdf5` on). [Docs](docs/bindings/wasm.md) |
| C ABI | `nirs4all-io-capi` | additive C ABI + generated header, the base for further bindings. [Docs](docs/bindings/capi.md) |

## Installation

```bash
pip install nirs4all-io            # Python 3.10+ (extras: numpy,pandas,sklearn,torch)
cargo add nirs4all-io              # Rust
R CMD INSTALL bindings/r/nirs4allio  # R
```

Full per-language instructions, feature flags and build-from-source steps are in
[docs/installation.md](docs/installation.md).

## Repository layout

```text
crates/
  nirs4all-io-core/   # data model, errors, sniffing contracts
  nirs4all-io/        # reader registry + 40+ native readers + directory walker
  nirs4all-io-capi/   # additive C ABI (cbindgen-generated header)
  nirs4all-io-cli/    # probe / read-json / scan
bindings/
  python/  r/  wasm/  # thin language bindings (no parser logic)
tools/reverse-lab/    # clean-room reverse-engineering helpers
samples/              # redistributable fixture corpus + per-format provenance
docs/                 # this documentation (Sphinx / MyST)
tests/                # cross-crate conformance & adversarial tests
```

## Contributing

The fastest way to help is to **request a format**, **send reference files**,
**report a misread file**, or **request a binding** through the
[issue templates](https://github.com/GBeurier/nirs4all-io/issues/new/choose).
Real sample files are the single biggest unblocker. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the full guide and the rule that shapes
everything: **parsers live only in the Rust core**.

## Non-goals

- no chemometrics or modelling algorithms here;
- no GUI;
- no parser logic inside language bindings;
- no GPL reference reader linked into the MIT runtime core (GPL readers are used
  only for conformance, isolated behind subprocesses).

## License

MIT. Fixture licenses are documented per format under `samples/`.
