---
orphan: true
---

# Sidecar resolver

`nirs4all-io` decodes spectroscopy files through the `Reader` trait. Some
formats reference more than one file: ENVI Standard cubes need a `.hdr`
header, AVIRIS/ERDAS LAN needs a `.spc` axis (and an optional `.GIS`
ground-truth), FGI HDF5+XML pairs an `.xml` metadata sidecar with an HDF5
payload, MATLAB Indian Pines pulls in `indian_pines_gt.mat`, and the ARM
MFRSR NetCDF reader honours an optional `.yaml` quality-control sidecar.

Before M1 those formats only worked when the companion files were on a
real filesystem. M1 adds a [`SidecarResolver`] contract so any reader
that needs sidecars can fetch them from disk or from an in-memory map.

## API surface

The trait lives in
[`crates/nirs4all-io-core/src/sidecar.rs`](../../crates/nirs4all-io-core/src/sidecar.rs):

```rust
pub trait SidecarResolver: Send + Sync {
    fn read(&self, relative: &Path) -> Result<Vec<u8>>;
    fn contains(&self, relative: &Path) -> bool;
    fn list(&self) -> Vec<PathBuf> { Vec::new() }
}
```

Three concrete impls ship under
[`crates/nirs4all-io/src/sidecars.rs`](../../crates/nirs4all-io/src/sidecars.rs):

| Impl | Use case |
|---|---|
| `FsSidecars` | Reads sidecars from a real directory (the parent of the primary file by default). Used internally by every `read_path` flow. |
| `InMemorySidecars` | Stores sidecars in a `BTreeMap<PathBuf, Vec<u8>>`. Used by `open_with_sidecars`, PyO3, R extendr and WASM bindings, the CLI `--sidecar` option, and the test harness. |
| `NoSidecars` | Errors on every lookup with `Error::UnsupportedSidecar`. Used by `open_bytes` so callers get a clear "needs sidecars" error instead of a confusing "file not found". |

Public registry entry points:

```rust
nirs4all_io::open_with_sidecars(name, bytes, sidecars: Arc<dyn SidecarResolver>)
nirs4all_io::open_with_sidecars_and_options(name, bytes, sidecars, options)
```

Reader trait additions (default implementations forward to existing
methods, so single-file readers are not affected):

```rust
fn sniff_with_sidecars(
    &self, head: &[u8], path: &Path, sidecars: &Arc<dyn SidecarResolver>,
) -> Option<FormatProbe>;

fn read_bytes_with_sidecars(
    &self, name: &Path, bytes: &[u8],
    sidecars: &Arc<dyn SidecarResolver>, options: &ReadOptions,
) -> Result<Vec<SpectralRecord>>;
```

ENVI SLI/Standard overrides `sniff_with_sidecars` because its detection
reads the `.hdr` companion text; without the resolver, sniffing fails in
pure-memory mode.

## HDF5 external files and external links

The pure-Rust `hdf5-reader` crate already exposes
`Hdf5File::from_bytes_with_options` and the `ExternalFileResolver` /
`ExternalLinkResolver` traits. The helper
[`open_hdf5`](../../crates/nirs4all-io/src/readers/hdf5_helpers.rs) wraps
both into an `Arc<SidecarBackedExternal>` so any HDF5 raw-data file or
external link referenced from inside the primary HDF5 container is
served by the same `SidecarResolver` instance the caller supplied. The
NetCDF reader uses the matching `NcFile::from_bytes_with_options`.

The synthetic test fixtures under
`crates/nirs4all-io/tests/fixtures/hdf5_external/` exercise both code
paths (see the matching tests in `tests/sidecars.rs`).

## Format scope (M1)

