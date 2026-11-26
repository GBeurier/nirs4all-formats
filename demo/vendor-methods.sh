#!/usr/bin/env bash
# Vendor @nirs4all/methods into demo/vendor/methods/ for the "Convert…" panel.
#
#   ./demo/vendor-methods.sh             # the exact version pinned in vendor/methods.version
#   ./demo/vendor-methods.sh DIST_DIR    # a local build's bindings/js/dist (testing only;
#                                        # never commit a vendored local build)
#
# The pin file is the single place that names the version. Only the browser
# files are copied: the JavaScript modules, n4m.wasm and the licences.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
pin="$(tr -d '[:space:]' < "$here/vendor/methods.version")"
target="$here/vendor/methods"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

if [[ $# -ge 1 ]]; then
  dist="$(cd "$1" && pwd)"
  package="$(dirname "$dist")"
else
  (cd "$work" && npm pack "@nirs4all/methods@$pin" >/dev/null)
  tar -xzf "$work"/nirs4all-methods-*.tgz -C "$work"
  package="$work/package"
  dist="$package/dist"
  version="$(node -p "require('$package/package.json').version")"
  if [[ "$version" != "$pin" ]]; then
    echo "error: npm returned @nirs4all/methods $version, pinned $pin" >&2
    exit 1
  fi
fi

rm -rf "$target"
mkdir -p "$target"
cp "$dist"/*.js "$dist/n4m.wasm" "$dist/THIRD_PARTY_LICENSES.md" "$target/"
cp -r "$dist/LICENSES" "$target/"
cp "$package/LICENSE" "$package/LICENSE-CeCILL-2.1" "$package/package.json" "$target/"
echo "vendored @nirs4all/methods from $dist into demo/vendor/methods (pin $pin)"
