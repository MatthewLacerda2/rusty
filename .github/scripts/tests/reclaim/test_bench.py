#!/usr/bin/env python3
"""#2381: `make bench-ab`'s trees against reclaim: a run's lock protects them, a dead
run's does not, and the warm `bench-main` is kept between runs."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path

from tests.reclaim.fixtures import Worktree, reclaim

MAIN = Path("/repo")


def judge(name: str, locked: str | None, alive: bool):
    machine = reclaim.Machine(lambda argv, **_: subprocess.CompletedProcess(argv, 0, "", ""))
    machine.alive = lambda pid: alive
    machine.is_ancestor = lambda *_: True  # detached at an origin/main commit: finished by that alone
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


if __name__ == "__main__":
    unittest.main()
