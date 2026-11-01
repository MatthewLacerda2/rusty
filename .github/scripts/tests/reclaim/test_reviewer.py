#!/usr/bin/env python3
"""#1538: a reviewer's checkout goes once GitHub has its commit, whichever PR it came from.

GitHub lists a commit's pull requests only while they are open when the commit
is not on `main`, and the queue rebases every reviewed head away from `main`,
so after a merge these checkouts have no PR at all: `r1` here.
"""

from __future__ import annotations

import json
import subprocess
import unittest
from pathlib import Path

from tests.reclaim.fixtures import Worktree, reclaim, rest_pull

MAIN = Path("/repo")
TREES = MAIN / ".claude" / "worktrees"
OPEN = {"o1"}  # commits an open PR holds; GitHub lists no PR for any other
NOT_ON_GITHUB = {"own"}


def machine(status: str = "", github: str = "up") -> reclaim.Machine:
    def answer(argv, **_):
        out, code, err = "", 0, ""
        if argv[0] == "gh" and "/pulls" in argv[2] and "commits/" in argv[2]:
            out = json.dumps([rest_pull("OPEN", "o1")] if argv[2].split("commits/")[1][:2] in OPEN else [])
        elif argv[0] == "gh" and "commits/" in argv[2]:
            if github == "down":
                code, err = 1, "gh: Bad Gateway (HTTP 502)"
            elif argv[2].split("commits/")[1] in NOT_ON_GITHUB:
                code, err = 1, "gh: No commit found for SHA (HTTP 422)"
        elif argv[0] == "gh":
            out = "[]"
        elif "get-url" in argv:
            out = "https://github.com/o/r.git"
        elif "status" in argv:
            out = status
        elif "--is-ancestor" in argv or "merge-tree" in argv or "--version" in argv:
            code = 1  # nothing here is on origin/main
        elif "rev-list" in argv:
            out = "1"  # fetched from a PR ref, so in no remote-tracking branch
        return subprocess.CompletedProcess(argv, code, out, err)
    found = reclaim.Machine(answer)
    found.alive = lambda pid: True  # a lock's owner runs: never the real process table (#1658)
    return found


def tree(name: str, head: str = "r1", branch: str | None = None, locked: str | None = None,
         busy: bool = False, **kw) -> tuple[str, str]:
    wt = Worktree(TREES / name, head, branch, locked)
    facts = machine(**kw).facts(wt, MAIN, cwds=[(42, wt.path)] if busy else [])
    return reclaim.verdict(wt, facts)


def branch(name: str, tip: str = "r1") -> tuple[str, str]:
    return reclaim.judge_branch(machine(), MAIN, name, tip)


HOLDS_NOTHING = ("remove", "a reviewer's checkout at a commit GitHub has")


class Removes(unittest.TestCase):
    def test_a_clean_agent_worktree_at_a_commit_github_has(self):
        self.assertEqual(tree("agent-x"), HOLDS_NOTHING)

    def test_an_agent_worktree_with_a_reviewer_branch_checked_out(self):
        self.assertEqual(tree("agent-x", branch="pr1531"), HOLDS_NOTHING)

    def test_a_reviewer_branch_no_worktree_holds(self):
        self.assertEqual(branch("pr1514"), HOLDS_NOTHING)

    def test_the_harness_branch_at_a_commit_not_on_main(self):
        self.assertEqual(branch("worktree-agent-x"), HOLDS_NOTHING)


class Keeps(unittest.TestCase):
    def test_locked(self):
        self.assertEqual(tree("agent-x", locked="claude agent agent-x (pid 9)")[0], "protect")

    def test_a_process_standing_in_it(self):
        self.assertEqual(tree("agent-x", busy=True)[0], "protect")

    def test_an_untracked_file(self):
        self.assertEqual(tree("agent-x", status="?? notes.md"),
                         ("protect", HOLDS_NOTHING[1] + ", but it has uncommitted changes"))

    def test_a_commit_github_does_not_have(self):
        self.assertEqual(tree("agent-x", head="own"), ("keep", "detached, not on origin/main"))

    def test_github_failing_is_said(self):
        self.assertEqual(tree("agent-x", github="down"), ("keep", "detached, not on origin/main, and"
                                                          " GitHub did not answer whether it has the commit"))

    def test_an_open_pr_holding_the_commit(self):
        self.assertEqual(tree("agent-x", head="o1"), ("keep", "detached at a commit of an open pull request"))

    def test_a_detached_worktree_not_an_agent_s(self):
        self.assertEqual(tree("queue"), ("keep", "detached, not on origin/main"))

    def test_any_other_branch_name(self):
        self.assertEqual(branch("feature-x"), ("keep", "no pull request"))
        self.assertEqual(branch("pr-1316"), ("keep", "no pull request"))


if __name__ == "__main__":
    unittest.main()
