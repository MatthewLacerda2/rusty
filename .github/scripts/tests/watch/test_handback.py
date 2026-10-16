#!/usr/bin/env python3
"""#751: under `--watch` a hand-back skips that pull request; the rest merge.

The starter still hears of it at once — one line per hand-back, beginning
`HANDED_BACK_LINE` — and from the exit status at the end, the most urgent
ending winning. A fix pushed to the handed-back branch re-queues it within
the same watch. Driven by the loop tests' fake world and clock.
"""

from __future__ import annotations

import tempfile
import unittest
import unittest.mock

from merging.queue import cli, outcomes, world
from merging.verdict import github
from merging.watch import ending, handbacks, loop
from tests.watch import fakes
from tests.watch.fakes import pr


def run(listings, verdicts, **kw):
    with tempfile.TemporaryDirectory() as tmp:
        world = fakes.World(tmp, listings, verdicts)
        said = []
        world.fx.say = lambda line, *rest: said.append(line)
        results, why, status = loop.run(world.fx, fakes.opts(**kw))
        return world, said, why, status, handbacks.recall(world.fx.memory)


class Skips(unittest.TestCase):
    def test_a_hand_back_is_announced_and_the_ones_behind_it_merge(self):
        verdicts = {1: outcomes.HANDED_BACK, 2: outcomes.MERGED, 3: outcomes.MERGED}
        world, said, why, status, memory = run([[pr(1, head="pushed1"), pr(2), pr(3)]], verdicts, idle=1, poll=30)
        self.assertEqual(world.taken, [1, 2, 3])
        self.assertEqual([s for s in said if s.startswith(ending.HANDED_BACK_LINE)], [ending.handed_back(1, "why")])
        self.assertEqual(memory, {1: "pushed1"})
        self.assertEqual(status, ending.HANDED_BACK_STATUS)
        self.assertIn("#1", why)

    def test_the_unmoved_head_is_not_retaken_in_the_same_watch(self):
        world, *_ = run([[pr(1, head="pushed1")]], {1: outcomes.HANDED_BACK}, idle=5, poll=60)
        self.assertEqual(world.taken, [1])

    def test_a_pushed_fix_is_taken_again_by_the_same_watch(self):
        listings = [[pr(1, head="pushed1")], [pr(1, head="pushed1")], [pr(1, head="fixed1")], []]
        with tempfile.TemporaryDirectory() as tmp:
            world = fakes.World(tmp, listings, {1: outcomes.HANDED_BACK})
            ends = iter([outcomes.HANDED_BACK, outcomes.MERGED])

            def turn(number, tick=None):
                world.taken.append(number)
                return number, next(ends), "why"

            world.fx.turn = turn
            _, why, status = loop.run(world.fx, fakes.opts(idle=5, poll=60))
        self.assertEqual(world.taken, [1, 1])
        self.assertEqual(status, ending.HANDED_BACK_STATUS)


class Status(unittest.TestCase):
    def test_the_most_urgent_ending_wins(self):
        self.assertEqual(ending.status([], False), 0)
        self.assertEqual(ending.status([4], False), ending.HANDED_BACK_STATUS)
        self.assertEqual(ending.status([], True), ending.MACHINE_STATUS)
        self.assertEqual(ending.status([4], True), ending.MACHINE_STATUS)

    def test_the_line_is_greppable_and_names_the_pull_request(self):
        line = ending.handed_back(7, "red CI.")
        self.assertTrue(line.startswith(f"{ending.HANDED_BACK_LINE} #7: red CI."))

    def test_main_returns_the_watch_status(self):
        verdicts = {1: outcomes.HANDED_BACK}
        with tempfile.TemporaryDirectory() as tmp:
            scripted = fakes.World(tmp, [[pr(1, head="pushed1")]], verdicts)
            fake = lambda repo, opts: scripted.fx  # noqa: E731
            with unittest.mock.patch.object(world, "effects", fake), \
                    unittest.mock.patch.object(github, "gh", lambda *a: {"nameWithOwner": "o/r"}), \
                    unittest.mock.patch("builtins.print"):
                self.assertEqual(cli.main(["--watch", "--no-check", "--idle=1", "--poll=30", f"--root={tmp}"]), ending.HANDED_BACK_STATUS)


if __name__ == "__main__":
    unittest.main()
