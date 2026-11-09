#!/usr/bin/env bash
# Opening a Rust file must not kill the language-server thread.
# The process has to stay up and answer a probe dump. rust-analyzer
# itself may be absent; that is a failed session, not a panic.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
# shellcheck source=scripts/macos-drive.sh
source "$root/scripts/macos-drive.sh"

out="$root/docs/evidence"
mkdir -p "$out"
report="$out/lsp.txt"
: > "$report"

fail() {
  echo "FAIL $*" | tee -a "$report" >&2
  exit 1
}

app="$(bash "$root/scripts/stage-lux-app.sh")"
drive_init "$app"
trap drive_stop EXIT

file="$DRIVE_WORK/sample.rs"
printf 'fn main() {\n    let value = 1;\n    let _ = value;\n}\n' > "$file"
drive_launch "$file"
sleep 0.8

if ! kill -0 "$DRIVE_PID" 2>/dev/null; then
  fail "Lux.app exited after opening a Rust file"
fi
drive_dump
if ! kill -0 "$DRIVE_PID" 2>/dev/null; then
  fail "Lux.app exited while answering the probe"
fi

text="$(drive_field text)"
if [[ "$text" != *"fn main()"* ]]; then
  fail "probe text did not contain the Rust source: $(printf %q "$text")"
fi
if grep -q "there is no reactor running" "$DRIVE_LOG"; then
  fail "language server thread panicked: no Tokio runtime"
fi
if grep -q "panicked at" "$DRIVE_LOG"; then
  fail "Lux.app panicked: $(grep 'panicked at' "$DRIVE_LOG" | head -3)"
fi

echo "PASS rust file stayed alive and the probe answered" | tee -a "$report"
echo "--> lsp runtime done"
