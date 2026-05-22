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

The WASM build exposes both the sniffer surface (`probeBytes`) and the full
decoding surface (`openBytes`). Every single-file reader in `nirs4all-io`
(CSV, JCAMP, ASD, SED, SIG, SPC, OPUS, OMNIC, BUCHI, PerkinElmer, Avantes,
MSA, OceanOptics, JASCO JWS, Horiba, Renishaw, TriVista, DigitalSurf,
Hamamatsu, WiTec, NumPy, Excel, AnIML, AllotropeASM, SiWareAPI, SCiO, USGS,
spectral matrix/table, sun photometer, mzML, Bruker DPT) implements
`Reader::read_bytes` directly and therefore works through `openBytes`.

Heavy native-deps readers (HDF5, NetCDF, Allotrope ADF, FGI HDF5+XML, MATLAB,
Parquet) cannot cross-compile to `wasm32-unknown-unknown` because their
underlying C libraries (libhdf5, libzstd, liblzma) lack a wasm backend in the
current dependency tree. They are gated behind the `fmt-hdf5`, `fmt-matlab`
and `fmt-parquet` Cargo features on `nirs4all-io` and remain off here.

Multi-file formats that materialize sidecars (ENVI Standard `.hdr` + `.img`,
AVIRIS ERDAS `.lan` + `.spc` + `.GIS`, FGI HDF5+XML) still require a real
filesystem and return a descriptive error from `openBytes`. Supporting them
in WASM is follow-up work and needs a `SidecarResolver` callback so the
browser side can supply each related file as `Map<filename, bytes>`.

## Smoke test

```bash
node bindings/wasm/tests/smoke.js
```

The test loads four committed fixtures (CSV, JCAMP-DX, ASD binary, a non-data
PDF) and asserts that the WASM probe routes each one to the expected reader.
