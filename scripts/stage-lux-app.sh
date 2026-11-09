#!/usr/bin/env bash
# Wrap the debug lux binary in target/Lux.app. The evidence and compare
# jobs launch this bundle, not a loose binary. Does not change Info.plist.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
bin="$root/target/debug/lux"
app="$root/target/Lux.app"

if [[ "${1:-}" != "--no-build" || ! -x "$bin" ]]; then
  cargo build -p lux --locked
fi
if [[ ! -x "$bin" ]]; then
  echo "missing $bin" >&2
  exit 1
fi

rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp "$bin" "$app/Contents/MacOS/lux"
cp "$root/packaging/macos/Info.plist" "$app/Contents/Info.plist"
chmod +x "$app/Contents/MacOS/lux"
echo "$app"
