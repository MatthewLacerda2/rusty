#!/usr/bin/env python3
"""The on-request report lands on the pull request once, and always fits (#912).

It lands on a merged pull request too, never on one closed unmerged (#1530).
"""

from __future__ import annotations

import importlib.util
import unittest

from tests import SCRIPTS

SCRIPT = SCRIPTS / "mutants-pr-comment.py"
_spec = importlib.util.spec_from_file_location("mutants_pr_comment", SCRIPT)
comment = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(comment)

URL = "https://github.com/o/r/actions/runs/1"


class Body(unittest.TestCase):
    def test_a_short_report_is_posted_whole_under_the_marker(self) -> None:
        text = comment.body("## Mutation on request\n\n2 missed\n", URL)
        self.assertTrue(text.startswith(comment.MARKER))
        self.assertIn("2 missed", text)
        self.assertIn(URL, text)
        self.assertNotIn("cut here", text)

    def test_a_long_report_is_cut_at_a_line_and_points_to_the_run(self) -> None:
        report = "\n".join(f"line {i} " + "x" * 50 for i in range(5000))
        text = comment.body(report, URL, limit=2000)
        self.assertLessEqual(len(text), 2000)
        self.assertTrue(text.startswith(comment.MARKER))
        self.assertIn("cut here", text)
        self.assertIn(URL, text)
        kept = text.split("\n\n*The report is cut")[0].splitlines()[1:]
        self.assertTrue(all(line.endswith("x" * 50) for line in kept))

    def test_the_real_limit_is_github_s(self) -> None:
        text = comment.body("y" * 100 + "\n" + "z" * 100_000, URL)
        self.assertLessEqual(len(text), 65536)


class Ours(unittest.TestCase):
    def test_finds_the_earlier_run_s_comment(self) -> None:
        comments = [{"id": 1, "user": "someone", "body": "lgtm"},
                    {"id": 2, "user": comment.BOT, "body": comment.MARKER + "\nold"}]
        self.assertEqual(comment.ours(comments), 2)

    def test_none_means_post_a_new_one(self) -> None:
        self.assertIsNone(comment.ours([]))
        self.assertIsNone(comment.ours([{"id": 3, "user": comment.BOT, "body": "ci says"}]))

    def test_a_human_quoting_the_marker_is_never_edited(self) -> None:
        quoted = [{"id": 4, "user": "someone", "body": comment.MARKER + "\nquoted"}]
        self.assertIsNone(comment.ours(quoted))


class Target(unittest.TestCase):
    def test_an_open_pull_request_wins(self) -> None:
        prs = [{"number": 1, "state": "closed", "merged_at": "2026-10-28T10:00:00Z"},
               {"number": 2, "state": "open", "merged_at": None}]
        self.assertEqual(comment.target(prs), 2)

    def test_else_the_latest_merged_one(self) -> None:
        prs = [{"number": 1, "state": "closed", "merged_at": "2026-10-27T10:00:00Z"},
               {"number": 3, "state": "closed", "merged_at": "2026-10-28T10:00:00Z"}]
        self.assertEqual(comment.target(prs), 3)

    def test_closed_unmerged_or_none_gets_nothing(self) -> None:
        self.assertIsNone(comment.target([{"number": 4, "state": "closed", "merged_at": None}]))
        self.assertIsNone(comment.target([]))


if __name__ == "__main__":
    unittest.main()
