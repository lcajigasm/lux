#!/usr/bin/env bash
# Layout a desktop dist tree. Signing/notarization needs Apple/Microsoft
# credentials and is not performed here.
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
cd "$root"
dest="${1:-dist/lux}"
# Never replace an existing destination: it may contain user data.
if [[ -e "$dest" || -L "$dest" ]]; then
  echo "package: destination already exists: $dest" >&2
  exit 1
fi
cargo build -p lux --release
bin="target/release/lux"
if [[ ! -f "$bin" ]]; then
  echo "package: built executable is missing: $bin" >&2
  exit 1
fi
mkdir -p "$(dirname "$dest")"
# mkdir without -p also rejects a destination created concurrently.
mkdir "$dest"
mkdir "$dest/bin" "$dest/share" "$dest/share/doc"
cp "$bin" "$dest/bin/lux"
cp README.md "$dest/share/doc/"
for license in LICENSE*; do
  [[ -f "$license" ]] || continue
  cp "$license" "$dest/share/doc/"
done
echo "packed $dest (unsigned)"

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
parent="$(dirname "$dest")"
if [[ "$(uname -s)" == "Darwin" ]]; then
  # Do not pass LUX_BIN. That flag skips lipo. The dmg is universal when
  # both Apple std targets are installed (`rustup target add`).
  LUX_VERSION="$version" \
    ./scripts/package-macos.sh "$parent" "lux-macos-universal.dmg"
elif [[ "$(uname -s)" == "Linux" ]]; then
  ./scripts/package-deb.sh "$root/$bin" "$version" "$parent/lux-linux-amd64.deb"
fi
