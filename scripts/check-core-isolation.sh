#!/usr/bin/env bash
# Enforces golden rule #2: lux-core must compile without any UI/graphics
# dependency. Checked by grepping the resolved dependency tree, not just
# Cargo.toml, so a transitive pull-in is caught too.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

forbidden='wgpu|winit|parley|swash|accesskit'

if cargo tree -p lux-core --edges normal | grep -Ei "$forbidden"; then
  echo "check-core-isolation: lux-core must not depend on UI/graphics crates (forbidden: $forbidden)" >&2
  exit 1
fi

echo "check-core-isolation: lux-core has no UI/graphics dependencies."
