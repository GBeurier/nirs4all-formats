# Contributing to nirs4all-formats

Thanks for your interest in improving nirs4all-formats — the Rust-first, low-level
reader library for NIRS and spectroscopy file formats.

There are two ways to help, and **you do not need to write code for either**:

- **Open an issue** — request a format, share sample files, report a bug, or
  ask for a binding.
- **Send a pull request** — add or harden a reader, fix a bug, or improve the
  docs.

## Open an issue (no code needed)

Use the [issue forms](https://github.com/GBeurier/nirs4all-formats/issues/new/choose):

| You want to… | Use the form |
|---|---|
| Ask for a format we don't read yet | **📡 Request a new format** |
| Share real files so we can build/validate a reader | **🧪 Provide reference files / samples** |
| Report a file that is misread, refused, or crashes | **🐞 Report a reader / library error** |
| Ask for a binding in another language | **🔌 Request a language binding** |

Before opening an issue, please skim the
[supported-format catalogue](docs/SUPPORTED_FORMATS.md): your format may already
be covered, sometimes through a generic CSV/Excel reader.

### Sharing sample files

Real reference files are the single biggest unblocker. The most useful bundle
contains, for the same scan:

- the **original raw file** (in its native format, not converted);
- a **human-readable export** from the vendor software (CSV / TXT / XLSX /
  JCAMP-DX…) so we can cross-check decoded values;
- the **instrument model** and **software version**;
- the **measurement mode** (raw counts, absorbance, reflectance, transmittance,
  radiance, irradiance);
- a few **control values** (e.g. first/last wavelength and value).

Data may be anonymised — keep the original format and the structural metadata
intact. Only share files you have the right to share. Redistributable fixtures
live under [`samples/`](samples/) with their source and license recorded;
private files can be kept local under `samples_local/`.

## The one rule that shapes everything: parsers live only in Rust

The Rust core (`crates/`) is the single source of truth. Bindings
(`bindings/python`, `bindings/r`, `bindings/wasm`) and the C ABI **must not**
reimplement any format parsing — they decode through the registry and convert
the resulting `SpectralRecord`s into idiomatic shapes. **A new format is always
a new Rust reader.**

Non-goals: no chemometrics/modelling, no GUI, no parser logic in bindings, and
no GPL reference reader linked into the MIT runtime core (GPL readers are used
only for conformance, isolated behind subprocesses).

## Send a pull request

### Set up

```bash
. "$HOME/.cargo/env"
cargo test --workspace
cargo run -p nirs4all-formats-cli -- probe samples/jcamp_dx/TESTSPEC.DX
```

### Adding or validating a format

A reader is **not** accepted just because it parses one file. Acceptance
requires the sniffer, normalized output, metadata, provenance, warnings,
adversarial behaviour (truncation/corruption) **and** a reference-loader
comparison, all documented and tested. The trail to follow:

1. Place the sample under `samples/` (redistributable) or `samples_local/`
   (private), and record its source and license in the relevant sample README.
2. Create the reader at `crates/nirs4all-formats/src/readers/<fmt>.rs`, wire it into
   `readers/mod.rs` (`pub mod` + `pub use`, with `#[cfg(feature = "…")]` if it is
   HDF5/MATLAB/Parquet-backed), and register it in the `readers()` list in
   `registry.rs`.
3. Add or update the format page under [`docs/formats/`](docs/formats/) and the
   [`docs/SUPPORTED_FORMATS.md`](docs/SUPPORTED_FORMATS.md) catalogue.
4. Update [`docs/FORMAT_MATRIX.md`](docs/FORMAT_MATRIX.md) (and
   [`docs/IMPLEMENTATION_DASHBOARD.md`](docs/IMPLEMENTATION_DASHBOARD.md) when
   the status changes materially).
5. Add probe/read/golden tests and any reference-reader comparison that is
   legally usable. See [`docs/CONFORMANCE.md`](docs/CONFORMANCE.md).

### Before you commit: the green gate

The authoritative pre-commit sequence lives in
[`docs/STATUS.md`](docs/STATUS.md) under *Last Green Gate* (fmt, test, clippy,
no-default-features build, wasm build, per-binding clippy, Python/R/WASM tests,
docs build, `git diff --check`). CI (`.github/workflows/ci.yml`) mirrors a
subset. Run the green gate locally and update `docs/STATUS.md` after it passes.

Golden summaries are re-blessed after a reviewed change with:

```bash
NIRS4ALL_FORMATS_ACCEPT_GOLDENS=1 cargo test -p nirs4all-formats --test goldens
```

## Code style

- `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`
  must be clean (warnings are errors).
- Keep the core dependency-light; gate heavy decoders (HDF5, MATLAB, Parquet)
  behind their Cargo features so the no-default-features and `wasm32` builds
  keep working.

## License

nirs4all-formats is MIT licensed. By contributing, you agree that your contributions
are licensed under the same terms. Fixture licenses are documented per format
under `samples/`.
