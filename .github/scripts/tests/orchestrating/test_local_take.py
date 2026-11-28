#!/usr/bin/env python3
"""The supervisor's take of the orchestrator lock backs off like a watch's machine failure.

An unreachable GitHub at the take is retried on `supervisor.BACKOFF`, never a
bare exit; a self-restart takes the lease it still holds at once.
"""

from __future__ import annotations

import io
import unittest
import unittest.mock
from pathlib import Path

from merging.watch import ending, supervisor
from orchestrating import lease, local
from tests.orchestrating.fakes import CLOUD, Remote


class Take(unittest.TestCase):
    def setUp(self):
        self.remote = Remote(self)
        self.root = self.remote.clone("desk")
        self.common = Path(self.root, ".git")
        self.slept: list[float] = []
        self.out = io.StringIO()

    def held(self, failures: int):
        real, calls = local.side.Side.take, []

        def flaky(mine):
            calls.append(1)
            if len(calls) <= failures:
                raise lease.Unreachable("github.com: connection reset")
            return real(mine)
        patch = unittest.mock.patch.object(local.side.Side, "take", flaky)
        patch.start()
        self.addCleanup(patch.stop)
        return local.held(self.root, self.common, lambda line: print(line, file=self.out), self.slept.append)

    def test_an_unreachable_first_take_retries_and_then_holds(self):
        with self.held(failures=1) as refused:
            self.assertIsNone(refused)
            self.assertEqual(lease.read(self.remote.side("a", CLOUD).git).holder, local.me())
        self.assertEqual(sum(self.slept), supervisor.BACKOFF[0])
        self.assertIn("retry 1 of 5; taking it again in 1 minute(s).", self.out.getvalue())
        self.assertNotIn("stopped.", self.out.getvalue())

    def test_five_failures_in_a_row_stop_with_the_machine_status(self):
        with self.held(failures=len(supervisor.BACKOFF) + 1) as refused:
            self.assertEqual(refused, ending.MACHINE_STATUS)
        self.assertEqual(sum(self.slept), sum(supervisor.BACKOFF))
        self.assertRegex(self.out.getvalue(), r"failed 6 times in a row; stopped\.\n$")

    def test_a_stop_during_the_wait_ends_it(self):
        (self.common / ending.STOP_FILE).parent.mkdir(parents=True, exist_ok=True)
        (self.common / ending.STOP_FILE).touch()
        with self.held(failures=1) as refused:
            self.assertEqual(refused, ending.STOPPED_STATUS)
        self.assertEqual(self.slept, [])

    def test_a_self_restarts_take_of_its_own_held_lease_succeeds_at_once(self):
        self.remote.side("desk", local.me()).take()  # held, fresh, by the process that re-executed
        with self.held(failures=0) as refused:
            self.assertIsNone(refused)
        self.assertEqual(self.slept, [])
        self.assertNotIn("held by", self.out.getvalue())


if __name__ == "__main__":
    unittest.main()
