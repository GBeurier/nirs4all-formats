#!/usr/bin/env bash
# Encrypt / decrypt the local-only sample corpus (`samples_local/`).
#
# `samples_local/` holds non-redistributable vendor fixtures and is gitignored.
# To keep a recoverable copy alongside the code without publishing the raw files,
# the whole tree is packed into a single AES-256 encrypted archive that *is*
# committed (`samples_local.tar.gz.enc`). The passphrase lives in `samples_local.key`
# at the repo root, which is gitignored — it never leaves your machine.
#
#   ./scripts/samples_local_crypt.sh encrypt   # samples_local/ -> samples_local.tar.gz.enc
#   ./scripts/samples_local_crypt.sh decrypt   # samples_local.tar.gz.enc -> samples_local/
#   ./scripts/samples_local_crypt.sh verify    # round-trip check, no writes to samples_local/
#
# A fresh checkout with the key in hand restores the corpus with `decrypt`.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"

ARCHIVE="samples_local.tar.gz.enc"
KEY="samples_local.key"
CIPHER=(-aes-256-cbc -pbkdf2 -iter 200000 -salt)

ensure_key() {
  if [[ ! -f "$KEY" ]]; then
    echo "▸ no $KEY found — generating a fresh 256-bit passphrase"
    openssl rand -base64 32 > "$KEY"
    chmod 600 "$KEY"
    echo "  keep $KEY safe; it is gitignored and is the ONLY way to decrypt $ARCHIVE"
  fi
}

case "${1:-}" in
  encrypt)
    [[ -d samples_local ]] || { echo "error: samples_local/ not found" >&2; exit 1; }
    ensure_key
    echo "▸ packing samples_local/ ($(find samples_local -type f | wc -l | tr -d ' ') files) → $ARCHIVE"
    tar czf - samples_local | openssl enc "${CIPHER[@]}" -pass "file:$KEY" -out "$ARCHIVE"
    echo "✓ wrote $ARCHIVE ($(du -h "$ARCHIVE" | cut -f1))"
    ;;
  decrypt)
    [[ -f "$ARCHIVE" ]] || { echo "error: $ARCHIVE not found" >&2; exit 1; }
    [[ -f "$KEY" ]] || { echo "error: $KEY not found — cannot decrypt" >&2; exit 1; }
    echo "▸ decrypting $ARCHIVE → samples_local/"
    openssl enc -d "${CIPHER[@]}" -pass "file:$KEY" -in "$ARCHIVE" | tar xzf -
    echo "✓ restored samples_local/"
    ;;
  verify)
    [[ -f "$ARCHIVE" && -f "$KEY" ]] || { echo "error: need $ARCHIVE and $KEY" >&2; exit 1; }
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    openssl enc -d "${CIPHER[@]}" -pass "file:$KEY" -in "$ARCHIVE" | tar xzf - -C "$tmp"
    if diff -rq samples_local "$tmp/samples_local" >/dev/null; then
      echo "✓ round-trip OK: $ARCHIVE decrypts byte-identical to samples_local/"
    else
      echo "✗ MISMATCH between samples_local/ and the decrypted archive" >&2
      diff -rq samples_local "$tmp/samples_local" >&2 || true
      exit 1
    fi
    ;;
  *)
    echo "usage: $0 {encrypt|decrypt|verify}" >&2
    exit 2
    ;;
esac
