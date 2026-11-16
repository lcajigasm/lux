#!/usr/bin/env python3
"""Decide the ok-gate commit status for a pull request.

Success requires a pull-request conversation comment by lcajigasm whose
body is exactly "OK" plus a lowercase hex prefix of the current head.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

AUTHOR = "lcajigasm"
PATTERN = re.compile(r"^\s*OK\s+([0-9a-f]{7,40})\s*$")
HEAD = re.compile(r"[0-9a-f]{7,40}")


def comment_login(comment: dict) -> str:
    if isinstance(comment.get("login"), str):
        return comment["login"]
    user = comment.get("user")
    if isinstance(user, dict) and isinstance(user.get("login"), str):
        return user["login"]
    return ""


def comment_body(comment: dict) -> str:
    body = comment.get("body")
    return body if isinstance(body, str) else ""


def decide(comments: list[dict], head: str) -> tuple[str, str]:
    """Return (state, description) for the ok-gate commit status."""
    current = head.strip().lower()
    if HEAD.fullmatch(current) is None:
        return "failure", "needs a head sha"
    short = current[:7]
    for comment in comments:
        if comment_login(comment) != AUTHOR:
            continue
        match = PATTERN.fullmatch(comment_body(comment))
        if match and current.startswith(match.group(1)):
            return "success", f"OK {short} from {AUTHOR}"
    return "failure", f"needs OK {short} from {AUTHOR}"


def comments_from_pages(pages: list) -> list[dict]:
    """Flatten `gh api --paginate --slurp` pages of issue comments."""
    comments = []
    for page in pages:
        if not isinstance(page, list):
            continue
        for item in page:
            if not isinstance(item, dict):
                continue
            user = item.get("user")
            login = ""
            if isinstance(user, dict) and isinstance(user.get("login"), str):
                login = user["login"]
            body = item.get("body")
            comments.append(
                {
                    "login": login,
                    "body": body if isinstance(body, str) else "",
                }
            )
    return comments


def fetch_comments(repo: str, number: int) -> list[dict]:
    proc = subprocess.run(
        [
            "gh",
            "api",
            "--paginate",
            "--slurp",
            f"repos/{repo}/issues/{number}/comments",
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise SystemExit(proc.returncode)
    return comments_from_pages(json.loads(proc.stdout or "[]"))


def write_output(path: str, state: str, description: str) -> None:
    with Path(path).open("a", encoding="utf-8") as handle:
        handle.write(f"state={state}\n")
        handle.write(f"description={description}\n")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Evaluate the ok-gate status.")
    parser.add_argument("--head")
    parser.add_argument("--pr", type=int)
    parser.add_argument("--repo")
    parser.add_argument("--comments-json")
    parser.add_argument("--github-output")
    args = parser.parse_args(argv)
    if not args.head:
        parser.error("--head is required")
    if args.comments_json:
        comments = json.loads(Path(args.comments_json).read_text(encoding="utf-8"))
    else:
        if args.pr is None or not args.repo:
            parser.error("--pr and --repo are required without --comments-json")
        comments = fetch_comments(args.repo, args.pr)
    state, description = decide(comments, args.head)
    if "\n" in description or len(description) > 140:
        raise SystemExit("status description is too long")
    if args.github_output:
        write_output(args.github_output, state, description)
    print(f"state={state}")
    print(f"description={description}")
    return 0


HEAD_A = "abcdef0123456789abcdef0123456789abcdef01"
HEAD_B = "fedcba9876543210fedcba9876543210fedcba98"


class OkGateTest(unittest.TestCase):
    def evaluate(self, comments: list[dict], head: str) -> tuple[str, str]:
        return decide(comments, head)

    def test_sha7_and_full_sha_pass(self) -> None:
        short = self.evaluate(
            [{"login": AUTHOR, "body": f"OK {HEAD_A[:7]}"}],
            HEAD_A,
        )
        full = self.evaluate(
            [{"login": AUTHOR, "body": f"OK {HEAD_A}"}],
            HEAD_A,
        )
        self.assertEqual(short, ("success", f"OK {HEAD_A[:7]} from {AUTHOR}"))
        self.assertEqual(full[0], "success")

    def test_wrong_sha_fails(self) -> None:
        state, description = self.evaluate(
            [{"login": AUTHOR, "body": f"OK {HEAD_B[:7]}"}],
            HEAD_A,
        )
        self.assertEqual(state, "failure")
        self.assertEqual(description, f"needs OK {HEAD_A[:7]} from {AUTHOR}")

    def test_other_author_fails(self) -> None:
        state, _ = self.evaluate(
            [{"user": {"login": "someone"}, "body": f"OK {HEAD_A[:7]}"}],
            HEAD_A,
        )
        self.assertEqual(state, "failure")

    def test_lowercase_ok_or_extra_text_fails(self) -> None:
        for body in (
            f"ok {HEAD_A[:7]}",
            f"OK {HEAD_A[:7]} thanks",
            f"Please OK {HEAD_A[:7]}",
            f"OK {HEAD_A[:6]}",
            f"OK {HEAD_A[:7].upper()}",
        ):
            state, _ = self.evaluate([{"login": AUTHOR, "body": body}], HEAD_A)
            self.assertEqual(state, "failure", body)

    def test_newest_push_invalidates_old_ok(self) -> None:
        comments = [
            {"login": AUTHOR, "body": f"OK {HEAD_A[:7]}"},
            {"login": AUTHOR, "body": f"OK {HEAD_A}"},
        ]
        state, description = self.evaluate(comments, HEAD_B)
        self.assertEqual(state, "failure")
        self.assertIn(HEAD_B[:7], description)
        still = self.evaluate(comments, HEAD_A)
        self.assertEqual(still[0], "success")

    def test_surrounding_whitespace_passes(self) -> None:
        state, _ = self.evaluate(
            [{"login": AUTHOR, "body": f"\n  OK\t{HEAD_A[:7]}  \n"}],
            HEAD_A,
        )
        self.assertEqual(state, "success")

    def test_comment_pages_keep_multiline_bodies(self) -> None:
        pages = [
            [
                {"user": {"login": AUTHOR}, "body": f"OK {HEAD_A[:7]}\n"},
                {"user": None, "body": None},
            ],
            [{"user": {"login": "bot"}, "body": f"OK {HEAD_A}"}],
        ]
        comments = comments_from_pages(pages)
        self.assertEqual(self.evaluate(comments, HEAD_A)[0], "success")
        self.assertEqual(comments[1]["login"], "")

    def test_cli_prints_the_decision(self) -> None:
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as handle:
            json.dump([{"login": AUTHOR, "body": f"OK {HEAD_A[:7]}"}], handle)
            path = handle.name
        proc = subprocess.run(
            [
                sys.executable,
                str(Path(__file__).resolve()),
                "--head",
                HEAD_A,
                "--comments-json",
                path,
            ],
            check=False,
            capture_output=True,
            text=True,
        )
        Path(path).unlink()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertIn("state=success", proc.stdout)


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(OkGateTest)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        sys.exit(0 if result.wasSuccessful() else 1)
    sys.exit(main())
