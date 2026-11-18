#!/usr/bin/env bash
# Read a list of changed paths on stdin. Print "true" when the macOS UI
# jobs should run, and "false" when every path is documentation.
# Empty input prints "true": a missing diff must not skip the UI jobs.
set -euo pipefail

classify() {
  local ui=false saw=false path
  while IFS= read -r path || [[ -n "${path:-}" ]]; do
    [[ -z "$path" ]] && continue
    saw=true
    case "$path" in
      docs/* | *.md | LICENSE | LICENSE-*) ;;
      *) ui=true ;;
    esac
  done
  if [[ "$saw" == false ]]; then
    ui=true
  fi
  printf '%s\n' "$ui"
}

if [[ "${1:-}" == "--self-test" ]]; then
  check() {
    local want="$1"
    shift
    local got
    got="$(printf '%s\n' "$@" | classify)"
    if [[ "$got" != "$want" ]]; then
      echo "ci-ui-paths: expected $want for: $*" >&2
      exit 1
    fi
  }
  check false "docs/STATUS.md" "README.md" "CHANGELOG.md" "LICENSE"
  check false "docs/adr/0001-lessons-from-v0.md"
  check false "crates/lux/README.md"
  check true "docs/STATUS.md" "crates/lux/src/lib.rs"
  check true ".github/workflows/ci.yml"
  check true "scripts/stage-lux-app.sh"
  check true "Cargo.toml"
  check true "LICENSE" "Cargo.lock"
  got="$(printf '' | classify)"
  if [[ "$got" != true ]]; then
    echo "ci-ui-paths: empty input must run UI jobs" >&2
    exit 1
  fi
  echo "ci-ui-paths: self-test ok"
  exit 0
fi

classify
