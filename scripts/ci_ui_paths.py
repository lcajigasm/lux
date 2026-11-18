#!/usr/bin/env python3
"""Decide whether a pull request should run the macOS UI suite.

Prints "true" or "false". A docs-only diff, a changelog, and a
version-only bump of Cargo.toml, Cargo.lock, and the macOS Info.plist
print "false". Anything else, including an empty diff, prints "true".
"""

from __future__ import annotations

import argparse
import re
import sys

VERSION_LINE = re.compile(r'^version = "[0-9]+\.[0-9]+\.[0-9]+"$')
PLIST_VERSION = re.compile(r"^<string>[0-9]+\.[0-9]+\.[0-9]+</string>$")
NAME_LINE = re.compile(r'^name = "([^"]+)"$')


def is_doc(path: str) -> bool:
    if path.startswith("docs/"):
        return True
    if path.endswith(".md"):
        return True
    return path in {"CHANGELOG", "CHANGELOG.md"}


def workspace_package(name: str | None) -> bool:
    if name is None:
        return False
    return name == "lux" or name == "xtask" or name.startswith("lux-")


def changed_body(lines: list[str]) -> list[str]:
    bodies = []
    for raw in lines:
        if not raw or raw[0] not in "+-":
            continue
        if raw.startswith(("+++", "---")):
            continue
        bodies.append(raw[1:].strip())
    return bodies


def version_file_ok(path: str, lines: list[str]) -> bool:
    bodies = changed_body(lines)
    if not bodies:
        return False
    if path == "Cargo.toml":
        return all(VERSION_LINE.match(body) for body in bodies)
    if path == "packaging/macos/Info.plist":
        return all(PLIST_VERSION.match(body) for body in bodies)
    if path == "Cargo.lock":
        package = None
        saw = False
        for raw in lines:
            if not raw:
                continue
            if raw[0] in "+-":
                prefix, body = raw[0], raw[1:].strip()
            elif raw[0] == " ":
                prefix, body = " ", raw[1:].strip()
            else:
                prefix, body = " ", raw.strip()
            if body.startswith("[[package]]"):
                package = None
                continue
            name = NAME_LINE.match(body)
            if name and prefix in " +-":
                package = name.group(1)
                if prefix in "+-":
                    return False
                continue
            if prefix not in "+-":
                continue
            saw = True
            if not VERSION_LINE.match(body) or not workspace_package(package):
                return False
        return saw
    return False


def files_in_diff(diff: str) -> dict[str, list[str]]:
    files: dict[str, list[str]] = {}
    path = None
    for line in diff.splitlines():
        if line.startswith("diff --git "):
            path = line.split(" b/", 1)[1]
            files.setdefault(path, [])
            continue
        if path is None:
            continue
        if line.startswith(("diff ", "index ", "@@", "new file", "deleted file", "old mode", "new mode", "similarity ", "rename ", "Binary files")):
            if line.startswith("Binary files"):
                files[path].append("+BINARY")
            continue
        if line.startswith(("+++", "---")):
            continue
        files[path].append(line)
    return files


def ui_should_run(diff: str) -> bool:
    files = files_in_diff(diff)
    if not files:
        return True
    for path, lines in files.items():
        if is_doc(path):
            continue
        if path in {"Cargo.toml", "Cargo.lock", "packaging/macos/Info.plist"}:
            if version_file_ok(path, lines):
                continue
        return True
    return False


def print_result(diff: str) -> None:
    print("true" if ui_should_run(diff) else "false")


RELEASE_DIFF = """\
diff --git a/CHANGELOG.md b/CHANGELOG.md
--- a/CHANGELOG.md
+++ b/CHANGELOG.md
@@ -1,2 +1,3 @@
+# 1.5.11
 unchanged
diff --git a/Cargo.lock b/Cargo.lock
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -1,4 +1,4 @@
 [[package]]
 name = "lux"
-version = "1.5.10"
+version = "1.5.11"
 dependencies = [
@@ -10,4 +10,4 @@ dependencies = [
 [[package]]
 name = "xtask"
-version = "1.5.10"
+version = "1.5.11"
diff --git a/Cargo.toml b/Cargo.toml
--- a/Cargo.toml
+++ b/Cargo.toml
@@ -1,2 +1,2 @@
 [workspace.package]
-version = "1.5.10"
+version = "1.5.11"
 edition = "2024"
diff --git a/docs/STATUS.md b/docs/STATUS.md
--- a/docs/STATUS.md
+++ b/docs/STATUS.md
@@ -1 +1 @@
-old
+new
diff --git a/packaging/macos/Info.plist b/packaging/macos/Info.plist
--- a/packaging/macos/Info.plist
+++ b/packaging/macos/Info.plist
@@ -1,4 +1,4 @@
     <key>CFBundleVersion</key>
-    <string>1.5.10</string>
+    <string>1.5.11</string>
     <key>CFBundleShortVersionString</key>
-    <string>1.5.10</string>
+    <string>1.5.11</string>
"""

DEPENDENCY_DIFF = """\
diff --git a/Cargo.toml b/Cargo.toml
--- a/Cargo.toml
+++ b/Cargo.toml
@@ -1,2 +1,2 @@
 [workspace.dependencies]
-serde = "1"
+serde = "2"
"""

LOCK_DEP_DIFF = """\
diff --git a/Cargo.lock b/Cargo.lock
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -1,4 +1,4 @@
 [[package]]
 name = "serde"
-version = "1.0.0"
+version = "2.0.0"
"""

PLIST_OTHER_DIFF = """\
diff --git a/packaging/macos/Info.plist b/packaging/macos/Info.plist
--- a/packaging/macos/Info.plist
+++ b/packaging/macos/Info.plist
@@ -1,2 +1,2 @@
     <key>CFBundleName</key>
-    <string>Lux</string>
+    <string>Lux Editor</string>
"""


def self_test() -> None:
    cases = [
        (RELEASE_DIFF, False),
        (DEPENDENCY_DIFF, True),
        (LOCK_DEP_DIFF, True),
        (PLIST_OTHER_DIFF, True),
        ("", True),
        (
            "diff --git a/docs/STATUS.md b/docs/STATUS.md\n--- a/docs/STATUS.md\n+++ b/docs/STATUS.md\n@@ -1 +1 @@\n-a\n+b\n",
            False,
        ),
        (
            "diff --git a/crates/lux/src/lib.rs b/crates/lux/src/lib.rs\n--- a/crates/lux/src/lib.rs\n+++ b/crates/lux/src/lib.rs\n@@ -1 +1 @@\n-a\n+b\n",
            True,
        ),
        (
            "diff --git a/.github/workflows/ci.yml b/.github/workflows/ci.yml\n--- a/.github/workflows/ci.yml\n+++ b/.github/workflows/ci.yml\n@@ -1 +1 @@\n-a\n+b\n",
            True,
        ),
    ]
    for diff, want in cases:
        got = ui_should_run(diff)
        if got != want:
            raise SystemExit(f"ci-ui-paths: expected {want} got {got}\n{diff[:180]}")
    print("ci-ui-paths: self-test ok")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--diff", help="Path of a unified diff. stdin when omitted.")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if args.diff:
        with open(args.diff, encoding="utf-8") as handle:
            diff = handle.read()
    else:
        diff = sys.stdin.read()
    print_result(diff)


if __name__ == "__main__":
    main()
