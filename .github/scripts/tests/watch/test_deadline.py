#!/usr/bin/env python3
"""#697: `--watch --for MINUTES` ends cleanly between pull requests.

Past the deadline the watch takes nothing new; a deadline that passes during a
take lets that take finish first. Driven by the loop tests' fake world and clock.
"""

from __future__ import annotations

import tempfile
import unittest
import unittest.mock

from merging.queue import cli, outcomes
from merging.watch import ending, loop
from tests.watch import fakes
from tests.watch.fakes import pr


class SlowWorld(fakes.World):
    """Each take costs `minutes` on the clock, like a CI wait."""

    def __init__(self, tmp, listings, verdicts, minutes):
        super().__init__(tmp, listings, verdicts)
        self.minutes = minutes

    def turn(self, number, tick=None):
        self.now += self.minutes * 60
        return super().turn(number)


def run(listings, verdicts, *flags, take_minutes=0, **kw):
    with tempfile.TemporaryDirectory() as tmp:
        world = SlowWorld(tmp, listings, verdicts, take_minutes)
        results, why, status = loop.run(world.fx, fakes.opts(*flags, **kw))
        return world, results, why, status


class Deadline(unittest.TestCase):
    def test_past_the_deadline_a_ready_one_is_not_taken(self):
        world, results, why, status = run([[pr(1)]], {1: outcomes.MERGED}, "--for=0")
        self.assertEqual(world.taken, [])
        self.assertEqual(status, 0)
        self.assertTrue(why.startswith(ending.DEADLINE))
        self.assertIn("0 merged, nothing in hand", why)

    def test_a_deadline_passing_mid_take_finishes_the_take_first(self):
        verdicts = {1: outcomes.MERGED, 2: outcomes.MERGED}
        world, results, why, status = run([[pr(1), pr(2)]], verdicts, "--for=10", take_minutes=30)
        self.assertEqual(world.taken, [1])
        self.assertEqual(results, [(1, outcomes.MERGED, "why")])
        self.assertEqual(status, 0)
        self.assertIn("1 merged", why)

    def test_takes_go_on_until_the_deadline(self):
        verdicts = {n: outcomes.MERGED for n in (1, 2, 3)}
        world, *_ = run([[pr(1), pr(2), pr(3)]], verdicts, "--for=25", take_minutes=10)
        self.assertEqual(world.taken, [1, 2, 3])
        world, *_ = run([[pr(1), pr(2), pr(3)]], verdicts, "--for=15", take_minutes=10)
        self.assertEqual(world.taken, [1, 2])

    def test_waiting_on_drafts_ends_at_the_deadline_not_the_idle_limit(self):
        world, _, why, status = run([[pr(1, draft=True)]], {}, "--for=5", idle=90, poll=60)
        self.assertEqual(status, 0)
        self.assertTrue(why.startswith(ending.DEADLINE))
        self.assertLess(world.now, 90 * 60)

    def test_without_for_there_is_no_deadline(self):
        world, _, why, _ = run([[pr(1), pr(2)]], {1: outcomes.MERGED, 2: outcomes.MERGED}, take_minutes=600)
        self.assertEqual(world.taken, [1, 2])
        self.assertNotIn(ending.DEADLINE, why)

    def test_a_hand_back_before_the_deadline_still_shows_in_the_status(self):
        world, _, why, status = run([[pr(1)]], {1: outcomes.HANDED_BACK}, "--for=1", take_minutes=30)
        self.assertEqual(status, ending.HANDED_BACK_STATUS)
        self.assertTrue(why.startswith(ending.DEADLINE))


class StopFile(unittest.TestCase):
    def test_a_requested_stop_ends_between_takes_with_its_own_status(self):
        with tempfile.TemporaryDirectory() as tmp:
            world = SlowWorld(tmp, [[pr(1)]], {1: outcomes.MERGED}, 0)
            world.stop = True
            _, why, status = loop.run(world.fx, fakes.opts())
        self.assertEqual((world.taken, status), ([], ending.STOPPED_STATUS))
        self.assertTrue(why.startswith(ending.STOP))
        self.assertEqual(ending.status([3], True, True, True), ending.STOPPED_STATUS)


class Arguments(unittest.TestCase):
    def test_for_needs_watch(self):
        self.assertEqual(cli.parse(["--watch", "--for", "70"]).for_minutes, 70)
        self.assertIsNone(cli.parse(["--watch"]).for_minutes)
        with self.assertRaises(SystemExit), unittest.mock.patch("sys.stderr"):
            cli.parse(["524", "--for", "70"])


if __name__ == "__main__":
    unittest.main()
