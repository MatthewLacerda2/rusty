#!/usr/bin/env python3
"""#1510: a reviewer's `worktree-agent-*` branch and its detached checkout of a finished PR go."""

from __future__ import annotations

import json
import subprocess
import unittest
from pathlib import Path

from tests.reclaim.fixtures import Facts, Worktree, reclaim, rest_pull

MAIN = Path("/repo")
ON_MAIN = {"aaaa"}
# commit: the pull requests GitHub says hold it, as (state, head)
BY_COMMIT = {"m1": [("MERGED", "m1")], "o1": [("OPEN", "o1")], "x1": [("MERGED", "y1")],
             "own": [], "aaaa": [("MERGED", "aaaa")]}


def machine() -> reclaim.Machine:
    def answer(argv, **_):
        out, code = "", 0
        if argv[0] == "gh" and "commits/" in argv[2]:
            out = json.dumps([rest_pull(*pr) for pr in BY_COMMIT[argv[2].split("commits/")[1].split("/")[0]]])
        elif argv[0] == "gh":
            out = "[]"
        elif "get-url" in argv:
            out = "https://github.com/o/r.git"
        elif "--is-ancestor" in argv:
            code = 0 if argv[-2] in ON_MAIN else 1
        elif "rev-list" in argv:
            out = "0" if argv[5] in ON_MAIN else "1"
        elif "merge-tree" in argv or "--version" in argv:
            code = 1
        return subprocess.CompletedProcess(argv, code, out, "")
    return reclaim.Machine(answer)


def branch(name: str, tip: str) -> tuple[str, str]:
    return reclaim.judge_branch(machine(), MAIN, name, tip)


def detached(head: str, locked: str | None = None, busy: list[int] | None = None) -> tuple[str, str]:
    wt = Worktree(MAIN / ".claude" / "worktrees" / "agent-x", head, locked=locked)
    facts = machine().facts(wt, MAIN, cwds=[(pid, wt.path) for pid in busy or []])
    return reclaim.verdict(wt, facts)


class Deletes(unittest.TestCase):
    def test_an_agent_branch_at_an_origin_main_commit(self):
        self.assertEqual(branch("worktree-agent-x", "aaaa"),
                         ("remove", "an agent worktree's branch; nothing of its own"))

    def test_a_detached_checkout_of_a_merged_pr_s_head(self):
        self.assertEqual(detached("m1"), ("remove", "detached at a merged pull request's head"))


class Keeps(unittest.TestCase):
    def test_an_agent_branch_with_a_commit_on_no_remote(self):
        self.assertEqual(branch("worktree-agent-x", "own"), ("keep", "no pull request"))

    def test_any_other_branch_at_an_origin_main_commit(self):
        self.assertEqual(branch("fresh", "aaaa"), ("keep", "no pull request"))

    def test_a_detached_checkout_of_an_open_pr_s_head(self):
        self.assertEqual(detached("o1"), ("keep", "detached at an open pull request's head"))

    def test_a_commit_that_is_not_the_merged_pr_s_head(self):
        self.assertEqual(detached("x1"), ("keep", "detached, not on origin/main"))

    def test_a_locked_one(self):
        action, why = detached("m1", locked="claude agent agent-x (pid 9)")
        self.assertEqual(action, "protect")
        self.assertIn("locked (claude agent agent-x (pid 9))", why)

    def test_one_a_process_stands_in(self):
        self.assertEqual(detached("m1", busy=[42])[0], "protect")

    def test_github_not_answering(self):
        wt = Worktree(Path("/w"), "m1")
        self.assertEqual(reclaim.verdict(wt, Facts(prs=None))[0], "keep")


if __name__ == "__main__":
    unittest.main()
