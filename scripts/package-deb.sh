#!/usr/bin/env bash
# Build an unsigned .deb that installs lux on PATH.
# Depends on libfontconfig and the EGL loader the release binary dlopens
# (wgpu's GL backend; this binary does not contain libvulkan.so.1). Also
# depends on the xkbcommon, X11, and Wayland libraries it dlopens before a
# window exists. libfreetype6 is pulled in by libfontconfig1.
set -euo pipefail

bin="${1:?path to the lux binary}"
version="${2:?version x.y.z}"
out="${3:-dist/lux-linux-amd64.deb}"
arch="${4:-amd64}"

version="${version#v}"
if [[ ! -f "$bin" ]]; then
  echo "package-deb: binary missing: $bin" >&2
  exit 1
fi
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "package-deb: version must be x.y.z, got $version" >&2
  exit 1
fi

root="$(git rev-parse --show-toplevel)"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

mkdir -p "$stage/DEBIAN" \
  "$stage/usr/bin" \
  "$stage/usr/share/applications" \
  "$stage/usr/share/doc/lux" \
  "$stage/usr/share/icons/hicolor/256x256/apps"

install -m 0755 "$bin" "$stage/usr/bin/lux"
if [[ -f "$root/packaging/linux/lux.desktop" ]]; then
  install -m 0644 "$root/packaging/linux/lux.desktop" "$stage/usr/share/applications/lux.desktop"
fi
icon="$root/packaging/macos/Lux.iconset/icon_256x256.png"
if [[ -f "$icon" ]]; then
  install -m 0644 "$icon" "$stage/usr/share/icons/hicolor/256x256/apps/lux.png"
fi
if [[ -f "$root/README.md" ]]; then
  install -m 0644 "$root/README.md" "$stage/usr/share/doc/lux/README.md"
fi
for license in "$root"/LICENSE*; do
  [[ -f "$license" ]] || continue
  install -m 0644 "$license" "$stage/usr/share/doc/lux/$(basename "$license")"
done

cat > "$stage/DEBIAN/control" << EOF
Package: lux
Version: ${version}
Section: editors
Priority: optional
Architecture: ${arch}
Maintainer: Lux Contributors <lcajigasm@users.noreply.github.com>
Depends: libfontconfig1, libegl1, libxkbcommon0, libxkbcommon-x11-0, libx11-6, libxi6, libxcursor1, libx11-xcb1, libwayland-client0, libwayland-egl1
Homepage: https://github.com/lcajigasm/lux
Description: Lux text and code editor
 Native editor with a Sublime Text keyboard model.
 The package is unsigned. It needs fontconfig and the EGL GPU loader,
 plus the xkbcommon, X11, and Wayland libraries used to open a window.
EOF

chmod 755 "$stage"
mkdir -p "$(dirname "$out")"
rm -f "$out"
dpkg-deb --root-owner-group -Zxz --build "$stage" "$out"

info="$(dpkg-deb -I "$out")"
printf '%s\n' "$info" | grep -q 'libfontconfig1' || {
  echo "package-deb: control is missing libfontconfig1" >&2
  exit 1
}
printf '%s\n' "$info" | grep -q 'libegl1' || {
  echo "package-deb: control is missing the EGL GPU loader dependency" >&2
  exit 1
}
echo "packed $out"
dpkg-deb -I "$out"
