#!/usr/bin/env bash
# Build the WebAssembly bundle the demo loads from `./pkg/`.
#
#   ./demo/build.sh            # build pkg/ and the no-server standalone HTML
#   ./demo/build.sh --serve    # optionally test the deployable static bundle
#
# The browser-first artifact is demo/nirs4all-formats-demo.html: one file with
# the WASM, JS and samples embedded, openable from file:// with no server.
# The multi-file index.html + app.js + pkg/ bundle is kept for GitHub Pages.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/.." && pwd)"

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "error: wasm-pack not found. Install it: https://rustwasm.github.io/wasm-pack/installer/" >&2
  exit 1
fi

echo "▸ generating formats table → demo/formats.json"
python3 "$here/gen_formats.py"

echo "▸ building nirs4all-formats-wasm (web target) → demo/pkg"
wasm-pack build "$repo/bindings/wasm" \
  --target web --release \
  --out-dir "$here/pkg" \
  --features console-errors

echo "✓ pkg/ ready ($(du -h "$here/pkg/nirs4all_formats_wasm_bg.wasm" | cut -f1) wasm)"

echo "▸ building standalone single-file HTML (no-modules target) → demo/nirs4all-formats-demo.html"
wasm-pack build "$repo/bindings/wasm" \
  --target no-modules --release \
  --out-dir "$here/pkg-nomod" \
  --features console-errors
python3 "$here/build_standalone.py"

if [[ "${1:-}" == "--serve" ]]; then
  echo "▸ serving http://localhost:8000/  (Ctrl-C to stop)"
  exec python3 -m http.server 8000 --directory "$here"
else
  echo
  echo "Open the no-server demo directly:"
  echo "  demo/nirs4all-formats-demo.html"
  echo
  echo "Optional static bundle test:"
  echo "  ./demo/build.sh --serve"
fi
