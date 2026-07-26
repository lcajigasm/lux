#!/usr/bin/env bash
# Enforces golden rule #5: no .rs file over 600 lines, no struct over 25 fields.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

python3 - <<'PYEOF'
import re
import sys
from pathlib import Path

MAX_LINES = 600
MAX_FIELDS = 25


def count_top_level_fields(body):
    depth = 0
    segments, current = [], []
    for ch in body:
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth = max(0, depth - 1)
        if ch == "," and depth == 0:
            segments.append("".join(current))
            current = []
        else:
            current.append(ch)
    segments.append("".join(current))
    return sum(1 for seg in segments if ":" in seg.strip())


violations = []
for path in sorted(Path(".").rglob("*.rs")):
    if "target" in path.parts or ".git" in path.parts:
        continue
    text = path.read_text(errors="replace")
    line_count = text.count("\n") + (1 if text and not text.endswith("\n") else 0)
    if line_count > MAX_LINES:
        violations.append(f"{path}: {line_count} lines (max {MAX_LINES})")

    # ponytail: heuristic struct-field counter, not a real Rust parser. It
    # ignores angle-bracket generics (ambiguous with `->` and comparisons) and
    # assumes rustfmt-style one-field-per-line formatting, which `cargo fmt
    # --check` already enforces elsewhere in CI. Upgrade to syn-based parsing
    # if a real struct ever produces a false negative.
    for m in re.finditer(r"\bstruct\s+([A-Za-z_][A-Za-z0-9_]*)[^{;]*\{", text):
        name = m.group(1)
        start = m.end() - 1
        depth, i = 0, start
        while i < len(text):
            if text[i] == "{":
                depth += 1
            elif text[i] == "}":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        body = text[start + 1 : i]
        fields = count_top_level_fields(body)
        if fields > MAX_FIELDS:
            line_no = text.count("\n", 0, m.start()) + 1
            violations.append(
                f"{path}: struct {name} at line {line_no} has {fields} fields (max {MAX_FIELDS})"
            )

if violations:
    print("check-limits: violations found")
    for v in violations:
        print(f"  - {v}")
    sys.exit(1)

print("check-limits: all files within the 600-line / 25-field limits.")
PYEOF
