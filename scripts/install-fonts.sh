#!/usr/bin/env bash
# Copy the bundled fonts, their OFL licenses, and NOTICE.md into a package.
# Usage: install-fonts.sh <destination fonts directory>
# Every packager calls this (see docs/adr/0020-bundled-developer-fonts.md).
# Fails when a shipped file does not match fonts/SHA256SUMS.
set -euo pipefail

dest="${1:?usage: install-fonts.sh <destination fonts directory>}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
src="$root/fonts"

if [[ ! -f "$src/SHA256SUMS" || ! -f "$src/NOTICE.md" ]]; then
  echo "install-fonts: $src is missing SHA256SUMS or NOTICE.md" >&2
  exit 1
fi
if command -v shasum >/dev/null 2>&1; then
  sha256=(shasum -a 256)
elif command -v sha256sum >/dev/null 2>&1; then
  sha256=(sha256sum)
else
  echo "install-fonts: neither shasum nor sha256sum is installed" >&2
  exit 1
fi
(cd "$src" && "${sha256[@]}" -c SHA256SUMS >/dev/null) || {
  echo "install-fonts: a font in $src does not match SHA256SUMS" >&2
  exit 1
}

mkdir -p "$dest"
install -m 0644 "$src/NOTICE.md" "$src/SHA256SUMS" "$dest/"
while read -r _sum file; do
  family="$(dirname "$file")"
  if [[ ! -f "$src/$family/OFL.txt" ]]; then
    echo "install-fonts: $family has no OFL.txt" >&2
    exit 1
  fi
  mkdir -p "$dest/$family"
  install -m 0644 "$src/$file" "$dest/$file"
  for extra in "$src/$family"/*.txt; do
    install -m 0644 "$extra" "$dest/$family/"
  done
done < "$src/SHA256SUMS"
echo "install-fonts: $dest"
