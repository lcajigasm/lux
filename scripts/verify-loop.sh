#!/usr/bin/env bash
# Headless verified loop: repeats a `claude -p` turn against a fixed verify
# command until it passes, gets stuck (same failure 3x), or hits --max.
# Flags verified against Claude Code CLI 2.1.206 — re-check `claude --help`
# if this starts failing after a CLI upgrade, flags do change between
# versions.
set -euo pipefail

usage() {
  echo "Usage: verify-loop.sh --goal \"<description>\" --verify \"<command>\" [--max N] [--receipt <path>] [--allowed-tools <tools>]"
  exit 1
}

GOAL="" VERIFY="" MAX=10 RECEIPT="" ALLOWED_TOOLS="Read,Edit,Bash(cargo *)"
while [ $# -gt 0 ]; do
  case "$1" in
    --goal) GOAL="$2"; shift 2 ;;
    --verify) VERIFY="$2"; shift 2 ;;
    --max) MAX="$2"; shift 2 ;;
    --receipt) RECEIPT="$2"; shift 2 ;;
    --allowed-tools) ALLOWED_TOOLS="$2"; shift 2 ;;
    *) usage ;;
  esac
done
[ -z "$GOAL" ] && usage
[ -z "$VERIFY" ] && usage

write_receipt() {
  local result="$1" iters="$2"
  [ -z "$RECEIPT" ] && return 0
  cat > "$RECEIPT" <<EOF
# Receipt: $(basename "$RECEIPT" .md)

- Goal: $GOAL
- Verify: \`$VERIFY\`
- Iterations: $iters / $MAX
- Result: $result
- Closed: $(date -u +%Y-%m-%dT%H:%MZ)
EOF
}

iter=0
session=""
last_output=""
identical_count=0

while [ "$iter" -lt "$MAX" ]; do
  iter=$((iter + 1))
  echo "== iteration $iter/$MAX =="

  if output=$(eval "$VERIFY" 2>&1); then
    echo "Gate green at iteration $iter."
    write_receipt "passed" "$iter"
    exit 0
  fi

  # Stuck condition: the same failure three times in a row means the loop
  # isn't converging — it needs a human, not more iterations burning budget.
  if [ "$output" = "$last_output" ]; then
    identical_count=$((identical_count + 1))
  else
    identical_count=0
  fi
  last_output="$output"
  if [ "$identical_count" -ge 2 ]; then
    echo "Same failure 3 times in a row. Stopping."
    write_receipt "stuck" "$iter"
    exit 2
  fi

  prompt="Goal: $GOAL
The verify gate failed. Fix the real cause, not the check.
Verify command: $VERIFY
Output:
$output"

  if [ -z "$session" ]; then
    session=$(claude -p "$prompt" \
      --allowedTools "$ALLOWED_TOOLS" \
      --output-format json | jq -r '.session_id')
  else
    claude -p "$prompt" \
      --allowedTools "$ALLOWED_TOOLS" \
      --resume "$session" >/dev/null
  fi
done

echo "Iteration ceiling ($MAX) reached without a green gate."
write_receipt "budget-exhausted" "$iter"
exit 1
