#!/usr/bin/env python3
"""#913: `make reclaim` removes only finished worktrees, and never unsafe ones.

What it reads (the porcelain listing, the flags) and the verdict on each worktree.
"""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path

from tests.reclaim.fixtures import PORCELAIN, Facts, Worktree, reclaim


class Parse(unittest.TestCase):
    def test_reads_branches_detached_heads_and_locks(self):
        trees = reclaim.parse_worktrees(PORCELAIN + "\nworktree /d\nHEAD dddd\ndetached\n")
        self.assertEqual([t.branch for t in trees][:2], ["main", "1-merged"])
        self.assertEqual(trees[4].locked, "claude agent (pid 1)")
        self.assertIsNone(trees[1].locked)
        self.assertIsNone(trees[-1].branch)
        self.assertEqual(trees[-1].head, "dddd")


class Verdict(unittest.TestCase):
    wt = Worktree(Path("/w"), "h", "9-x")

    def test_pull_request_state_decides_a_branch(self):
        for prs, action in [(["MERGED"], "remove"), (["CLOSED"], "remove"), (["OPEN"], "keep"),
                            (["CLOSED", "OPEN"], "keep"), ([], "keep"), (None, "keep")]:
            self.assertEqual(reclaim.verdict(self.wt, Facts(prs=prs))[0], action, prs)

    def test_ancestry_decides_a_detached_head(self):
        detached = Worktree(Path("/w"), "h")
        self.assertEqual(reclaim.verdict(detached, Facts(on_main=True))[0], "remove")
        self.assertEqual(reclaim.verdict(detached, Facts(on_main=False))[0], "keep")

    def test_finished_but_unsafe_is_protected_and_named(self):
        locked = Worktree(Path("/w"), "h", "9-x", locked="")
        for wt, facts, says in [(self.wt, Facts(prs=["MERGED"], dirty=True), "uncommitted"),
                                (self.wt, Facts(prs=["MERGED"], unpushed=2), "2 commit(s)"),
                                (self.wt, Facts(prs=["MERGED"], current=True), "runs from"),
                                (self.wt, Facts(prs=["MERGED"], busy=[42]), "pid 42"),
                                (self.wt, Facts(prs=["MERGED"], busy=None), "no way to tell"),
                                (locked, Facts(prs=["MERGED"]), "locked")]:
            action, why = reclaim.verdict(wt, facts)
            self.assertEqual(action, "protect")
            self.assertIn(says, why)


class Cwds(unittest.TestCase):
    def test_reads_lsof_field_output_when_there_is_no_proc(self):
        if Path("/proc/self/cwd").exists():
            self.skipTest("Linux reads /proc, not lsof")
        lsof = "p12\nfcwd\nn/a/b\np13\nfcwd\nn/c\n"
        answer = lambda argv, **_: subprocess.CompletedProcess(argv, 1, lsof, "")
        machine = reclaim.Machine(answer)
        self.assertEqual(machine.cwds(), [(12, Path("/a/b")), (13, Path("/c"))])

    def test_no_lsof_output_is_unknown_not_empty(self):
        if Path("/proc/self/cwd").exists():
            self.skipTest("Linux reads /proc, not lsof")
        answer = lambda argv, **_: subprocess.CompletedProcess(argv, 1, "", "")
        self.assertIsNone(reclaim.Machine(answer).cwds())


if __name__ == "__main__":
    unittest.main()
