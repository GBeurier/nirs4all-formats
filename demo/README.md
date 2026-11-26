# nirs4all-formats — in-browser demo

A single static page that decodes spectroscopy files **entirely in your
browser**. Drop a vendor file and the `nirs4all-formats` Rust reader registry —
compiled to WebAssembly — sniffs the format, decodes the spectra, and renders
the axes, signal channels, metadata and provenance. Nothing is uploaded; there
is no backend.

Live: **https://formats.nirs4all.org/**

## What it shows

- **Format detection** — the ordered sniffer candidates with confidence and the
  reason each reader matched.
- **Interactive spectral plot** — every record overlaid, with a crosshair
  readout (x value + unit, y value), axis-aware orientation (wavenumber/cm⁻¹
  drawn high→low, wavelength/nm low→high), and per-channel switching.
- **Signal channels** — role, source, type, dims/shape and per-signal axis.
- **Provenance** — reader name/version, schema version, source SHA-256, and the
  reader warnings exactly as the Rust core emits them.
- **Metadata & targets** — the decoded instrument metadata as a collapsible tree.
- **File workspace** — the header names the file and holds the **Export** menu;
  the properties (format, signals, axes, records, provenance, metadata, targets)
  come first, then the spectra with the **Convert** panel beside them (stacked
  under the chart on narrow screens).
- **Convert** — every control has a label and a short help text. The Rust planner
  (`planConversion`) states what will happen in plain language, asks what the file
  does not say (for example percent or fraction) as inline choices, and refuses,
  with a fix, what cannot be converted. **Convert** runs the steps with
  nirs4all-methods (`vendor/methods/`, loaded only when a conversion runs) and adds
  the result as a derived signal. Method ids and parameters stay in a collapsed
  *Technical details* block; the limits in a collapsed *Limits* block.
- **Original / Converted / Both** — after a conversion the chart shows the original,
  the result, or both overlaid on one chart (`mountSpectraCompare` from nirs4all-ui):
  one shared x-axis and a second y-axis when the units differ. When the axis unit
  changed (cm⁻¹ → nm), the original is drawn on the converted axis computed by the
  same Methods `AxisUnitConversion` step; the page does no unit arithmetic.
- **Export** — the menu groups the targets by the `maturity` the Formats writer
  catalogue declares: *Standard formats* (CSV, Excel, JCAMP-DX, Galactic SPC, JSON
  records, and a ZIP of every signal as CSV), *Experimental* (the reverse-engineered
  FOSS WinISI and NSAS layouts) and *Other formats* (two-column text, OPUS DPT).
  Software presets sit in the group of their format. Choosing a target plans it:
  needed conversions run first (questions in a small dialog) and stay on the page,
  then the Rust writer produces the file and the page lists the conversions and
  the losses. A signal converted in the side panel is the one exported.
- **Download view** — writes exactly the spectra drawn in the chart (one signal, or
  original and converted in the overlay, the original on the converted axis when
  the unit changed) as CSV or JSON records, through the same Rust writers.
- **Library** — one tabbed example per language (Python, R, Rust,
  JavaScript/WASM, CLI, C, Java, C#, Julia and Docker), each with its registry and
  install command, showing how to read a file, reach the spectra matrix, axis,
  targets and metadata, and export or plan a conversion. The examples run against
  this repository's bindings.
- **Format catalogue** — sortable by name, coverage and status from the column
  headers.

The single-file build does not include nirs4all-methods, so conversions (and
exports that need one) need the served page.

## Run It Without A Server

The default local demo is the single-file build:

`build.sh` produces **`demo/nirs4all-formats-demo.html`** — one
self-contained file with the WebAssembly module, the JS and every sample
base64-embedded. Just **double-click it** (works from a `file://` path; no
server, no fetch, no modules).

```bash
./demo/build.sh
# → open demo/nirs4all-formats-demo.html in any browser
```

Build only the standalone:

```bash
wasm-pack build bindings/wasm --target no-modules --release \
  --out-dir demo/pkg-nomod --features console-errors
python3 demo/build_standalone.py     # → demo/nirs4all-formats-demo.html (~4.5 MB)
```

## Static Deployment Bundle

The multi-file `index.html` loads the wasm-pack `--target web` bundle as an ES
module, so it must be served over http(s) — module WASM will not load from
`file://`. This bundle is only for static hosting such as GitHub Pages; there is
still no backend and nothing is uploaded.

```bash
./demo/build.sh --serve              # optional local static-hosting check
# → open http://localhost:8000/
```

## Files

| Path | Role |
|---|---|
| `index.html` | Markup + the nirs4all.org design system (inline CSS). |
| `app.js` | UI engine: drag-drop, WASM calls, the canvas spectral plot, rendering. Dual-mode (served ↔ embedded). **No parsing** — that lives only in Rust. |
| `samples/` | Redistributable conformance fixtures + `manifest.json` (the "try a sample" chips). |
| `vendor/methods.version` | The exact `@nirs4all/methods` version the demo uses; the only place to change it. |
| `vendor/methods/` | That version's npm `dist/` (JS modules, `n4m.wasm`, licences), written by `vendor-methods.sh`. |
| `vendor-methods.sh` | Vendors the pinned version with `npm pack`, or a local build's `dist/` for testing (never committed). |
| `vendor/ui/` | The nirs4all-ui browser entry (`nirs4all-ui-browser.iife.js`, `browser.css`) with its licence texts. Copied from the published npm package `nirs4all-ui@0.3.2` (`dist/browser/`), which provides `SpectraCompareChart` / `mountSpectraCompare`. |
| `tests/` | Playwright smokes: `shared-ui-smoke`, `page-smoke`, `convert-smoke`, `export-smoke` (top Export and chart download), plus `sidecar-routing`. |
| `pkg/` | `wasm-pack --target web` output for the served version (git-ignored; built by `build.sh` / CI). |
| `pkg-nomod/` | `wasm-pack --target no-modules` output — input to the standalone build (git-ignored). |
| `build_standalone.py` | Base64-embeds the no-modules wasm + samples into the single-file HTML. |
| `nirs4all-formats-demo.html` | The generated single-file build (git-ignored; ~4.5 MB). |
| `build.sh` | Builds `pkg/` + the standalone file, optionally serves. |

`app.js` auto-detects its mode: if `window.__N4F_EMBED__` is present (standalone)
it instantiates the embedded wasm and reads embedded samples; otherwise it lazily
imports `./pkg/` and fetches `./samples/`. The built `pkg*/` and the standalone
HTML are not committed — they are regenerated by `build.sh` / CI.

## Deep links

Append `#sample=<name>` to auto-load a bundled sample, e.g.
`…/#sample=PE1800.DX` — handy for embedding a specific decode.

## Deployment

`.github/workflows/demo-pages.yml` builds both wasm bundles, assembles
`index.html` + `app.js` + `samples/` + `pkg/` **and** the standalone
`nirs4all-formats-demo.html` (linked from the footer as "Single-file ⤓"), then
publishes the `demo/` folder to GitHub Pages. Enable Pages → *Source: GitHub
Actions* once.
