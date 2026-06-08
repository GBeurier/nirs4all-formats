#!/usr/bin/env python3
"""Assemble a single self-contained HTML you can open directly (file://).

Everything the served demo fetches over http — the WebAssembly module, the
no-modules JS glue, the sample files and their manifest — is base64-embedded
into one HTML file with NO module imports and NO fetch calls, so it works by
just double-clicking it. The served version (index.html + ./pkg + ./samples)
keeps working unchanged; both share app.js, which auto-detects the mode.

Usage:
    # 1. build the no-modules wasm bundle the standalone embeds
    wasm-pack build bindings/wasm --target no-modules --release \
        --out-dir demo/pkg-nomod --features console-errors
    # 2. assemble
    python3 demo/build_standalone.py
    # -> demo/nirs4all-formats-demo.html
"""
import base64
import json
import pathlib
import re
import sys

_MIME = {
    ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg",
    ".svg": "image/svg+xml", ".webp": "image/webp", ".gif": "image/gif",
}


def inline_assets(html: str, root: pathlib.Path) -> str:
    """Replace every src="assets/…" with a base64 data: URI so the standalone
    file shows the institutional logos with no fetch (works from file://)."""
    def repl(m: "re.Match[str]") -> str:
        rel = m.group(1)
        f = root / rel
        if not f.exists():
            sys.exit(f"asset missing: {f}")
        mime = _MIME.get(f.suffix.lower(), "application/octet-stream")
        return f'src="data:{mime};base64,{b64(f)}"'

    return re.sub(r'src="(assets/[^"]+)"', repl, html)

HERE = pathlib.Path(__file__).resolve().parent
PKG = HERE / "pkg-nomod"
OUT = HERE / "nirs4all-formats-demo.html"

GLUE = PKG / "nirs4all_formats_wasm.js"
WASM = PKG / "nirs4all_formats_wasm_bg.wasm"


def b64(path: pathlib.Path) -> str:
    return base64.b64encode(path.read_bytes()).decode("ascii")


def main() -> int:
    if not GLUE.exists() or not WASM.exists():
        sys.exit(
            f"missing no-modules build at {PKG}.\n"
            "Run: wasm-pack build bindings/wasm --target no-modules --release "
            "--out-dir demo/pkg-nomod --features console-errors"
        )

    index = (HERE / "index.html").read_text()
    app_js = (HERE / "app.js").read_text()
    glue_js = GLUE.read_text()
    manifest = json.loads((HERE / "samples" / "manifest.json").read_text())

    samples = {}
    for s in manifest["samples"]:
        for name in [s["file"], *s.get("sidecars", [])]:
            f = HERE / "samples" / name
            if not f.exists():
                sys.exit(f"sample missing: {f}")
            samples[name] = b64(f)

    formats_path = HERE / "formats.json"
    formats = json.loads(formats_path.read_text()) if formats_path.exists() else None

    embed = {
        "wasm": b64(WASM),
        "manifest": manifest,
        "samples": samples,
        "formats": formats,
    }
    # </script> can never appear inside base64 or our JSON, but guard anyway.
    embed_js = "window.__N4F_EMBED__ = " + json.dumps(embed) + ";"
    embed_js = embed_js.replace("</", "<\\/")

    blob = (
        '<script>\n/* wasm-bindgen (no-modules) glue */\n' + glue_js + "\n</script>\n"
        '<script>\n/* embedded wasm + samples (base64) */\n' + embed_js + "\n</script>\n"
        '<script>\n/* demo UI */\n' + app_js + "\n</script>\n"
    )

    marker = '<script src="./app.js" defer></script>'
    if marker not in index:
        sys.exit(f"could not find {marker!r} in index.html")
    html = index.replace(marker, blob)
    html = inline_assets(html, HERE)

    OUT.write_text(html)
    mb = OUT.stat().st_size / 1024 / 1024
    print(f"wrote {OUT.relative_to(HERE.parent)} ({mb:.1f} MB) — open it directly in a browser")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
