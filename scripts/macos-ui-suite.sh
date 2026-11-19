#!/usr/bin/env bash
# Every pull-request UI scenario, one after another, on one macos-15 runner.
# The workflow compiles lux once and exports LUX_PREBUILT=1 before this.
# Performance scenarios (H1-H6, key frame, release timings) are macos-perf.yml.
set -uo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
chmod +x scripts/*.sh scripts/*.py 2>/dev/null || chmod +x scripts/*.sh

if [[ ! -x "$root/target/debug/lux" ]]; then
  echo "missing $root/target/debug/lux; the workflow builds it once" >&2
  exit 1
fi

scenarios=(
  app-icon-shot.sh
  buffer-menu.sh
  close-folder.sh
  close-root.sh
  compare-captures.sh
  conflict-shot.sh
  evidence-macos.sh
  file-drop.sh
  file-menu.sh
  git-branch-e2e.sh
  git-status-e2e.sh
  gutter-clip.sh
  html-preview.sh
  render-icons.sh
  macos-keystrokes.sh
  markdown-preview.sh
  markdown-shot.sh
  monokai-text.sh
  p0-shortcuts.sh
  p0-editing.sh
  p0-hidpi.sh
  p0-checklist.sh
  p0-same-window.sh
  p0-lsp.sh
  selection-contrast.sh
  selection-drag.sh
  settings-e2e.sh
  sidebar-clip.sh
  tab-view-audit.sh
  theme-break.sh
  tree-drag.sh
  tree-watch.sh
  triple-click.sh
  unstage-shot.sh
  window-restore.sh
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
  echo "macos UI suite failed"
  exit 1
fi
echo "macos UI suite passed"
