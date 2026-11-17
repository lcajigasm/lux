#!/usr/bin/env bash
# macos-15: Settings as JSON must not leave its view on the Plain Text buffer.
# The same process then opens, closes, and switches JSON, Markdown, HTML,
# Plain Text, and Settings. After each step the probe has to describe only
# the buffer on screen. Screenshots are the compiled app.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "tab-view-audit runs on macOS. This host is $(uname -s)." >&2
  exit 1
fi

real_home="${HOME}"
out="$root/docs/evidence/tab-view-audit"
rm -rf "$out"
mkdir -p "$out"
report="$out/report.txt"
: > "$report"
problems=0

pass() { echo "PASS $1" | tee -a "$report"; }
problem() {
  echo "FAIL $1" | tee -a "$report" >&2
  problems=$((problems + 1))
}
die() {
  echo "FAIL $1" | tee -a "$report" >&2
  if [[ -n "${DRIVE_PROBE:-}" && -s "$DRIVE_PROBE" ]]; then
    echo "probe:" | tee -a "$report" >&2
    cat "$DRIVE_PROBE" | tee -a "$report" >&2
  fi
  tail -40 "${DRIVE_LOG:-/dev/null}" >&2 || true
  exit 1
}

# Fixtures stay outside the checkout. A file inside the repo gets gutter
# marks from git, and those are not the diagnostic leak under test.
home="$(mktemp -d)"
files="$(mktemp -d)"
printf 'asdf\n' > "$files/note.txt"
printf '{\n  "a": 1\n}\n' > "$files/data.json"
printf '# Title\n\nHello.\n' > "$files/note.md"
printf '<h1>Title</h1>\n<p>Hello</p>\n' > "$files/page.html"

echo "--> Building lux"
HOME="$real_home" cargo build -p lux --locked
app="$(HOME="$real_home" bash "$root/scripts/stage-lux-app.sh" --no-build)"
echo "--> bundled app $app" | tee -a "$report"

export HOME="$home"
drive_init "$app"
cleanup() {
  drive_stop || true
  rm -rf "$home" "$files"
}
trap cleanup EXIT

capture() {
  local dest="$1"
  kill -USR1 "$DRIVE_PID"
  local _
  for _ in $(seq 1 40); do
    if [[ -s "$dest" ]]; then
      return 0
    fi
    sleep 0.25
  done
  die "snapshot missing: $dest"
}

launch_bare() {
  drive_stop
  rm -f "$DRIVE_PROBE"
  : > "$DRIVE_LOG"
  "$DRIVE_APP/Contents/MacOS/lux" --probe "$DRIVE_PROBE" "$@" >"$DRIVE_LOG" 2>&1 &
  DRIVE_PID=$!
  drive_wait_window
  sleep 0.3
  drive_focus
}

record() {
  local label="$1"
  echo "$label language=$(drive_field language) highlight=$(drive_field highlight) syntax_tokens=$(drive_field syntax_tokens) diagnostics=$(drive_field diagnostics) diagnostic_lines=$(drive_field diagnostic_lines) gutter_marks=$(drive_field gutter_marks) preview=$(drive_field preview) path=$(drive_field active_path)" | tee -a "$report"
}

run_palette() {
  local query="$1"
  local opened=0
  local _
  for _ in $(seq 1 4); do
    drive_key tap 35 cmd,shift
    sleep 0.4
    drive_dump || die "probe died opening the palette"
    if [[ "$(drive_field palette)" == "1" ]]; then
      opened=1
      break
    fi
  done
  [[ "$opened" == 1 ]] || die "palette did not open for $query"
  drive_key chars "$query"
  local typed=0
  for _ in $(seq 1 40); do
    sleep 0.25
    drive_dump || die "probe died while typing $query"
    if [[ "$(drive_field palette_query)" == "$query" ]]; then
      typed=1
      break
    fi
  done
  [[ "$typed" == 1 ]] || die "palette query is '$(drive_field palette_query)', want $query"
  drive_key tap 36 ""
  sleep 0.35
}

# Highlighting is filled in on a later frame. Wait until it matches this buffer.
await_syntax() {
  local highlight="$1"
  local tokens="$2"
  local _
  for _ in $(seq 1 60); do
    drive_dump || die "probe died while waiting for $highlight"
    local got_highlight got_tokens
    got_highlight="$(drive_field highlight)"
    got_tokens="$(drive_field syntax_tokens)"
    if [[ "$got_highlight" == "$highlight" ]]; then
      if [[ "$tokens" == "zero" && "$got_tokens" == "0" ]]; then
        return 0
      fi
      if [[ "$tokens" == "some" && "$got_tokens" != "0" ]]; then
        return 0
      fi
    fi
    sleep 0.25
  done
  return 1
}

