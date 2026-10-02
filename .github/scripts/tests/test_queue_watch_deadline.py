#!/usr/bin/env python3
"""#697: `--watch --for MINUTES` ends cleanly between pull requests.

Past the deadline the watch takes nothing new; a deadline that passes during a
take lets that take finish first. Driven by the loop tests' fake world and clock.
"""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
import unittest.mock
from pathlib import Path

_spec = importlib.util.spec_from_file_location("queue_watch_loop", Path(__file__).with_name("test_queue_watch_loop.py"))
loop = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(loop)
queue, watch, pr = loop.queue, loop.watch, loop.pr


class SlowWorld(loop.World):
    """Each take costs `minutes` on the clock, like a CI wait."""

    def __init__(self, tmp, listings, verdicts, minutes):
        super().__init__(tmp, listings, verdicts)
        self.minutes = minutes

    def turn(self, number):
        self.now += self.minutes * 60
        return super().turn(number)


def run(listings, verdicts, *flags, take_minutes=0, **kw):
    with tempfile.TemporaryDirectory() as tmp:
        world = SlowWorld(tmp, listings, verdicts, take_minutes)
        results, why, clean = watch.run(world.fx, loop.opts(*flags, **kw))
        return world, results, why, clean


class Deadline(unittest.TestCase):
    def test_past_the_deadline_a_ready_one_is_not_taken(self):
        world, results, why, clean = run([[pr(1)]], {1: queue.MERGED}, "--for=0")
        self.assertEqual(world.taken, [])
        self.assertTrue(clean)
        self.assertTrue(why.startswith(watch.DEADLINE))
        self.assertIn("0 merged, nothing in hand", why)

    def test_a_deadline_passing_mid_take_finishes_the_take_first(self):
        verdicts = {1: queue.MERGED, 2: queue.MERGED}
        world, results, why, clean = run([[pr(1), pr(2)]], verdicts, "--for=10", take_minutes=30)
        self.assertEqual(world.taken, [1])
        self.assertEqual(results, [(1, queue.MERGED, "why")])
        self.assertTrue(clean)
        self.assertIn("1 merged", why)

    def test_takes_go_on_until_the_deadline(self):
        verdicts = {n: queue.MERGED for n in (1, 2, 3)}
        world, *_ = run([[pr(1), pr(2), pr(3)]], verdicts, "--for=25", take_minutes=10)
        self.assertEqual(world.taken, [1, 2, 3])
        world, *_ = run([[pr(1), pr(2), pr(3)]], verdicts, "--for=15", take_minutes=10)
        self.assertEqual(world.taken, [1, 2])

    def test_waiting_on_drafts_ends_at_the_deadline_not_the_idle_limit(self):
        world, _, why, clean = run([[pr(1, draft=True)]], {}, "--for=5", idle=90, poll=60)
        self.assertTrue(clean)
        self.assertTrue(why.startswith(watch.DEADLINE))
        self.assertLess(world.now, 90 * 60)

    def test_without_for_there_is_no_deadline(self):
        world, _, why, _ = run([[pr(1), pr(2)]], {1: queue.MERGED, 2: queue.MERGED}, take_minutes=600)
        self.assertEqual(world.taken, [1, 2])
        self.assertNotIn(watch.DEADLINE, why)

    def test_a_hand_back_still_exits_as_one(self):
        world, _, why, clean = run([[pr(1)]], {1: queue.HANDED_BACK}, "--for=1", take_minutes=30)
        self.assertFalse(clean)
        self.assertIn("#1 was handed back", why)


class Arguments(unittest.TestCase):
    def test_for_needs_watch(self):
        self.assertEqual(queue.parse(["--watch", "--for", "110"]).for_minutes, 110)
        self.assertIsNone(queue.parse(["--watch"]).for_minutes)
        with self.assertRaises(SystemExit), unittest.mock.patch("sys.stderr"):
            queue.parse(["524", "--for", "110"])


if __name__ == "__main__":
    unittest.main()
