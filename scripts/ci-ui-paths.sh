#!/usr/bin/env bash
# Print true when the macOS UI suite should run. See ci_ui_paths.py.
set -euo pipefail
root="$(cd "$(dirname "$0")" && pwd)"
exec python3 "$root/ci_ui_paths.py" "$@"
