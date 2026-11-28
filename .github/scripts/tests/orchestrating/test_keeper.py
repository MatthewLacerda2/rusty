"""#2667: the lease is renewed while a long command runs, and a loss stops the local watch."""

from __future__ import annotations

import io
import unittest
import unittest.mock
from pathlib import Path

from merging.watch import ending
from orchestrating import keeper, lease, local
from tests.orchestrating.fakes import CLOUD, Remote


class Beating(unittest.TestCase):
    def test_a_lost_lease_is_said_once_and_ends_the_renewals(self):
        lost, said = [], []
        beat = keeper.Keeper(lambda: "taken by cloud x", lost.append, said.append)
        self.assertFalse(beat.beat())
        self.assertEqual(lost, ["taken by cloud x"])

    def test_github_down_is_retried_never_taken_for_a_loss(self):
        def down():
            raise lease.Unreachable("no network")
        lost, said = [], []
        beat = keeper.Keeper(down, lost.append, said.append, every=60)
        self.assertTrue(beat.beat())
        self.assertEqual(lost, [])
        self.assertIn("trying again in 1 minute(s)", said[0])

    def test_the_thread_renews_on_its_beat_and_stops_with_the_block(self):
        renewed = []
        with keeper.Keeper(lambda: renewed.append(1), print, print, every=0.01):
            while len(renewed) < 2:
                pass
        self.assertGreaterEqual(len(renewed), 2)


class Supervisor(unittest.TestCase):
    """`make queue-daemon`'s side: `orchestrating/local.py`."""

    def setUp(self):
        self.remote = Remote(self)
        self.root = self.remote.clone("desk")
        self.common = Path(self.root, ".git")

    def held(self):
        out = io.StringIO()
        context = local.held(self.root, self.common, lambda line: print(line, file=out))
        return context, out

    def test_it_refuses_with_one_stopped_line_while_a_cloud_fire_holds_the_lock(self):
        self.remote.side("a", CLOUD).take()
        context, out = self.held()
        with context as refused:
            self.assertEqual(refused, lease.HELD_STATUS)
        self.assertRegex(out.getvalue(), rf"^supervisor: .*held by {CLOUD}.*; stopped\.\n$")

    def test_it_holds_the_lock_while_it_runs_and_frees_it_after(self):
        context, out = self.held()
        with context as refused:
            self.assertIsNone(refused)
            self.assertEqual(lease.read(self.remote.side("a", CLOUD).git).holder, local.me())
        self.assertIsNone(lease.read(self.remote.side("a", CLOUD).git).holder)
        self.assertIn("released the orchestrator lock", out.getvalue())

    def test_a_restarted_supervisor_takes_its_own_lease_back_at_once(self):
        self.remote.side("desk", local.me()).take()  # the one that died left it held
        context, _ = self.held()
        with context as refused:
            self.assertIsNone(refused)

    def test_losing_the_lock_asks_the_watch_to_stop(self):
        caught = {}

        class Caught(keeper.Keeper):
            def __init__(self, renew, lost, say, every=0):
                caught["lost"] = lost
                super().__init__(renew, lost, say, every=3600)
        context, out = self.held()
        with unittest.mock.patch.object(local.keeper, "Keeper", Caught), context:
            caught["lost"](f"taken by {CLOUD}")
        self.assertTrue((self.common / ending.STOP_FILE).exists())
        self.assertIn(f"supervisor: taken by {CLOUD}; asking the watch to stop", out.getvalue())


if __name__ == "__main__":
    unittest.main()
