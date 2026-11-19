#!/usr/bin/env bash
# H1-H6 plus the other release timings. Nightly, manual, or a pull request
# that touches render or startup. The workflow compiles the release binary
# once and exports LUX_PREBUILT=1. A scenario may still compile its baseline.
set -uo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
chmod +x scripts/*.sh

if [[ ! -x "$root/target/release/lux" ]]; then
  echo "missing $root/target/release/lux; the workflow builds it once" >&2
  exit 1
fi

scenarios=(
  h1-cold-start.sh
  h1-overlap.sh
  h2-key-frame.sh
  h3-open.sh
  h4-open.sh
  h5-scroll-frame.sh
  h6-rss.sh
  key-frame.sh
  p0-perf.sh
)

failed=0
for script in "${scenarios[@]}"; do
  echo "=== $script ==="
  if bash "$root/scripts/$script"; then
    echo "PASS $script"
  else
    echo "FAIL $script"
    failed=1
  fi
done

if [[ "$failed" != 0 ]]; then
  echo "macos performance suite failed"
  exit 1
fi
echo "macos performance suite passed"
