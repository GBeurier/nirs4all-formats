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

# Shared nirs4all-ui browser entry. The release build stages these exact bytes
# (one UI build, one copy per host); this script only embeds them.
UI_DIR = HERE / "vendor" / "ui"
UI_JS = UI_DIR / "nirs4all-ui-browser.iife.js"
UI_CSS = UI_DIR / "browser.css"
UI_JS_MARKER = '<script src="./vendor/ui/nirs4all-ui-browser.iife.js" defer></script>'
UI_CSS_MARKER = '<link rel="stylesheet" href="./vendor/ui/browser.css" />'


def replace_once(html: str, marker: str, replacement: str) -> str:
    """Replace `marker` exactly once; a missing or repeated marker is a build error."""
    n = html.count(marker)
    if n != 1:
        sys.exit(f"expected exactly one {marker!r} in index.html, found {n}")
    # str.replace with a callable-free literal: no regex/backslash interpretation.
    return html.replace(marker, replacement)


def refuse_closers(label: str, text: str, closer: str) -> None:
    """Refuse text that would terminate its inline <script>/<style> early.
    The text is embedded verbatim: rewriting '</' blindly could corrupt regex
    literals, so a hit is a build error to fix at the source."""
    low = text.lower()
    if closer in low:
        sys.exit(f"{label} contains {closer!r}; it cannot be inlined verbatim")
    # '<!--' followed by '<script' switches the HTML parser into its double-escaped
    # script state, where the real '</script>' no longer ends the block.
    if closer == "</script" and re.search(r"<!--.*<script", low, re.S):
        sys.exit(f"{label} contains '<!--' before '<script'; it cannot be inlined verbatim")


def ui_notices() -> str:
    """Licence/notice text staged with the UI bundle (UI and React notices).
    Only FILES among the vendor/ui descendants are read (nested ones included,
    directories ignored): a file named like a notice/licence, or anything under
    LICENSES/. Each is embedded once under its path relative to vendor/ui, in a
    deterministic order; at least one must exist."""
    found = sorted(
        p.relative_to(UI_DIR).as_posix() for p in UI_DIR.rglob("*")
        if p.is_file() and (
            any(t in p.name.upper() for t in ("NOTICE", "LICENSE", "LICENCE", "LICENSING"))
            or p.relative_to(UI_DIR).parts[0] == "LICENSES"
        )
    )
    if not found:
        sys.exit(f"no notice/licence file staged next to the UI bundle in {UI_DIR}")
    parts = [f"=== {rel} ===\n{(UI_DIR / rel).read_text(encoding='utf-8')}" for rel in found]
    return "\n\n".join(parts)
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
    if not UI_JS.exists() or not UI_CSS.exists():
        sys.exit(
            f"missing staged shared UI at {UI_DIR} "
            "(nirs4all-ui-browser.iife.js, browser.css and a notice file); "
            "the release build stages it from the single UI build."
        )

    index = (HERE / "index.html").read_text()
    app_js = (HERE / "app.js").read_text()
    glue_js = GLUE.read_text()
    ui_js = UI_JS.read_text(encoding="utf-8")
    ui_css = UI_CSS.read_text(encoding="utf-8")
    notices = ui_notices()
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

    # Inline text must not close its own <script>/<style>: refuse, never rewrite.
    for label, text in (("wasm-bindgen glue", glue_js), ("app.js", app_js),
                        ("shared UI bundle", ui_js), ("UI notices", notices)):
        refuse_closers(label, text, "</script")
    refuse_closers("shared UI CSS", ui_css, "</style")

    # Common UI (identical bytes to the served build) goes before the demo scripts.
    # Notices ride in an inert text/plain block; the bundle keeps its own headers.
    ui_blob = (
        '<script type="text/plain" id="nirs4all-ui-notices">\n' + notices + "\n</script>\n"
        '<script>\n/* nirs4all-ui browser entry (see notices above) */\n' + ui_js + "\n</script>"
    )
    ui_style = "<style>\n/* nirs4all-ui browser entry */\n" + ui_css + "\n</style>"

    marker = '<script src="./app.js" defer></script>'
    html = inline_assets(index, HERE)
    html = replace_once(html, UI_CSS_MARKER, ui_style)
    html = replace_once(html, UI_JS_MARKER, ui_blob)
    html = replace_once(html, marker, blob)
    # The verbatim notice texts may mention vendor/ui/ in prose; only a reference outside them is a load.
    if "vendor/ui" in html.replace(notices, ""):
        sys.exit("standalone output still references vendor/ui")

    OUT.write_text(html)
    mb = OUT.stat().st_size / 1024 / 1024
    print(f"wrote {OUT.relative_to(HERE.parent)} ({mb:.1f} MB) — open it directly in a browser")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
