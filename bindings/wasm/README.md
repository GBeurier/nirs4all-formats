# nirs4all-formats (WebAssembly / JS)

Browser- and Node-friendly bridge to the `nirs4all-formats` Rust core. It runs the
format sniffers **and** the decoders entirely in WebAssembly, from in-memory
bytes — no filesystem required.

## Build

```bash
# Browser (ES modules)
wasm-pack build bindings/wasm --target web --release
# Node.js / Bun
wasm-pack build bindings/wasm --target nodejs --release --out-dir pkg-node
```

This emits the `nirs4all-formats-wasm` package (JS glue + `.wasm` + TypeScript
typings) under `pkg/` (or `pkg-node/`).

## Surface

```ts
import init, {
  version, features, probeBytes, openBytes, openWithSidecars,
} from "./pkg/nirs4all_formats_wasm.js";

await init();

version();   // "0.1.0-alpha.0"
features();  // { hdf5: true, matlab: false, parquet: false }

const bytes = new Uint8Array(await file.arrayBuffer());

// Sniff: ordered candidate readers, best first
probeBytes(file.name, bytes);
// [{ format: "jcamp-dx", reader: "...", confidence: "definite", reason: "..." }]

// Decode: SpectralRecord[] (same JSON shape as `nirs4all-formats read-json`)
const records = openBytes(file.name, bytes);
```

`openBytes` / `probeBytes` take the file **name** (several sniffers
disambiguate by extension) plus the bytes. The returned records match the
[data model](../../docs/DATA_MODEL.md): `signals`, `signal_type`, `targets`,
`metadata`, `provenance`, `quality_flags`.

### Sidecar formats

Multi-file formats (ENVI Standard `.img`+`.hdr`, ENVI SLI, AVIRIS/ERDAS LAN,
FGI XML+HDF5, NetCDF MFRSR) return an `UnsupportedSidecar` error from
`openBytes`. Supply the companions as a `{ name: Uint8Array }` map instead:

```ts
const records = openWithSidecars("cube.img", imgBytes, { "cube.hdr": hdrBytes });
```

## Scope & feature flags

The WASM build compiles `fmt-hdf5` **on** (pure-Rust HDF5/NetCDF decoders, so
generic HDF5, FGI XML+HDF5, NetCDF MFRSR and Allotrope ADF work) and
`fmt-matlab` / `fmt-parquet` **off** (their C dependencies have no wasm
backend). Call `features()` to check at runtime.

## Smoke test

```bash
node bindings/wasm/tests/smoke.js
node bindings/wasm/tests/sidecars.test.js
```

`smoke.js` loads committed fixtures (CSV, JCAMP-DX, ASD binary, a non-data PDF)
and asserts the probe routes each to the expected reader; `sidecars.test.js`
exercises `openWithSidecars`.
