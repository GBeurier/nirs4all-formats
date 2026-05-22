# nirs4all-io WebAssembly binding

Browser-friendly bridge to the format sniffers of `nirs4all-io`.

## Build

```bash
# Browser (ES modules)
wasm-pack build bindings/wasm --target web --release

# Node.js / Bun
wasm-pack build bindings/wasm --target nodejs --release --out-dir pkg-node
```

## Surface

```ts
import init, { version, features, probeBytes } from './pkg/nirs4all_io_wasm.js';

await init();

console.log(version());                // "0.1.0-alpha.0"
console.log(features());               // { hdf5: false, matlab: false, parquet: false }

const bytes = new Uint8Array(await file.arrayBuffer());
const probes = probeBytes(file.name, bytes);
// [{ format: 'jcamp-dx', reader: '...', confidence: 'definite', reason: '...' }]
```

## Current scope

The WASM build compiles only the byte-based sniffer surface
(`builtin_probes`). It returns the ordered list of candidate readers for a
file given its name and bytes, without running the full reader pipeline.

Heavy native-deps readers (HDF5, NetCDF, Allotrope ADF, FGI HDF5+XML, MATLAB,
Parquet) cannot cross-compile to `wasm32-unknown-unknown` because their
underlying C libraries (libhdf5, libzstd, liblzma) lack a wasm backend in the
current dependency tree. They are gated behind the `fmt-hdf5`, `fmt-matlab`
and `fmt-parquet` Cargo features on `nirs4all-io` and remain off here.

Full file decoding from WASM is follow-up work: it requires either an
in-WASM virtual filesystem layer or a `Reader::read_bytes` entry point on the
core readers so multi-file (sidecar) formats can be passed as `Map<filename,
bytes>`.

## Smoke test

```bash
node bindings/wasm/tests/smoke.js
```

The test loads four committed fixtures (CSV, JCAMP-DX, ASD binary, a non-data
PDF) and asserts that the WASM probe routes each one to the expected reader.
