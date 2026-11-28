#!/usr/bin/env python3
"""#2666: an open `human` issue says why it is held, in a `Human:` line in its body
or a comment, and the board reads comments for that label only."""

from __future__ import annotations

import contextlib
import io
import json
import subprocess
import unittest

from tests.board.test_board import NOW, Issue, board

FLAG = "no human reason: #7 t: ask the operator why it is held and post `Human: <his reason>`"


def check(body: str, comments=(), labels=("human", "infrastructure")) -> str:
    issue = Issue(7, "t", set(labels), body, comments=list(comments))
    return "\n".join(board.issue_findings([issue], [], [], NOW, 24))


class HumanReason(unittest.TestCase):
    def test_flags_a_human_issue_without_the_line(self):
        self.assertIn(FLAG, check("Needs a real window.", ["Any news?"]))

    def test_passes_a_line_in_the_body_or_a_comment(self):
        self.assertEqual(check("Why\n\nHuman: needs the Mac"), "")
        self.assertEqual(check("Why", ["first", "> **Human:** parked behind the batch"]), "")
        self.assertEqual(check("", ["Human: old", "Human: new reason"]), "")

    def test_ignores_a_line_in_a_fenced_block(self):
        self.assertIn(FLAG, check("```\nHuman: an example\n```", ["~~~\nHuman: x\n~~~"]))

    def test_only_human_issues_are_checked(self):
        self.assertEqual(check("", labels=("infrastructure",)), "")


class Gather(unittest.TestCase):
    def test_reads_comments_for_human_issues_only(self):
        asked = []
        answers = {
            "repos/o/r/branches": [{"name": "main"}],
            "repos/o/r/issues?state=open": [
                {"number": 3, "title": "t3", "labels": [{"name": "human"}, {"name": "feature"}],
                 "assignees": [], "body": ""},
                {"number": 5, "title": "t5", "labels": [{"name": "feature"}], "assignees": []}],
            "repos/o/r/issues/3/comments": [{"body": "Human: needs speakers"}],
            "repos/o/r/pulls?state=open": [],
        }

        def gh(argv, **_):
            path = argv[2].split("&per_page=")[0].split("?per_page=")[0]
            asked.append(path)
            return subprocess.CompletedProcess(argv, 0, json.dumps(answers[path]), "")
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(board.main([], board.Gh(gh), repo="o/r", now=NOW), 0)
        self.assertIn("repos/o/r/issues/3/comments", asked)
        self.assertNotIn("repos/o/r/issues/5/comments", asked)


if __name__ == "__main__":
    unittest.main()
