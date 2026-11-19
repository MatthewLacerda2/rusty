#!/usr/bin/env python3
"""#2381: `make bench-ab`'s trees against reclaim: a run's lock protects them, a dead
run's does not, and the warm `bench-main` is kept between runs."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path

from tests.reclaim.fixtures import Worktree, reclaim

MAIN = Path("/repo")


def judge(name: str, locked: str | None, alive: bool, on_main: bool = True, pr_state: str | None = None):
    machine = reclaim.Machine(lambda argv, **_: subprocess.CompletedProcess(argv, 0, "", ""))
    machine.alive = lambda pid: alive
    machine.is_ancestor = lambda *_: on_main  # on main: detached at an origin/main commit, finished by that
    machine.pull_number = lambda n: None if pr_state is None else {"number": n, "state": pr_state}
    machine.pulls_of = lambda commit: []  # the local merge commit a power cut leaves: no PR holds it
    wt = Worktree(MAIN / ".claude" / "worktrees" / name, "m1", locked=locked)
    return reclaim.verdict(wt, machine.facts(wt, MAIN, procs=[]))


class BenchTrees(unittest.TestCase):
    def test_a_live_runs_lock_protects_its_tree_mid_run(self):
        action, why = judge("pr-7-head", "bench-ab pid 5", alive=True)
        self.assertEqual(action, "protect")
        self.assertIn("bench-ab pid 5", why)

    def test_a_dead_runs_lock_protects_nothing(self):
        self.assertEqual(judge("pr-7-head", "bench-ab pid 5", alive=False)[0], "remove")

    def test_the_warm_main_tree_is_kept_between_runs(self):
        for locked in (None, "bench-ab pid 5"):
            action, why = judge("bench-main", locked, alive=False)
            self.assertEqual(action, "keep")
            self.assertIn("warm main tree", why)


class PowerCut(unittest.TestCase):
    """A cut mid-run leaves `pr-<n>-head` at the local merge commit: judged by its name's PR."""

    def test_a_merged_or_closed_prs_tree_goes(self):
        for state in ("MERGED", "CLOSED"):
            for locked in (None, "bench-ab pid 5"):
                action, why = judge("pr-7-head", locked, alive=False, on_main=False, pr_state=state)
                self.assertEqual(action, "remove", why)

    def test_an_open_prs_tree_or_a_live_runs_is_kept(self):
        self.assertEqual(judge("pr-7-head", None, alive=False, on_main=False, pr_state="OPEN")[0], "keep")
        self.assertEqual(judge("pr-7-head", "bench-ab pid 5", alive=True, on_main=False,
                               pr_state="MERGED")[0], "protect")

    def test_github_silent_keeps_it(self):
        self.assertEqual(judge("pr-7-head", None, alive=False, on_main=False, pr_state=None)[0], "keep")


if __name__ == "__main__":
    unittest.main()
