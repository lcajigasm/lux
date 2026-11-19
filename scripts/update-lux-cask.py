#!/usr/bin/env python3
"""Replace the version and sha256 in Casks/lux.rb. Nothing else."""

from __future__ import annotations

import re
import sys
from pathlib import Path

VERSION_LINE = re.compile(r'^  version "[^"]+"$', re.M)
SHA_LINE = re.compile(r'^  sha256 "[^"]+"$', re.M)


def update_cask(text: str, version: str, sha: str) -> str:
    text, version_count = VERSION_LINE.subn(f'  version "{version}"', text, count=1)
    text, sha_count = SHA_LINE.subn(f'  sha256 "{sha}"', text, count=1)
    if version_count != 1 or sha_count != 1:
        raise SystemExit(
            f"could not update Casks/lux.rb (version={version_count}, sha256={sha_count})"
        )
    return text


def self_test() -> None:
    sample = (
        'cask "lux" do\n'
        '  version "1.5.10"\n'
        '  sha256 "abc"\n'
        "end\n"
    )
    got = update_cask(sample, "1.5.11", "def")
    if 'version "1.5.11"' not in got or 'sha256 "def"' not in got:
        raise SystemExit(got)
    if 'version "1.5.10"' in got:
        raise SystemExit("old version left in place")
    print("update-lux-cask: self-test ok")


def main() -> None:
    if len(sys.argv) == 2 and sys.argv[1] == "--self-test":
        self_test()
        return
    if len(sys.argv) != 4:
        raise SystemExit("usage: update-lux-cask.py Casks/lux.rb VERSION SHA")
    path, version, sha = sys.argv[1:]
    file = Path(path)
    file.write_text(update_cask(file.read_text(), version, sha))


if __name__ == "__main__":
    main()
