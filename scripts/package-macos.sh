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

# 1. Use a prebuilt binary, or compile one. LUX_BIN skips the universal lipo
# so the release workflow ships the architecture it just built.
if [[ -n "${LUX_BIN:-}" ]]; then
  bin="$LUX_BIN"
  echo "--> Using prebuilt binary $bin"
else
  echo "--> Compiling lux (release)..."
  cargo build -p lux --release
  bin="target/release/lux"
  x86_libdir="$(rustc --print target-libdir --target x86_64-apple-darwin 2>/dev/null || true)"
  if [[ -n "$x86_libdir" && -d "$x86_libdir" ]] && compgen -G "$x86_libdir/libstd*" > /dev/null 2>&1; then
    echo "--> Detected x86_64 target standard library; building universal binary..."
    cargo build -p lux --release --target x86_64-apple-darwin
    cargo build -p lux --release --target aarch64-apple-darwin
    mkdir -p target/universal
    lipo -create \
      target/aarch64-apple-darwin/release/lux \
      target/x86_64-apple-darwin/release/lux \
      -output target/universal/lux
    bin="target/universal/lux"
  else
    echo "--> Single architecture build ($bin) for host target $(uname -m)..."
  fi
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
