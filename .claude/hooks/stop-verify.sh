#!/usr/bin/env bash
# Wired as a Stop hook in .claude/settings.json.
# Blocks session close while the active crate's gate is still red — the
# technical enforcement of golden rule #1 ("nothing simulated"). CLAUDE.md
# alone is context, not enforcement; this is what actually stops a session
# from being declared done while a test is failing.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

crate="${LUX_ACTIVE_CRATE:-}"
[ -z "$crate" ] && exit 0 # no active crate declared, nothing to force here

log=$(mktemp -t lux-stop-verify)
if cargo test -p "$crate" --quiet >"$log" 2>&1; then
  rm -f "$log"
  exit 0
else
  echo "cargo test -p $crate is still red. Fix it before treating this deliverable as done. See $log" >&2
  exit 2
fi
