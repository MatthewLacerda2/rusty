#!/usr/bin/env python3
"""#1173: an open bug issue names its kind: a root issue it is a sub-issue of, a
searched case, or the root itself."""

from __future__ import annotations

import unittest

from tests.board.test_board import NOW, Issue, board


def check(body: str, labels=("bug",), parent: int | None = None) -> str:
    issue = Issue(7, "t", set(labels), body, parent=parent)
    return "\n".join(board.issue_findings([issue], [], [], NOW, 24))


class Kind(unittest.TestCase):
    def test_flags_a_bug_without_the_line(self):
        self.assertIn("kind: #7 t: no `Kind:` line", check("Found by a review."))

    def test_passes_a_root_instance_with_its_sub_issue_link(self):
        self.assertEqual(check("Why\n\nKind: #1170\n", parent=1170), "")

    def test_flags_a_root_instance_without_the_link(self):
        self.assertIn("`Kind: #1170` but its sub-issue parent is none", check("Kind: #1170"))
        self.assertIn("parent is #1172", check("Kind: #1170", parent=1172))

    def test_a_case_names_its_search(self):
        self.assertEqual(check("Kind: case — searched outline, hull"), "")
        self.assertIn("names no search", check("Kind: case"))

    def test_passes_a_root(self):
        self.assertEqual(check("Kind: root"), "")
        self.assertEqual(check("Kind: root — one issue per red workflow"), "")

    def test_flags_an_unknown_or_repeated_kind(self):
        self.assertIn("is not `#N`", check("Kind: dunno"))
        self.assertIn("2 `Kind:` lines", check("Kind: root\nKind: root"))

    def test_only_open_bugs_outside_planning_are_checked(self):
        self.assertEqual(check("", labels=("feature",)), "")
        self.assertEqual(check("", labels=("bug", "planning")), "")


if __name__ == "__main__":
    unittest.main()
