#!/usr/bin/env bash
# Enforces golden rule #3: a >10% regression against the stored criterion
# baseline fails the build. Run after `cargo bench -p lux-benches --bench
# <name> -- --baseline <baseline>`, which leaves fresh results in
# target/criterion/<bench>/new/ next to the stored target/criterion/<bench>/<baseline>/.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

BASELINE="${1:-main}"
THRESHOLD_PCT="${BENCH_REGRESSION_THRESHOLD:-10}"

python3 - "$BASELINE" "$THRESHOLD_PCT" <<'PYEOF'
import json
import sys
from pathlib import Path

baseline_name, threshold_pct = sys.argv[1], float(sys.argv[2])
root = Path("target/criterion")
if not root.exists():
    print("check-bench-regression: no target/criterion directory (nothing benchmarked); skipping.")
    sys.exit(0)

failures = []
checked = 0
for bench_dir in sorted(p for p in root.iterdir() if p.is_dir() and p.name != "report"):
    baseline_file = bench_dir / baseline_name / "estimates.json"
    new_file = bench_dir / "new" / "estimates.json"
    if not baseline_file.exists() or not new_file.exists():
        continue
    checked += 1
    baseline_mean = json.loads(baseline_file.read_text())["mean"]["point_estimate"]
    new_mean = json.loads(new_file.read_text())["mean"]["point_estimate"]
    pct_change = (new_mean - baseline_mean) / baseline_mean * 100
    status = "REGRESSION" if pct_change > threshold_pct else "ok"
    print(f"{bench_dir.name}: {baseline_mean:.1f}ns -> {new_mean:.1f}ns ({pct_change:+.1f}%) [{status}]")
    if pct_change > threshold_pct:
        failures.append(bench_dir.name)

if checked == 0:
    print(f"check-bench-regression: no benchmark had both a '{baseline_name}' baseline and a fresh run; nothing to compare.")
    sys.exit(0)

if failures:
    print(f"check-bench-regression: {len(failures)} benchmark(s) regressed more than {threshold_pct}%: {', '.join(failures)}")
    sys.exit(1)

print(f"check-bench-regression: all {checked} benchmark(s) within {threshold_pct}% of the '{baseline_name}' baseline.")
PYEOF
