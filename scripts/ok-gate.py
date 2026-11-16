#!/usr/bin/env python3
"""Decide the ok-gate commit status for a pull request.

Success requires a pull-request conversation comment whose body is exactly
"OK" plus a lowercase hex prefix of the current head. The author is
`user.login`: lcajigasm, or a login listed in the repository variable
OK_GATE_TRUSTED. The body never grants authorship.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import unittest
from datetime import datetime, timezone
from pathlib import Path

AUTHOR = "lcajigasm"
PATTERN = re.compile(r"^\s*OK\s+([0-9a-f]{7,40})\s*$")
HEAD = re.compile(r"[0-9a-f]{7,40}")


def parse_trusted(raw: str | None) -> set[str]:
    """Split a comma-separated login list. Empty means no extra accounts."""
    if not raw or not raw.strip():
        return set()
    return {part.strip() for part in raw.split(",") if part.strip()}


def allowed_logins(raw: str | None) -> set[str]:
    """lcajigasm always counts. The variable only adds logins."""
    return {AUTHOR} | parse_trusted(raw)


def comment_login(comment: dict) -> str:
    """The GitHub account on the comment. Never a name written in the body."""
    user = comment.get("user")
    if isinstance(user, dict) and isinstance(user.get("login"), str):
        return user["login"]
    login = comment.get("login")
    if isinstance(login, str):
        return login
    return ""


def comment_body(comment: dict) -> str:
    body = comment.get("body")
    return body if isinstance(body, str) else ""


def parse_time(value: object) -> datetime | None:
    if not isinstance(value, str) or not value.strip():
        return None
    text = value.strip()
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    try:
        parsed = datetime.fromisoformat(text)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed


def edited_after_head(comment: dict, head_at: str | None) -> bool:
    """An edit after the head commit's committer time does not count.

    Comments with no timestamps, and comments that were not edited, still
    count. When the head time is missing, any edit is rejected.
    """
    created = parse_time(comment.get("created_at"))
    updated = parse_time(comment.get("updated_at"))
    if created is None or updated is None or updated <= created:
        return False
    pushed = parse_time(head_at)
    if pushed is None:
        return True
    return updated > pushed


def decide(
    comments: list[dict],
    head: str,
    trusted: str | None = "",
    head_at: str | None = None,
) -> tuple[str, str]:
    """Return (state, description) for the ok-gate commit status."""
    current = head.strip().lower()
    if HEAD.fullmatch(current) is None:
        return "failure", "needs a head sha"
    short = current[:7]
    allowed = allowed_logins(trusted)
    for comment in comments:
        login = comment_login(comment)
        if login not in allowed:
            continue
        if edited_after_head(comment, head_at):
            continue
        match = PATTERN.fullmatch(comment_body(comment))
        if match and current.startswith(match.group(1)):
            return "success", f"OK {short} from {login}"
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
            created = item.get("created_at")
            updated = item.get("updated_at")
            comments.append(
                {
                    "login": login,
                    "body": body if isinstance(body, str) else "",
                    "created_at": created if isinstance(created, str) else "",
                    "updated_at": updated if isinstance(updated, str) else "",
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
    parser.add_argument("--head-at", default="")
    parser.add_argument("--pr", type=int)
    parser.add_argument("--repo")
    parser.add_argument("--comments-json")
    parser.add_argument("--github-output")
    parser.add_argument(
        "--trusted",
        default=None,
        help="Comma-separated logins. Defaults to the OK_GATE_TRUSTED env var.",
    )
    args = parser.parse_args(argv)
    if not args.head:
        parser.error("--head is required")
    if args.comments_json:
        comments = json.loads(Path(args.comments_json).read_text(encoding="utf-8"))
    else:
        if args.pr is None or not args.repo:
            parser.error("--pr and --repo are required without --comments-json")
        comments = fetch_comments(args.repo, args.pr)
    trusted = args.trusted
    if trusted is None:
        trusted = os.environ.get("OK_GATE_TRUSTED", "")
    state, description = decide(comments, args.head, trusted, args.head_at or None)
    if "\n" in description or len(description) > 140:
        raise SystemExit("status description is too long")
    if args.github_output:
        write_output(args.github_output, state, description)
    print(f"state={state}")
    print(f"description={description}")
    return 0


HEAD_A = "abcdef0123456789abcdef0123456789abcdef01"
HEAD_B = "fedcba9876543210fedcba9876543210fedcba98"
SERVICE = "lux-ok[bot]"


class OkGateTest(unittest.TestCase):
    def evaluate(
        self,
        comments: list[dict],
        head: str,
        trusted: str = "",
        head_at: str | None = None,
    ) -> tuple[str, str]:
        return decide(comments, head, trusted, head_at)

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

    def test_trusted_service_account_passes(self) -> None:
        state, description = self.evaluate(
            [{"user": {"login": SERVICE}, "body": f"OK {HEAD_A[:7]}"}],
            HEAD_A,
            trusted=f" other[bot] , {SERVICE} ",
        )
        self.assertEqual((state, description), ("success", f"OK {HEAD_A[:7]} from {SERVICE}"))

    def test_unlisted_account_fails(self) -> None:
        state, description = self.evaluate(
            [{"login": "other[bot]", "body": f"OK {HEAD_A[:7]}"}],
            HEAD_A,
            trusted=SERVICE,
        )
        self.assertEqual(state, "failure")
        self.assertEqual(description, f"needs OK {HEAD_A[:7]} from {AUTHOR}")

    def test_empty_trusted_list_accepts_only_owner(self) -> None:
        for raw in ("", "   ", ",,,"):
            state, _ = self.evaluate(
                [{"login": SERVICE, "body": f"OK {HEAD_A[:7]}"}],
                HEAD_A,
                trusted=raw,
            )
            self.assertEqual(state, "failure", raw)
        owner, _ = self.evaluate(
            [{"login": AUTHOR, "body": f"OK {HEAD_A[:7]}"}],
            HEAD_A,
            trusted="",
        )
        self.assertEqual(owner, "success")

    def test_owner_counts_when_the_list_names_other_accounts(self) -> None:
        state, _ = self.evaluate(
            [{"login": AUTHOR, "body": f"OK {HEAD_A[:7]}"}],
            HEAD_A,
            trusted=SERVICE,
        )
        self.assertEqual(state, "success")

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

    def test_author_is_user_login_not_body_text(self) -> None:
        pages = [
            [
                {
                    "login": AUTHOR,
                    "user": {"login": "someone"},
                    "body": f"OK {HEAD_A[:7]}",
                    "author": AUTHOR,
                }
            ]
        ]
        comments = comments_from_pages(pages)
        self.assertEqual(comments[0]["login"], "someone")
        state, _ = self.evaluate(comments, HEAD_A, trusted=AUTHOR)
        self.assertEqual(state, "failure")

    def test_edited_after_head_does_not_count(self) -> None:
        comment = {
            "login": AUTHOR,
            "body": f"OK {HEAD_A[:7]}",
            "created_at": "2026-11-16T10:00:00Z",
            "updated_at": "2026-11-16T12:00:00Z",
        }
        state, _ = self.evaluate([comment], HEAD_A, head_at="2026-11-16T11:00:00Z")
        self.assertEqual(state, "failure")
        service = {
            "login": SERVICE,
            "body": f"OK {HEAD_A[:7]}",
            "created_at": "2026-11-16T10:00:00Z",
            "updated_at": "2026-11-16T12:00:00Z",
        }
        state, _ = self.evaluate(
            [service],
            HEAD_A,
            trusted=SERVICE,
            head_at="2026-11-16T11:00:00Z",
        )
        self.assertEqual(state, "failure")

    def test_edited_before_head_counts(self) -> None:
        comment = {
            "login": SERVICE,
            "body": f"OK {HEAD_A[:7]}",
            "created_at": "2026-11-16T10:00:00Z",
            "updated_at": "2026-11-16T10:30:00Z",
        }
        state, _ = self.evaluate(
            [comment],
            HEAD_A,
            trusted=SERVICE,
            head_at="2026-11-16T11:00:00Z",
        )
        self.assertEqual(state, "success")

    def test_missing_head_time_rejects_an_edit(self) -> None:
        comment = {
            "login": AUTHOR,
            "body": f"OK {HEAD_A[:7]}",
            "created_at": "2026-11-16T10:00:00Z",
            "updated_at": "2026-11-16T10:00:01Z",
        }
        state, _ = self.evaluate([comment], HEAD_A, head_at="")
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
            {"login": SERVICE, "body": f"OK {HEAD_A}"},
        ]
        state, description = self.evaluate(comments, HEAD_B, trusted=SERVICE)
        self.assertEqual(state, "failure")
        self.assertIn(HEAD_B[:7], description)
        still = self.evaluate(comments, HEAD_A, trusted=SERVICE)
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
        self.assertEqual(comments[0]["created_at"], "")

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
                "--trusted",
                "",
                "--comments-json",
                path,
            ],
            check=False,
            capture_output=True,
            text=True,
            env={**os.environ, "OK_GATE_TRUSTED": SERVICE},
        )
        Path(path).unlink()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertIn("state=success", proc.stdout)

    def test_cli_trusted_env_accepts_the_service_account(self) -> None:
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as handle:
            json.dump([{"login": SERVICE, "body": f"OK {HEAD_A[:7]}"}], handle)
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
            env={**os.environ, "OK_GATE_TRUSTED": SERVICE},
        )
        Path(path).unlink()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertIn(f"description=OK {HEAD_A[:7]} from {SERVICE}", proc.stdout)


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(OkGateTest)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        sys.exit(0 if result.wasSuccessful() else 1)
    sys.exit(main())
