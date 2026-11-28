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

from merging.view import events
from merging.watch import detach, ending, supervisor
from orchestrating import lease, local
from tests.orchestrating.fakes import CLOUD, Remote


class Take(unittest.TestCase):
    def setUp(self):
        self.remote = Remote(self)
        self.root = self.remote.clone("desk")
        self.common = Path(self.root, ".git")
        self.slept: list[float] = []
        self.out = io.StringIO()

    def held(self, failures: int, error: str = "github.com: connection reset", during=None, **kwargs):
        real, calls = local.side.Side.take, []

        def flaky(mine):
            calls.append(1)
            if len(calls) <= failures:
                if during:
                    during()
                raise lease.Unreachable(error)
            return real(mine)
        patch = unittest.mock.patch.object(local.side.Side, "take", flaky)
        patch.start()
        self.addCleanup(patch.stop)
        return local.held(self.root, self.common, lambda line: print(line, file=self.out), self.slept.append,
                          **kwargs)

    def stop(self) -> Path:
        path = self.common / ending.STOP_FILE
        path.parent.mkdir(parents=True, exist_ok=True)
        path.touch()
        return path

    def test_an_unreachable_first_take_retries_and_then_holds(self):
        with self.held(failures=1) as refused:
            self.assertIsNone(refused)
            self.assertEqual(lease.read(self.remote.side("a", CLOUD).git).holder, local.me())
        self.assertEqual(sum(self.slept), supervisor.BACKOFF[0])
        self.assertIn("retry 1 of 5; taking it again in 1 minute(s).", self.out.getvalue())
        self.assertNotIn("stopped.", self.out.getvalue())

    def test_a_sixth_failure_in_a_row_stops_with_the_machine_status(self):
        with self.held(failures=len(supervisor.BACKOFF) + 1) as refused:
            self.assertEqual(refused, ending.MACHINE_STATUS)
        self.assertEqual(sum(self.slept), sum(supervisor.BACKOFF))
        self.assertRegex(self.out.getvalue(), r"failed 6 times in a row; stopped\.\n$")

    def test_a_stop_during_the_wait_ends_it_and_is_consumed(self):
        with self.held(failures=1, during=self.stop) as refused:
            self.assertEqual(refused, ending.STOPPED_STATUS)
        self.assertEqual(self.slept, [])
        self.assertFalse((self.common / ending.STOP_FILE).exists())

    def test_a_stale_stop_is_cleared_so_a_failing_first_take_still_retries(self):
        self.stop()
        with self.held(failures=1) as refused:
            self.assertIsNone(refused)
        self.assertIn(detach.CLEARED, self.out.getvalue())
        self.assertEqual(sum(self.slept), supervisor.BACKOFF[0])

    def test_a_restarted_supervisor_keeps_its_stop_and_obeys_it_once(self):
        stop = self.stop()
        with self.held(failures=1, restarted=True) as refused:
            self.assertEqual(refused, ending.STOPPED_STATUS)
        self.assertFalse(stop.exists())

    def test_a_worktree_removed_during_the_wait_ends_it(self):
        with self.held(failures=1, gone=lambda: self.root) as refused:
            self.assertEqual(refused, supervisor.GONE_STATUS)

    def test_a_multi_line_error_stays_on_the_one_line_the_events_filter_reads(self):
        with self.held(failures=len(supervisor.BACKOFF) + 1, error="fatal: one\nerror: two\n"):
            pass
        lines = [line for line in self.out.getvalue().splitlines() if "could not be taken" in line]
        self.assertEqual(len(lines), len(supervisor.BACKOFF) + 1)
        self.assertTrue(all("(fatal: one error: two)" in line for line in lines))
        self.assertFalse(any(events.keep(line, set()) for line in lines[:-1]))  # a retry carries on
        self.assertTrue(events.keep(lines[-1], set()))  # the give-up is passed on

    def test_a_self_restarts_take_of_its_own_held_lease_succeeds_at_once(self):
        self.remote.side("desk", local.me()).take()  # held, fresh, by the process that re-executed
        with self.held(failures=0) as refused:
            self.assertIsNone(refused)
        self.assertEqual(self.slept, [])
        self.assertNotIn("held by", self.out.getvalue())


if __name__ == "__main__":
    unittest.main()
