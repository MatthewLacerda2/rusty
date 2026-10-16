#!/usr/bin/env python3
"""#913: a worktree with unpushed work is kept, and `main` reports what it did."""

from __future__ import annotations

import json
import subprocess
import unittest
from pathlib import Path

from tests.reclaim.fixtures import MAIN, Facts, FakeMachine, Worktree, reclaim, run


class Unpushed(unittest.TestCase):
    def facts(self, stray: str, in_pr: bool) -> Facts:
        def answer(argv, **_):
            out = {"gh": json.dumps([{"state": "MERGED", "headRefOid": "pr"}])}.get(argv[0], "")
            if "rev-list" in argv:
                out = stray
            code = 1 if "--is-ancestor" in argv and not in_pr else 0
            return subprocess.CompletedProcess(argv, code, out, "")
        machine = reclaim.Machine(answer)
        return machine.facts(Worktree(Path("/w"), "h", "9-x"), Path("/repo"), cwds=[])

    def test_commits_stranded_by_a_deleted_remote_branch_are_safe_in_the_pr(self):
        self.assertEqual(self.facts("3", in_pr=True).unpushed, 0)

    def test_commits_nowhere_else_count_as_unpushed(self):
        self.assertEqual(self.facts("3", in_pr=False).unpushed, 3)


class Main(unittest.TestCase):
    def test_dry_run_names_everything_and_deletes_nothing(self):
        machine, out = run("--dry-run")
        self.assertEqual(machine.removed, [])
        self.assertNotIn(["git", "-C", str(MAIN), "worktree", "prune"], machine.calls)
        self.assertIn("would remove: merged (1-merged): pull request merged", out)
        self.assertIn("kept: open (2-open): pull request open", out)
        self.assertIn("PROTECTED: dirty (3-dirty)", out)
        self.assertIn("kept: agent (worktree-agent): no pull request", out)
        self.assertIn("target/ 3.0 GiB", out)
        self.assertIn("PROTECTED: queue (detached q111", out)
        self.assertIn("(pid 42)", out)
        self.assertNotIn("would remove: merged-too", out)

    def test_a_worktree_outside_the_root_is_named_never_removed(self):
        machine, out = run()
        self.assertNotIn("merged-too", machine.removed)
        self.assertIn("OUTSIDE: /elsewhere/merged-too (4-elsewhere, target/ 3.0 GiB)", out)

    def test_the_main_checkout_and_the_root_s_worktrees_are_not_outside(self):
        _, out = run("--dry-run")
        outside = [line for line in out.splitlines() if line.startswith("OUTSIDE")]
        self.assertEqual(len(outside), 1, outside)

    def test_removes_only_finished_safe_worktrees_then_prunes(self):
        machine, out = run()
        self.assertEqual(machine.removed, ["merged"])
        self.assertIn(["git", "-C", str(MAIN), "worktree", "prune"], machine.calls)
        self.assertIn("reclaim: removed 1 of 5 worktree(s); free disk 10.0 GiB -> 10.0 GiB", out)

    def test_unknown_argument_is_refused(self):
        with self.assertRaises(SystemExit):
            reclaim.main(["--force"], FakeMachine())


if __name__ == "__main__":
    unittest.main()
