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
mkdir -p "$app_path/Contents/SharedSupport/bin"
cp "$bin" "$app_path/Contents/SharedSupport/bin/lux"
chmod +x "$app_path/Contents/SharedSupport/bin/lux"
# Two release trees plus the disk-image scratch exhausted the runner
# (hdiutil: No space left on device, runs 41335548609 and 41341898224).
# Deleting only the arch directories still left the cargo registry and
# the rest of target/, and hdiutil failed while copying the binary.
# The bundle already holds the fat binary, so the whole target tree can go.
if [[ -z "${LUX_BIN:-}" ]]; then
  echo "--> Freeing build output before hdiutil"
  df -h /
  rm -rf target
  if [[ -n "${CI:-}" ]]; then
    rm -rf "$HOME/.cargo/registry/src" "$HOME/.cargo/registry/cache" \
      "$HOME/Library/Developer/Xcode" \
      "$HOME/Library/Caches/org.swift.swiftpm"
  fi
  df -h /
fi

sed "s/<string>1.0.0<\\/string>/<string>${version}<\\/string>/g" \
  packaging/macos/Info.plist > "$app_path/Contents/Info.plist"
cp "$icns_file" "$app_path/Contents/Resources/lux.icns"
echo -n "APPLlux " > "$app_path/Contents/PkgInfo"

# Ad-hoc signature only. There is no Apple Developer ID certificate in CI,
# and this script does not read signing secrets. Gatekeeper will still warn.
echo "--> Applying ad-hoc codesign (no Developer ID)..."
codesign --force --deep --sign - "$app_path"

# 4. Create DMG with Applications drag-and-drop link.
# `hdiutil create -srcfolder` picks its own scratch size and has failed
# with "No space left on device" after the image was already mounted
# (runs 41335548609 and 41341898224). The writable image is an explicit
# size taken from the staged tree, then converted to UDZO.
echo "--> Creating DMG staging..."
staging_dir="$(mktemp -d)"
raw_path=""
mounted=""
cleanup_dmg() {
  if [[ -n "${mounted:-}" ]]; then
    hdiutil detach "$mounted" -force >/dev/null 2>&1 || true
  fi
  rm -rf "$staging_dir"
  if [[ -n "${raw_path:-}" ]]; then
    rm -f "$raw_path"
  fi
}
trap cleanup_dmg EXIT

detach_volume() {
  local name="$1"
  local tries=0
  while [[ -d "/Volumes/$name" && "$tries" -lt 5 ]]; do
    echo "--> Detaching leftover /Volumes/$name"
    hdiutil detach "/Volumes/$name" -force >/dev/null 2>&1 \
      || diskutil unmount force "/Volumes/$name" >/dev/null 2>&1 \
      || true
    tries=$((tries + 1))
    sleep 1
  done
}
detach_volume "Lux"

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
raw_path="$dist_dir/.lux-rw.dmg"
rm -f "$dmg_path" "$raw_path"

staging_kb="$(du -sk "$staging_dir" | awk '{print $1}')"
# Half again, plus 32 MiB for the HFS catalog. Never smaller than 64 MiB.
size_mb=$(( (staging_kb + staging_kb / 2 + 32768 + 1023) / 1024 ))
if (( size_mb < 64 )); then
  size_mb=64
fi
echo "--> Writable image ${size_mb}m for ${staging_kb} KiB of payload"
df -h /

# A sized image is read/write by default. `-format UDRW` is rejected
# unless `-srcfolder` or `-srcdevice` is also set (macOS 15:
# "hdiutil: create: -format requires -srcfolder or -srcdevice").
hdiutil create \
  -size "${size_mb}m" \
  -fs HFS+ \
  -volname "Lux" \
  -ov \
  "$raw_path"

echo "--> Copying payload into the writable image..."
mount_output="$(hdiutil attach -nobrowse -readwrite "$raw_path")"
mounted="$(echo "$mount_output" | grep -o '/Volumes/[^	]*' | head -n 1)"
if [[ -z "$mounted" || ! -d "$mounted" ]]; then
  echo "Error: Failed to mount the writable image." >&2
  echo "$mount_output" >&2
  exit 1
fi
cp -R "$staging_dir/." "$mounted/"
# The staged tree is now inside the image. Drop it before compress.
rm -rf "$staging_dir"
hdiutil detach "$mounted"
mounted=""

echo "--> Compressing to $dmg_path..."
hdiutil convert "$raw_path" -format UDZO -ov -o "${dmg_path%.dmg}"
rm -f "$raw_path"
raw_path=""

# 5. Verify DMG mountability and contents
echo "--> Verifying DMG mounting and integrity..."
mount_output="$(hdiutil attach -nobrowse -readonly "$dmg_path")"
mounted="$(echo "$mount_output" | grep -o '/Volumes/[^	]*' | head -n 1)"

if [[ -z "$mounted" || ! -d "$mounted" ]]; then
  echo "Error: Failed to mount disk image for verification." >&2
  exit 1
fi

echo "--> Mounted at: $mounted"
if [[ ! -d "$mounted/$app_name" ]]; then
  echo "Error: $app_name missing inside mounted DMG!" >&2
  exit 1
fi

if [[ ! -L "$mounted/Applications" ]]; then
  echo "Error: /Applications symlink missing inside mounted DMG!" >&2
  exit 1
fi

echo "--> Verifying mounted binary execution..."
"$mounted/$app_name/Contents/MacOS/lux" --help 2>&1 || true
cli="$mounted/$app_name/Contents/SharedSupport/bin/lux"
if [[ ! -x "$cli" ]]; then
  echo "Error: CLI executable missing inside mounted DMG!" >&2
  exit 1
fi
version_line="$("$cli" --version)"
case "$version_line" in
  lux\ *) ;;
  *)
    echo "Error: CLI --version printed: $version_line" >&2
    exit 1
    ;;
esac

hdiutil detach "$mounted"
mounted=""
echo "--> DMG detached successfully."

echo ""
echo "=================================================="
echo " Packaging Complete!"
echo " App Bundle: $app_path"
echo " DMG Image:  $dmg_path ($(ls -lh "$dmg_path" | awk '{print $5}'))"
echo "=================================================="
