#!/usr/bin/env python3
"""#1658: a lock held by a dead harness agent protects nothing; a live one, or a supervisor's, still does."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path

from merging.watch import tenancy
from tests.reclaim.fixtures import Worktree, reclaim

MAIN = Path("/repo")
AGENT = "claude agent agent-ab12 (pid {} start 27562625)"


def judge(locked: str, alive: bool, dirty: bool = False):
    def answer(argv, **_):
        out = " M f" if dirty and "status" in argv else ""
        code = 0
        return subprocess.CompletedProcess(argv, code, out, "")
    machine = reclaim.Machine(answer)
    machine.alive = lambda pid: alive
    wt = Worktree(MAIN / ".claude" / "worktrees" / "agent-x", "m1", locked=locked)
    facts = machine.facts(wt, MAIN, cwds=[])
    return reclaim.verdict(wt, facts), facts


class DeadAgentLock(unittest.TestCase):
    def test_parses_the_harness_reason(self):
        self.assertEqual(tenancy.agent_pid_of(AGENT.format(3440856)), 3440856)
        self.assertEqual(tenancy.agent_pid_of("claude agent agent-x (pid 9)"), 9)
        for other in (None, "", tenancy.reason(7), "someone else"):
            self.assertIsNone(tenancy.agent_pid_of(other))

    def test_dead_owner_on_a_finished_checkout_is_removed(self):
        (action, _), facts = judge(AGENT.format(5), alive=False)
        self.assertEqual(action, "remove")
        self.assertEqual(facts.dead_agent, 5)

    def test_live_owner_is_kept(self):
        self.assertEqual(judge(AGENT.format(5), alive=True)[0][0], "protect")

    def test_dead_owner_with_uncommitted_changes_is_kept(self):
        (action, why), _ = judge(AGENT.format(5), alive=False, dirty=True)
        self.assertEqual(action, "protect")
        self.assertIn("uncommitted", why)

    def test_supervisors_dead_lock_is_still_stale(self):
        (action, why), _ = judge(tenancy.reason(5), alive=False)
        self.assertEqual(action, "protect")
        self.assertIn("stale lock", why)


if __name__ == "__main__":
    unittest.main()
