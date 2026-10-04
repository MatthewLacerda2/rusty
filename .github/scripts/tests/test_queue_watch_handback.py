#!/usr/bin/env python3
"""#751: under `--watch` a hand-back skips that pull request; the rest merge.

The starter still hears of it at once — one line per hand-back, beginning
`HANDED_BACK_LINE` — and from the exit status at the end, the most urgent
ending winning. A fix pushed to the handed-back branch re-queues it within
the same watch. Driven by the loop tests' fake world and clock.
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


def run(listings, verdicts, **kw):
    with tempfile.TemporaryDirectory() as tmp:
        world = loop.World(tmp, listings, verdicts)
        said = []
        world.fx.say = lambda line, *rest: said.append(line)
        results, why, status = watch.run(world.fx, loop.opts(**kw))
        return world, said, why, status, watch.recall(world.fx.memory)


class Skips(unittest.TestCase):
    def test_a_hand_back_is_announced_and_the_ones_behind_it_merge(self):
        verdicts = {1: queue.HANDED_BACK, 2: queue.MERGED, 3: queue.MERGED}
        world, said, why, status, memory = run([[pr(1, head="pushed1"), pr(2), pr(3)]], verdicts, idle=1, poll=30)
        self.assertEqual(world.taken, [1, 2, 3])
        self.assertEqual([s for s in said if s.startswith(watch.HANDED_BACK_LINE)], [watch.handed_back(1, "why")])
        self.assertEqual(memory, {1: "pushed1"})
        self.assertEqual(status, watch.HANDED_BACK_STATUS)
        self.assertIn("#1", why)

    def test_the_unmoved_head_is_not_retaken_in_the_same_watch(self):
        world, *_ = run([[pr(1, head="pushed1")]], {1: queue.HANDED_BACK}, idle=5, poll=60)
        self.assertEqual(world.taken, [1])

    def test_a_pushed_fix_is_taken_again_by_the_same_watch(self):
        listings = [[pr(1, head="pushed1")], [pr(1, head="pushed1")], [pr(1, head="fixed1")], []]
        with tempfile.TemporaryDirectory() as tmp:
            world = loop.World(tmp, listings, {1: queue.HANDED_BACK})
            outcomes = iter([queue.HANDED_BACK, queue.MERGED])

            def turn(number):
                world.taken.append(number)
                return number, next(outcomes), "why"

            world.fx.turn = turn
            _, why, status = watch.run(world.fx, loop.opts(idle=5, poll=60))
        self.assertEqual(world.taken, [1, 1])
        self.assertEqual(status, watch.HANDED_BACK_STATUS)


class Status(unittest.TestCase):
    def test_the_most_urgent_ending_wins(self):
        self.assertEqual(watch.status([], False), 0)
        self.assertEqual(watch.status([4], False), watch.HANDED_BACK_STATUS)
        self.assertEqual(watch.status([], True), watch.MACHINE_STATUS)
        self.assertEqual(watch.status([4], True), watch.MACHINE_STATUS)

    def test_the_line_is_greppable_and_names_the_pull_request(self):
        line = watch.handed_back(7, "red CI.")
        self.assertTrue(line.startswith(f"{watch.HANDED_BACK_LINE} #7: red CI."))

    def test_main_returns_the_watch_status(self):
        verdicts = {1: queue.HANDED_BACK}
        with tempfile.TemporaryDirectory() as tmp:
            world = loop.World(tmp, [[pr(1, head="pushed1")]], verdicts)
            fake = lambda repo, opts: world.fx  # noqa: E731
            with unittest.mock.patch.object(queue, "effects", fake), \
                    unittest.mock.patch.object(queue.mergeable, "gh", lambda *a: {"nameWithOwner": "o/r"}), \
                    unittest.mock.patch("builtins.print"):
                self.assertEqual(queue.main(["--watch", "--no-check", "--idle=1", "--poll=30"]), watch.HANDED_BACK_STATUS)


if __name__ == "__main__":
    unittest.main()