| Format | Sidecar | Mode covered |
|---|---|---|
| ENVI SLI | `<stem>.hdr` (or vice versa) | Bytes + path |
| ENVI Standard cube | `<stem>.hdr`, optional `<stem>.img/.dat` data hint | Bytes + path |
| AVIRIS / ERDAS LAN | `<stem>.spc` axis, optional `92AV3GT.GIS` | Bytes + path |
| FGI HDF5+XML | `<DataReference path="…">` HDF5 payload | Bytes + path (incl. HDF5 from bytes) |
| Generic HDF5 | none, but supports `ExternalFileResolver`/`ExternalLinkResolver` via the sidecar resolver | Bytes + path |
| MATLAB v7.3 | none (HDF5 from bytes) | Bytes + path |
| MATLAB Indian Pines | `indian_pines_gt.mat` | Bytes + path |
| ARM MFRSR (NetCDF) | `<stem>.yaml` QC sidecar (optional) | Bytes + path |
| Allotrope ADF | none (HDF5 from bytes) | Bytes + path |

`open_bytes` keeps refusing sidecar-bearing formats explicitly — it now
returns `Error::UnsupportedSidecar` instead of the previous "does not
support in-memory reads" string.

## Binding parity

| Binding | New entry point |
|---|---|
| Rust | `open_with_sidecars(name, bytes, Arc<dyn SidecarResolver>)` |
| Python (PyO3) | `nirs4all_io.open_with_sidecars(name, bytes, sidecars: dict[str, bytes])` |
| R (extendr) | `nirs4allio_open_with_sidecars(name, raw_bytes, sidecars = list(name = raw))` |
| WebAssembly | `openWithSidecars(filename: string, primary: Uint8Array, sidecars: Record<string, Uint8Array>)` — ENVI/ERDAS LAN plus HDF5-backed FGI XML+HDF5 and NetCDF MFRSR (`fmt-hdf5` on by default) |
| CLI | `nirs4all-io read-json PATH --bytes-file PATH --sidecar key=path` |

### WASM HDF5 support (F3, resolved 2026-05-25)

`fmt-hdf5` is now on by default in `bindings/wasm/Cargo.toml`, so the
WASM binding decodes generic HDF5, FGI XML+HDF5, NetCDF MFRSR (with its
QC YAML sidecar) and Allotrope ADF. The `features()` JS export reports
`{ hdf5: true, matlab: false, parquet: false }`; `fmt-matlab` and
`fmt-parquet` stay off (no wasm-side need yet).

This was previously blocked because `hdf5-reader` 0.5.0 declared
`read_exact_at` only under `#[cfg(unix)]` / `#[cfg(windows)]` while
`FileStorage::read_range` called it unconditionally (`storage.rs:214`),
so `wasm32-unknown-unknown` failed to link even though we only ever use
`BytesStorage` through `Hdf5File::from_vec_with_options`. The fix adds a
`#[cfg(not(any(unix, windows)))]` fallback returning
`io::ErrorKind::Unsupported`; it was merged upstream as
[`roteiro-gis/netcdf-rust` PR #46](https://github.com/roteiro-gis/netcdf-rust/pull/46)
(merge commit `f0cdd2f0`).

crates.io still publishes only `hdf5-reader` 0.5.0 (the fix is on
`main`, not yet released), so the WASM crate pulls the fixed crate
through a pinned patch in `bindings/wasm/Cargo.toml`:

```toml
[patch.crates-io]
hdf5-reader = { git = "https://github.com/roteiro-gis/netcdf-rust", rev = "f0cdd2f0dbd2c6a036fc18cc4ae58d4072d7cb0b" }
```

**When `hdf5-reader` 0.5.1 lands on crates.io**: drop the
`[patch.crates-io]` block and bump the `hdf5-reader` requirement in
`crates/nirs4all-io/Cargo.toml` to `0.5.1`. No other change is needed —
`fmt-hdf5` is already wired through the WASM crate's own feature.

Validated end-to-end: `wasm-pack build --target nodejs` +
`node tests/sidecars.test.js` decodes the FGI HDF5+XML fixture to 50
records (`provenance.format == "fgi-hdf5-xml"`).
