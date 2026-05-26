# Installation

nirs4all-io is one Rust core with thin bindings. Pick the surface you need —
they all decode through the same readers.

> **Version:** `0.1.0-alpha.0`. The project is in active alpha; APIs are stable
> in shape but may still change. Tagged releases publish Python wheels to PyPI
> and attach C ABI archives to the GitHub release (see [`RELEASE.md`](RELEASE.md)).

## Python

Requires Python 3.10+.

```bash
pip install nirs4all-io
```

The wheel ships the native extension — no Rust toolchain needed. Optional
projections pull their own dependencies via extras:

```bash
pip install "nirs4all-io[numpy,pandas,sklearn,torch]"
```

`to_polars()` needs `polars`, `to_xarray()` needs `xarray`, and
`to_spectrodataset()` needs `nirs4all`; install those alongside as needed.

**From source** (for the latest `main` or local development):

```bash
pip install maturin
cd bindings/python && maturin develop --release
```

## Rust

Add the facade crate to your `Cargo.toml`:

```toml
[dependencies]
nirs4all-io = "0.1.0-alpha.0"
```

Default features bundle the HDF5, MATLAB and Parquet readers. Build a leaner
core, or target `wasm32`, by turning them off:

```bash
cargo build -p nirs4all-io --no-default-features                       # core readers only
cargo build -p nirs4all-io --no-default-features \
  --features fmt-hdf5                                                   # add one back
```

| Feature | Default | Adds |
|---|---|---|
| `fmt-hdf5` | on | HDF5, NetCDF, Allotrope ADF, FGI XML+HDF5 (pure-Rust HDF5 stack) |
| `fmt-matlab` | on | MATLAB v5 / v7.3 and prospectr RData (**requires `fmt-hdf5`**) |
| `fmt-parquet` | on | Arrow / Parquet tables |

## Command-line tool

The CLI binary is `nirs4all-io`:

```bash
cargo install --path crates/nirs4all-io-cli
# or, without installing:
cargo run -p nirs4all-io-cli -- probe path/to/file
```

See the [usage guide](usage.md) and [CLI contract](CLI.md) for commands.

## R

The package is `nirs4allio`. With a Rust toolchain present, `R CMD INSTALL`
builds the native extendr library; without one, it falls back to the
`nirs4all-io` CLI transport.

```bash
R CMD INSTALL bindings/r/nirs4allio
```

If you use the CLI fallback, point `NIRS4ALL_IO_CLI` at a prebuilt binary, or
run from a source checkout where `cargo run -p nirs4all-io-cli` is available.

## WebAssembly / JavaScript

Built with `wasm-pack`. The WASM build compiles `fmt-hdf5` **on** and
`fmt-matlab` / `fmt-parquet` **off**.

```bash
# Browser (ES modules)
wasm-pack build bindings/wasm --target web --release
# Node.js / Bun
wasm-pack build bindings/wasm --target nodejs --release --out-dir pkg-node
```

This produces the `nirs4all-io-wasm` package (JS glue + `.wasm` + TypeScript
typings) under `bindings/wasm/pkg*`.

## C ABI

A small, additive C ABI for embedding or building further bindings:

```bash
cargo build -p nirs4all-io-capi --release
```

`build.rs` regenerates `crates/nirs4all-io-capi/include/nirs4all_io.h` via
cbindgen. Tagged releases also ship per-OS archives bundling the static/shared
library, the header and the license. See the [C ABI page](bindings/capi.md).
