#!/usr/bin/env bash
# Wrap the lux binary in target/Lux.app. The evidence, compare, and P0
# jobs launch this bundle. Pass --release for the performance job.
# Does not change Info.plist.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
profile="debug"
build=1
while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) profile="release" ;;
    --no-build) build=0 ;;
    *)
      echo "usage: stage-lux-app.sh [--release] [--no-build]" >&2
      exit 2
      ;;
  esac
  shift
done
bin="$root/target/$profile/lux"
app="$root/target/Lux.app"

if [[ "$build" == 1 || ! -x "$bin" ]]; then
  if [[ "$profile" == "release" ]]; then
    cargo build -p lux --release --locked
  else
    cargo build -p lux --locked
  fi
fi
if [[ ! -x "$bin" ]]; then
  echo "missing $bin" >&2
  exit 1
fi

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/SharedSupport/bin"
cp "$bin" "$app/Contents/MacOS/lux"
cp "$bin" "$app/Contents/SharedSupport/bin/lux"
cp "$root/packaging/macos/Info.plist" "$app/Contents/Info.plist"
chmod +x "$app/Contents/MacOS/lux" "$app/Contents/SharedSupport/bin/lux"
echo "$app"
