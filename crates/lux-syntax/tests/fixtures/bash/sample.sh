#!/usr/bin/env bash
# Fixture exercising common Bash constructs.
set -euo pipefail

name="${1:-world}"

greet() {
  echo "hello, $name"
}

if [ -n "$name" ]; then
  greet
else
  echo "no name given" >&2
  exit 1
fi
