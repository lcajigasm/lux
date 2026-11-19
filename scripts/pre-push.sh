#!/usr/bin/env bash
# Run the checks that have to be green before a push. No macOS runner.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