expect_view() {
  local label="$1"
  local language="$2"
  local highlight="$3"
  local tokens="$4"
  local diagnostics="$5"
  local preview="$6"
  local needle="$7"
  await_syntax "$highlight" "$tokens" || true
  drive_dump || die "probe died at $label"
  record "$label"
  local bad=0
  [[ "$(drive_field language)" == "$language" ]] || bad=1
  [[ "$(drive_field highlight)" == "$highlight" ]] || bad=1
  if [[ "$tokens" == "zero" ]]; then
    [[ "$(drive_field syntax_tokens)" == "0" ]] || bad=1
  else
    [[ "$(drive_field syntax_tokens)" != "0" ]] || bad=1
  fi
  if [[ "$diagnostics" == "zero" ]]; then
    [[ "$(drive_field diagnostics)" == "0" && -z "$(drive_field diagnostic_lines)" ]] || bad=1
  else
    [[ "$(drive_field diagnostics)" != "0" ]] || bad=1
  fi
  [[ "$(drive_field gutter_marks)" == "0" ]] || bad=1
  [[ "$(drive_field preview)" == "$preview" ]] || bad=1
  if [[ "$needle" == "-" ]]; then
    [[ -z "$(drive_field active_path)" ]] || bad=1
  else
    [[ "$(drive_field active_path)" == *"$needle"* ]] || bad=1
  fi
  if [[ "$bad" == 1 ]]; then
    problem "$label does not match $language (highlight=$highlight tokens=$tokens diagnostics=$diagnostics preview=$preview)"
  else
    pass "$label"
  fi
}

focus_path() {
  local needle="$1"
  local key
  for key in 18 19 20 21 23; do
    drive_key tap "$key" cmd
    sleep 0.3
    drive_dump || die "probe died switching toward $needle"
    if [[ "$(drive_field active_path)" == *"$needle"* ]]; then
      return 0
    fi
  done
  die "no tab matches $needle (active=$(drive_field active_path) tabs=$(drive_field tabs))"
}

close_active() {
  local before
  drive_dump || die "probe died before close"
  before="$(drive_field tabs)"
  drive_key tap 13 cmd
  local _
  for _ in $(seq 1 40); do
    sleep 0.25
    drive_dump || die "probe died after close"
    if [[ "$(drive_field tabs)" -lt "$before" ]]; then
      return 0
    fi
  done
  die "close left $(drive_field tabs) tabs at $(drive_field active_path)"
}

echo "--> Settings JSON, then the untitled Plain Text buffer"
launch_bare \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/01-settings-open.png" \
  --snapshot-on-usr1 "$out/02-plain-after-close.png"

ready=0
for _ in $(seq 1 40); do
  if drive_dump && [[ "$(drive_field language)" == "Plain Text" && "$(drive_field tabs)" == "1" ]]; then
    ready=1
    break
  fi
  sleep 0.25
done
[[ "$ready" == 1 ]] || die "untitled buffer did not open"

drive_key chars "asdf"
ready=0
for _ in $(seq 1 40); do
  sleep 0.25
  drive_dump || die "probe died after typing"
  if [[ "$(drive_field text)" == *asdf* ]]; then
    ready=1
    break
  fi
done
[[ "$ready" == 1 ]] || die "asdf was not inserted (text=$(drive_field text))"

run_palette "settings (json)"
ready=0
for _ in $(seq 1 60); do
  sleep 0.25
  drive_dump || die "probe died opening settings"
  if [[ "$(drive_field language)" == "JSON" && "$(drive_field diagnostics)" != "0" && "$(drive_field tabs)" == "2" ]]; then
    ready=1
    break
  fi
done
[[ "$ready" == 1 ]] || die "settings JSON did not open beside asdf (language=$(drive_field language) diagnostics=$(drive_field diagnostics) tabs=$(drive_field tabs))"
await_syntax "JSON" "some" || true
record "settings-open"
capture "$out/01-settings-open.png"

drive_key tap 13 cmd
ready=0
for _ in $(seq 1 40); do
  sleep 0.25
  drive_dump || die "probe died after closing settings"
  if [[ "$(drive_field language)" == "Plain Text" && "$(drive_field text)" == *asdf* && "$(drive_field tabs)" == "1" ]]; then
    ready=1
    break
  fi
done
[[ "$ready" == 1 ]] || die "closing settings did not return to the Plain Text buffer (language=$(drive_field language) tabs=$(drive_field tabs) text=$(drive_field text))"
capture "$out/02-plain-after-close.png"
expect_view "plain-after-close" "Plain Text" "none" "zero" "zero" "0" "-"
if python3 "$root/scripts/tab_view_pixels.py" "$out/02-plain-after-close.png" | tee -a "$report"; then
  pass "plain text capture has no red diagnostic paint"
else
  problem "plain text capture still has red diagnostic paint"
fi

echo "--> Switch JSON, Markdown, HTML, Plain Text, and Settings"
drive_launch "$files/page.html" \
  --snapshot-scale 2 \
  --snapshot-on-usr1 "$out/03-json.png" \
  --snapshot-on-usr1 "$out/04-markdown-preview.png" \
  --snapshot-on-usr1 "$out/05-html-preview.png" \
  --snapshot-on-usr1 "$out/06-plain-beside-settings.png" \
  --snapshot-on-usr1 "$out/07-plain-after-audit.png" \
  "$files/note.txt" "$files/data.json" "$files/note.md"

