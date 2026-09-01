#!/usr/bin/env bash
# Layout a desktop dist tree. Signing/notarization needs Apple/Microsoft
# credentials and is not performed here.
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
cd "$root"
dest="${1:-dist/lux}"
rm -rf "$dest"
mkdir -p "$dest/bin" "$dest/share/doc"
cargo build -p lux --release
bin="target/release/lux"
if [[ -f "$bin" ]]; then
  cp "$bin" "$dest/bin/lux"
fi
cp README.md "$dest/share/doc/" 2>/dev/null || true
cp LICENSE* "$dest/share/doc/" 2>/dev/null || true
echo "packed $dest (unsigned)"
