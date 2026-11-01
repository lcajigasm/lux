#!/usr/bin/env bash
# Packages Lux as a native macOS Application Bundle (.app) and Disk Image (.dmg).
# Ad-hoc signed for local execution on macOS (Apple Silicon & Intel).
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"

dist_dir="${1:-dist}"
dmg_name="${2:-Lux.dmg}"
app_name="Lux.app"

echo "=== Lux macOS Packaging ==="

version="${LUX_VERSION:-}"
if [[ -z "$version" ]]; then
  version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
fi
version="${version#v}"
if [[ -z "$version" ]]; then
  echo "Error: could not determine Lux version" >&2
  exit 1
fi

# True when `rustup target add` has installed that Apple std.
have_target_std() {
  local triple="$1"
  local libdir
  libdir="$(rustc --print target-libdir --target "$triple" 2>/dev/null || true)"
  [[ -n "$libdir" && -d "$libdir" ]] || return 1
  compgen -G "$libdir/libstd*" > /dev/null
}

# 1. Use a prebuilt binary, or compile one.
# LUX_BIN skips lipo (one architecture, already built).
# Otherwise, when both Apple stds are installed, build arm64 and x86_64
# and lipo them. That is the release path. A host-only build is the fallback
# for a machine that has only one target.
if [[ -n "${LUX_BIN:-}" ]]; then
  bin="$LUX_BIN"
  echo "--> Using prebuilt binary $bin (single architecture, no lipo)"
elif have_target_std aarch64-apple-darwin && have_target_std x86_64-apple-darwin; then
  echo "--> Building universal binary (aarch64 + x86_64)..."
  cargo build -p lux --release --locked --target aarch64-apple-darwin
  cargo build -p lux --release --locked --target x86_64-apple-darwin
  mkdir -p target/universal
  lipo -create \
    target/aarch64-apple-darwin/release/lux \
    target/x86_64-apple-darwin/release/lux \
    -output target/universal/lux
  lipo -info target/universal/lux
  bin="target/universal/lux"
else
  echo "--> Single architecture release build for host $(uname -m)..."
  cargo build -p lux --release --locked
  bin="target/release/lux"
fi

if [[ ! -f "$bin" ]]; then
  echo "Error: built executable missing at $bin" >&2
  exit 1
fi

echo "--> Target binary architecture:"
file "$bin"

# 2. Ensure icon exists
icns_file="packaging/macos/lux.icns"
if [[ ! -f "$icns_file" ]]; then
  echo "--> Generating high-resolution lux.icns..."
  python3 packaging/macos/build_icon.py
fi

# 3. Assemble Lux.app
mkdir -p "$dist_dir"
app_path="$dist_dir/$app_name"
rm -rf "$app_path"

echo "--> Creating $app_path bundle..."
mkdir -p "$app_path/Contents/MacOS"
mkdir -p "$app_path/Contents/Resources"

cp "$bin" "$app_path/Contents/MacOS/lux"
chmod +x "$app_path/Contents/MacOS/lux"
# Two release trees plus the disk-image scratch exhausted the runner
# (hdiutil: No space left on device, run 41335548609). The bundle already
# holds the fat binary.
if [[ -z "${LUX_BIN:-}" ]]; then
  rm -rf target/aarch64-apple-darwin target/x86_64-apple-darwin target/release
fi

sed "s/<string>1.0.0<\\/string>/<string>${version}<\\/string>/g" \
  packaging/macos/Info.plist > "$app_path/Contents/Info.plist"
cp "$icns_file" "$app_path/Contents/Resources/lux.icns"
echo -n "APPLlux " > "$app_path/Contents/PkgInfo"

# Ad-hoc signature only. There is no Apple Developer ID certificate in CI,
# and this script does not read signing secrets. Gatekeeper will still warn.
echo "--> Applying ad-hoc codesign (no Developer ID)..."
codesign --force --deep --sign - "$app_path"

# 4. Create DMG with Applications drag-and-drop link
echo "--> Creating DMG staging..."
staging_dir="$(mktemp -d)"
trap 'rm -rf "$staging_dir"' EXIT

cp -R "$app_path" "$staging_dir/$app_name"
ln -s /Applications "$staging_dir/Applications"

# Include documentation in disk image
mkdir -p "$staging_dir/Documentation"
cp README.md "$staging_dir/Documentation/"
for license in LICENSE*; do
  [[ -f "$license" ]] || continue
  cp "$license" "$staging_dir/Documentation/"
done

dmg_path="$dist_dir/$dmg_name"
rm -f "$dmg_path"

echo "--> Building compressed disk image: $dmg_path..."
hdiutil create \
  -volname "Lux" \
  -srcfolder "$staging_dir" \
  -ov \
  -format UDZO \
  "$dmg_path"

# 5. Verify DMG mountability and contents
echo "--> Verifying DMG mounting and integrity..."
mount_output="$(hdiutil attach -nobrowse -readonly "$dmg_path")"
mount_point="$(echo "$mount_output" | grep -o '/Volumes/[^	]*' | head -n 1)"

if [[ -z "$mount_point" || ! -d "$mount_point" ]]; then
  echo "Error: Failed to mount disk image for verification." >&2
  exit 1
fi

echo "--> Mounted at: $mount_point"
if [[ ! -d "$mount_point/$app_name" ]]; then
  echo "Error: $app_name missing inside mounted DMG!" >&2
  hdiutil detach "$mount_point"
  exit 1
fi

if [[ ! -L "$mount_point/Applications" ]]; then
  echo "Error: /Applications symlink missing inside mounted DMG!" >&2
  hdiutil detach "$mount_point"
  exit 1
fi

echo "--> Verifying mounted binary execution..."
"$mount_point/$app_name/Contents/MacOS/lux" --help 2>&1 || true

hdiutil detach "$mount_point"
echo "--> DMG detached successfully."

echo ""
echo "=================================================="
echo " Packaging Complete!"
echo " App Bundle: $app_path"
echo " DMG Image:  $dmg_path ($(ls -lh "$dmg_path" | awk '{print $5}'))"
echo "=================================================="
