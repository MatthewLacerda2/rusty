#!/usr/bin/env python3
"""#971: stray branches, bad names, stale assignments and unlabelled issues are found.

The pure checks take the board as data; `main` gets a fake `gh api` answering
from a table, so these run offline.
"""

from __future__ import annotations

import contextlib
import io
import json
import importlib.util
import subprocess
import sys
import unittest
from datetime import datetime, timedelta, timezone

from tests import SCRIPTS

_spec = importlib.util.spec_from_file_location("board", SCRIPTS / "board.py")
board = importlib.util.module_from_spec(_spec)
sys.modules["board"] = board  # dataclasses look their module up while loading
_spec.loader.exec_module(board)
Branch, Issue = board.Branch, board.Issue

NOW = datetime(2026, 10, 16, 20, 0, tzinfo=timezone.utc)
OLD = NOW - timedelta(days=90)


def branches(found: list[str]) -> str:
    return "\n".join(found)


class Branches(unittest.TestCase):
    def check(self, *items: Branch) -> str:
        return branches(board.branch_findings(list(items), NOW, 24))

    def test_flags_the_two_claude_strays(self):
        out = self.check(Branch("claude/gracious-cori-0wmcfl", 2, 316, [], OLD),
                         Branch("claude/keen-cerf-f7fahe", 4, 317, [], OLD))
        self.assertIn("stray: `claude/gracious-cori-0wmcfl`: 2 ahead, 316 behind", out)
        self.assertIn("stray: `claude/keen-cerf-f7fahe`", out)
        self.assertIn("name: `claude/keen-cerf-f7fahe`", out)

    def test_passes_a_branch_with_a_draft_pr(self):
        self.assertEqual(self.check(Branch("971-make-board", 3, 0, ["open"], OLD)), "")

    def test_a_fresh_branch_has_time_to_open_its_pr(self):
        self.assertEqual(self.check(Branch("971-make-board", 1, 0, [], NOW - timedelta(hours=1))), "")

    def test_flags_a_merged_branch_still_standing(self):
        self.assertIn("merged, not deleted: `919-rustls-advisories`",
                      self.check(Branch("919-rustls-advisories", 0, 5, ["merged"], OLD)))

    def test_names(self):
        for good in ("main", "163-fix-coverage-scope", "document-ai-parity", "dependabot/cargo/glam-0.30"):
            self.assertTrue(board.name_ok(good), good)
        for bad in ("Fix_Thing", "claude/x", "163-Fix", "a--b", "-a"):
            self.assertFalse(board.name_ok(bad), bad)

    def test_keep_exempts_a_branch_with_its_reason(self):
        board.KEEP["claude/x"] = "the operator's experiment"
        try:
            err = io.StringIO()
            with contextlib.redirect_stderr(err):
                self.assertEqual(self.check(Branch("claude/x", 1, 0, [], OLD)), "")
            self.assertIn("the operator's experiment", err.getvalue())
        finally:
            del board.KEEP["claude/x"]


class Issues(unittest.TestCase):
    def check(self, issue: Issue, branch_names=(), prs=()) -> str:
        return branches(board.issue_findings(
            [issue], [Branch(n) for n in branch_names], list(prs), NOW, 24))

    def test_flags_a_missing_type_label_but_not_on_planning(self):
        self.assertIn("no type label: #7", self.check(Issue(7, "t", {"agent"})))
        self.assertEqual(self.check(Issue(7, "t", {"planning"})), "")
        self.assertEqual(self.check(Issue(7, "t", {"feature"})), "")

    def test_flags_an_old_assignment_with_no_branch_or_pr(self):
        self.assertIn("stale assignment: #7 t: assigned 48h ago",
                      self.check(Issue(7, "t", {"feature"}, "", NOW - timedelta(hours=48))))

    def test_passes_a_fresh_assignment_or_one_being_worked(self):
        old = NOW - timedelta(hours=48)
        self.assertEqual(self.check(Issue(7, "t", {"feature"}, "", NOW - timedelta(hours=1))), "")
        self.assertEqual(self.check(Issue(7, "t", {"feature"}, "", old), ["7-slug"]), "")
        pr = {"head": {"ref": "doc-fix"}, "title": "x", "body": "Closes #7"}
        self.assertEqual(self.check(Issue(7, "t", {"feature"}, "", old), prs=[pr]), "")
        pr["body"] = "Closes #70"
        self.assertIn("stale", self.check(Issue(7, "t", {"feature"}, "", old), prs=[pr]))

    def test_an_inline_keep_marker_exempts_an_issue(self):
        with contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(self.check(Issue(7, "t", set(), "<!-- board: keep, a tracker -->")), "")


class Main(unittest.TestCase):
    def test_reads_github_over_rest_and_exits_1_on_findings(self):
        answers = {
            "repos/o/r/branches": [{"name": "main"}, {"name": "claude/keen-cerf-f7fahe"}],
            "repos/o/r/compare/main...claude%2Fkeen-cerf-f7fahe": {
                "ahead_by": 4, "behind_by": 317,
                "commits": [{"commit": {"committer": {"date": "2026-07-03T07:44:33Z"}}}]},
            "repos/o/r/pulls?state=all&head=o:claude%2Fkeen-cerf-f7fahe": [],
            "repos/o/r/issues?state=open": [
                {"number": 3, "title": "t3", "labels": [{"name": "bug"}], "assignees": [],
                 "body": "Kind: #9", "parent_issue_url": "https://api.github.com/repos/o/r/issues/9"},
                {"number": 4, "title": "pr", "labels": [], "pull_request": {}}],
            "repos/o/r/pulls?state=open": [],
        }

        def gh(argv, **_):
            path = argv[2].split("&per_page=")[0].split("?per_page=")[0]
            return subprocess.CompletedProcess(argv, 0, json.dumps(answers[path]), "")
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = board.main([], board.Gh(gh), repo="o/r", now=NOW)
        self.assertEqual(code, 1)
        self.assertIn("## Board: 2 finding(s)", out.getvalue())
        self.assertNotIn("#4", out.getvalue())

    def test_refuses_a_bad_argument(self):
        with self.assertRaises(SystemExit):
            board.main(["--hours", "x"], board.Gh(None), repo="o/r", now=NOW)


if __name__ == "__main__":
    unittest.main()