ready=0
for _ in $(seq 1 80); do
  sleep 0.25
  drive_dump || die "probe died while opening the audit files"
  if [[ "$(drive_field tabs)" == "4" && "$(drive_field active_path)" == "$files/page.html" ]]; then
    ready=1
    break
  fi
done
[[ "$ready" == 1 ]] || die "the four files did not open (tabs=$(drive_field tabs) path=$(drive_field active_path))"

expect_view "open-html" "HTML" "HTML" "some" "zero" "0" "page.html"

focus_path "note.txt"
expect_view "switch-plain" "Plain Text" "none" "zero" "zero" "0" "note.txt"
[[ "$(drive_field text)" == *asdf* ]] || problem "plain text buffer lost asdf"

focus_path "data.json"
expect_view "switch-json" "JSON" "JSON" "some" "zero" "0" "data.json"
capture "$out/03-json.png"

focus_path "note.md"
expect_view "switch-markdown" "Markdown" "Markdown" "some" "zero" "0" "note.md"
run_palette "toggle preview"
ready=0
for _ in $(seq 1 40); do
  sleep 0.25
  drive_dump || die "probe died after the markdown preview"
  if [[ "$(drive_field preview)" == "1" && "$(drive_field preview_blocks)" == *heading:Title* ]]; then
    ready=1
    break
  fi
done
[[ "$ready" == 1 ]] || problem "markdown preview did not render Title (preview=$(drive_field preview) blocks=$(drive_field preview_blocks))"
expect_view "markdown-preview" "Markdown" "Markdown" "some" "zero" "1" "note.md"
capture "$out/04-markdown-preview.png"

focus_path "page.html"
expect_view "html-after-markdown-preview" "HTML" "HTML" "some" "zero" "0" "page.html"
run_palette "toggle preview"
ready=0
for _ in $(seq 1 40); do
  sleep 0.25
  drive_dump || die "probe died after the html preview"
  if [[ "$(drive_field preview)" == "1" && "$(drive_field preview_blocks)" == *heading:Title* ]]; then
    ready=1
    break
  fi
done
[[ "$ready" == 1 ]] || problem "html preview did not render Title (preview=$(drive_field preview) blocks=$(drive_field preview_blocks))"
expect_view "html-preview" "HTML" "HTML" "some" "zero" "1" "page.html"
capture "$out/05-html-preview.png"

focus_path "note.md"
expect_view "markdown-preview-kept" "Markdown" "Markdown" "some" "zero" "1" "note.md"

focus_path "note.txt"
expect_view "plain-between-previews" "Plain Text" "none" "zero" "zero" "0" "note.txt"

run_palette "settings (json)"
ready=0
for _ in $(seq 1 60); do
  sleep 0.25
  drive_dump || die "probe died opening settings during the audit"
  if [[ "$(drive_field active_path)" == *settings.json && "$(drive_field diagnostics)" != "0" ]]; then
    ready=1
    break
  fi
done
[[ "$ready" == 1 ]] || die "settings did not become active during the audit"
expect_view "settings-active" "JSON" "JSON" "some" "some" "0" "settings.json"

focus_path "note.txt"
expect_view "plain-beside-settings" "Plain Text" "none" "zero" "zero" "0" "note.txt"
capture "$out/06-plain-beside-settings.png"

focus_path "settings.json"
expect_view "settings-again" "JSON" "JSON" "some" "some" "0" "settings.json"
close_active
focus_path "note.txt"
expect_view "plain-after-audit" "Plain Text" "none" "zero" "zero" "0" "note.txt"
capture "$out/07-plain-after-audit.png"
if python3 "$root/scripts/tab_view_pixels.py" "$out/07-plain-after-audit.png" | tee -a "$report"; then
  pass "plain text capture after the audit has no red diagnostic paint"
else
  problem "plain text capture after the audit still has red diagnostic paint"
fi

focus_path "data.json"
close_active
focus_path "note.md"
expect_view "markdown-after-json-close" "Markdown" "Markdown" "some" "zero" "1" "note.md"
focus_path "page.html"
expect_view "html-after-json-close" "HTML" "HTML" "some" "zero" "1" "page.html"
focus_path "note.txt"
expect_view "plain-after-json-close" "Plain Text" "none" "zero" "zero" "0" "note.txt"

focus_path "note.md"
close_active
focus_path "page.html"
expect_view "html-after-markdown-close" "HTML" "HTML" "some" "zero" "1" "page.html"
focus_path "note.txt"
expect_view "plain-after-markdown-close" "Plain Text" "none" "zero" "zero" "0" "note.txt"

if [[ "$problems" != 0 ]]; then
  echo "FAIL $problems view checks" | tee -a "$report" >&2
  exit 1
fi
echo "PASS tab view audit" | tee -a "$report"
