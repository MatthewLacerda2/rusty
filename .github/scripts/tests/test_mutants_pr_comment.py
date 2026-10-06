#!/usr/bin/env python3
"""The on-request report lands on the pull request once, and always fits (#912)."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "mutants-pr-comment.py"
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


if __name__ == "__main__":
    unittest.main()
