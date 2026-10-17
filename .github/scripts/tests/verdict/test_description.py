#!/usr/bin/env python3
"""The five description checks #966 adds beside #927's, each with a case it must
flag and a case it must pass, drawn from the pull requests that slipped."""

from __future__ import annotations

import unittest
from unittest import mock

from merging.queue import pulls
from merging.verdict import carried, closing, github, protocol
from tests.verdict.test_claims import answers, issue


def described(body: str, files=("src/app/mod.rs",), labels=(), login="MatthewLacerda2") -> dict:
    return {
        "body": body,
        "files": [{"path": p} for p in files],
        "labels": [{"name": n} for n in labels],
        "author": {"login": login},
    }


class OwnLine(unittest.TestCase):
    def test_a_keyword_inside_a_sentence_is_flagged_and_quoted(self):
        for line in ("Then close #769 with this diagnosis.", "It quoted its `Closes #769`.", "This resolved #767's gap."):
            lines = protocol.stray({"body": f"Closes #1\n\n{line}"})
            self.assertIn(line, lines[0], line)

    def test_keywords_on_their_own_lines_pass(self):
        # #759, #837 and #897 close several issues, one line each.
        self.assertEqual(protocol.stray({"body": "Closes #759\nCloses #760\n- Fixes #3\nresolves: #4\n\nRefs #5 in prose."}), [])


class Issueless(unittest.TestCase):
    def test_code_without_an_issue_is_refused(self):
        # #795: three `.rs` files, no issue, no `bug` label.
        self.assertIn("names no issue but changes code", carried.issueless(described("Tidy the shell."))[0])
        self.assertTrue(carried.issueless(described("API tweak.", files=["docs/api/Input.md"])))

    def test_docs_a_bug_dependabot_or_an_issue_pass(self):
        self.assertEqual(carried.issueless(described("Typo.", files=["README.md", "docs/ui.md"])), [])
        self.assertEqual(carried.issueless(described("Fix a crash.", labels=["bug"])), [])
        self.assertEqual(carried.issueless(described("Bump x.", login="app/dependabot")), [])
        self.assertEqual(carried.issueless(described("Closes #5")), [])
        self.assertEqual(carried.issueless(described("Refs #5")), [])


class SquashTitle(unittest.TestCase):
    def test_the_closed_issue_joins_the_title_before_the_pull_request(self):
        # `Bump rustls to 0.23.46 … (#943)` came from branch `919-…`.
        pull = {"title": "Bump rustls to 0.23.46", "number": 943, "body": "Closes #919"}
        self.assertEqual(pulls.subject(pull), "Bump rustls to 0.23.46 (#919) (#943)")
        pull = {"title": "Two at once (#1) (#9)", "number": 9, "body": "Closes #1\nCloses #2"}
        self.assertEqual(pulls.subject(pull), "Two at once (#1) (#2) (#9)")

    def test_a_title_already_carrying_its_issue_is_left_alone(self):
        pull = {"title": "Fix it (#12)", "number": 30, "body": "Closes #12"}
        self.assertEqual(pulls.subject(pull), "Fix it (#12) (#30)")


class Blocked(unittest.TestCase):
    def closes(self, blockers):
        with mock.patch.object(github, "gh", side_effect=answers({8: issue()}, {8: blockers})):
            return closing.closed_by("o/r", {"body": "Closes #8"})

    def test_an_open_recorded_blocker_refuses(self):
        lines = closing.refusal(self.closes([{"number": 7, "state": "open"}]))
        self.assertEqual(lines[0], "it closes #8, which is blocked by #7 (open).")

    def test_a_closed_or_removed_blocker_passes(self):
        self.assertEqual(closing.refusal(self.closes([{"number": 7, "state": "closed"}])), [])
        self.assertEqual(closing.refusal(self.closes([])), [])


class Images(unittest.TestCase):
    def test_an_added_capture_is_refused(self):
        lines = carried.images(described("x", files=["after.png", "src/a.rs", "shots/b.JPG"]))
        self.assertIn("`after.png`, `shots/b.JPG`", lines[0])

    def test_the_allowlist_and_a_deletion_pass(self):
        self.assertEqual(carried.images(described("x", files=["engine/icon/icon_64.png"])), [])
        removed = {"files": [{"filename": "after.png", "status": "removed"}]}
        self.assertEqual(carried.images(removed), [])


if __name__ == "__main__":
    unittest.main()
