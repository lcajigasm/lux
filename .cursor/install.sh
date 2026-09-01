#!/usr/bin/env bash
# Idempotent bootstrap for the Lux workspace.
#
# The Rust toolchain itself is pinned by rust-toolchain.toml (channel
# 1.97.1) and is expected to already be present in the base image; this
# script only adds the two things a bare image is missing to run the full
# `cargo run -p xtask` verify loop that CI runs:
#
#   1. libfontconfig1-dev — the desktop/render stack links against
#      fontconfig via yeslogic-fontconfig-sys, so the workspace won't build
#      without it. CI installs the same package (see .github/workflows/ci.yml).
#   2. cargo-deny — the `deny check` gate in xtask verify / CI needs it.
#
# It is safe to run repeatedly: every step is guarded so a warm environment
# short-circuits instead of redoing work.
set -euo pipefail

if ! dpkg -s libfontconfig1-dev >/dev/null 2>&1; then
  sudo apt-get update
  sudo apt-get install -y --no-install-recommends libfontconfig1-dev
fi

if ! command -v cargo-deny >/dev/null 2>&1; then
  cargo install cargo-deny --locked
fi

# Warm the build so the first real command is fast and any toolchain or
# dependency problem surfaces during setup rather than mid-task.
cargo build --workspace
