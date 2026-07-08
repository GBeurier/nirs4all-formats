# WebAssembly / JS Binding

The WASM binding runs the `nirs4all-formats` sniffers and decoders in the browser or
in Node, entirely from in-memory bytes. Parser logic stays in the Rust core; the
binding only serialises records to plain JS objects.

## Build & package

Built with `wasm-pack`:

```bash
wasm-pack build bindings/wasm --target web    --release                  # browser (ES modules)
wasm-pack build bindings/wasm --target nodejs --release --out-dir pkg-node  # Node / Bun
```

The published npm package is `@nirs4all/formats-wasm`. Local `wasm-pack`
builds still emit JS glue, the `.wasm` binary and generated TypeScript typings
under `pkg/` / `pkg-node/`.

## API

| Function | Signature | Returns |
|---|---|---|
| `version()` | `() => string` | Crate version, e.g. `"0.2.6"`. |
| `features()` | `() => { hdf5: boolean, matlab: boolean, parquet: boolean }` | Which format features this bundle was compiled with. |
| `probeBytes(filename, bytes)` | `(string, Uint8Array) => Probe[]` | Ordered candidate readers (best first). |
| `openBytes(filename, bytes)` | `(string, Uint8Array) => SpectralRecord[]` | Decoded records (single-file formats). |
| `openWithSidecars(filename, bytes, sidecars)` | `(string, Uint8Array, Record<string, Uint8Array>) => SpectralRecord[]` | Decoded records for sidecar-bearing formats. |

`init()` (the default export) must be awaited once before any call.

The `filename` argument is required because several sniffers disambiguate by
extension (`.lan`, `.spc`, `.hdr`, …). Only the first 8 KB of `bytes` is used for
probing; pass the whole buffer or just the head.

### Shapes

- **Probe** — `{ format, reader, confidence, reason }`.
- **SpectralRecord** — the same JSON shape as `nirs4all-formats read-json`:
  `{ signals, signal_type, targets, metadata, provenance, quality_flags }`. See
  the [data model](../DATA_MODEL.md). (JSON cannot represent `NaN`/`Inf`; use the
  native or Python path when signal values may be non-finite.)

## Example

```ts
import init, { version, features, probeBytes, openBytes, openWithSidecars }
  from "@nirs4all/formats-wasm";

await init();
console.log(version(), features());

const bytes = new Uint8Array(await file.arrayBuffer());
const candidates = probeBytes(file.name, bytes);

try {
  const records = openBytes(file.name, bytes);
  // ... use records[0].signals, .metadata, .provenance
} catch (err) {
  // Sidecar formats throw UnsupportedSidecar — supply companions instead:
  const records = openWithSidecars("cube.img", imgBytes, { "cube.hdr": hdrBytes });
}
```

## Scope

The WASM build compiles `fmt-hdf5`, `fmt-matlab` and `fmt-parquet` **on**, so
single-file HDF5/NetCDF payloads, MATLAB MAT / RData, Parquet tables and the
HDF5-backed sidecar formats (FGI XML+HDF5, NetCDF MFRSR, Allotrope ADF) all
decode in the browser. The core uses browser-compatible compression
(`lzma-rs` for RData, snappy/zstd for Parquet) and a pure-Rust HDF5 stack, so no
C dependency is needed. Check `features()` at runtime to confirm what a given
bundle was compiled with.

Sidecar formats (ENVI Standard, ENVI SLI, AVIRIS/ERDAS LAN, FGI XML+HDF5,
NetCDF MFRSR) return `UnsupportedSidecar` from `openBytes`; route them through
`openWithSidecars` with a `{ name: Uint8Array }` map of companion files.

## Tests

```bash
node bindings/wasm/tests/smoke.js
node bindings/wasm/tests/sidecars.test.js
```
