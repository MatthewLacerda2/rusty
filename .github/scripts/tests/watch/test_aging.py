#!/usr/bin/env python3
"""#928: no pull request starves behind the conflict-first order (#753).

Past [`aging.AGE_MINUTES`] in line, or [`aging.AGE_PASSES`] takes ahead of it,
a pull request goes first whatever it conflicts with. Leaving the line resets
both. Driven by the loop tests' fake world and clock.
"""

from __future__ import annotations

import tempfile
import unittest

from merging.queue import outcomes
from merging.watch import aging, loop, order
from tests.watch import fakes
from tests.watch.test_select import BOTS, pr


class Line(unittest.TestCase):
    def test_age_counts_from_first_sight_in_line(self):
        line = aging.Line()
        line.see([1], 0.0)
        line.see([1, 2], 30 * 60)
        self.assertEqual(line.aged(59 * 60), set())
        self.assertEqual(line.aged(60 * 60), {1})

    def test_each_take_ahead_is_a_pass(self):
        line = aging.Line()
        line.see([1, 2], 0.0)
        for _ in range(aging.AGE_PASSES - 1):
            line.took(2)
        self.assertEqual(line.aged(0.0), set())
        line.took(2)
        self.assertEqual(line.aged(0.0), {1})
        self.assertIn("passed over 3 time(s)", line.said(1, 0.0))

    def test_leaving_the_line_starts_the_wait_afresh(self):
        line = aging.Line()
        line.see([1], 0.0)
        line.took(9), line.took(9), line.took(9)
        line.see([], 61 * 60)
        line.see([1], 62 * 60)
        self.assertEqual(line.aged(62 * 60), set())


class Order(unittest.TestCase):
    def test_an_aged_one_goes_first_whatever_its_conflicts(self):
        pulls = [pr(10), pr(11), pr(12)]
        clashes = {frozenset((10, 11)), frozenset((10, 12))}
        self.assertEqual(order.pick(pulls, {}, {}, set(), BOTS, clashes)["number"], 11)
        self.assertEqual(order.pick(pulls, {}, {}, set(), BOTS, clashes, {10})["number"], 10)

    def test_dependabot_stays_last_even_aged(self):
        self.assertEqual(order.pick([pr(5, bot=True), pr(6)], {}, {}, set(), BOTS, set(), {5})["number"], 6)


class InTheLoop(unittest.TestCase):
    def test_the_broad_one_waits_three_takes_then_goes(self):
        numbers = (10, 11, 12, 13, 14, 15)
        with tempfile.TemporaryDirectory() as tmp:
            world = fakes.World(tmp, [[pr(n) for n in numbers]], dict.fromkeys(numbers, outcomes.MERGED))
            world.clashes, said = {frozenset((10, n)) for n in numbers[1:]}, []
            world.fx.say = said.append
            loop.run(world.fx, fakes.opts())
        self.assertEqual(world.taken, [11, 12, 13, 10, 14, 15])
        self.assertTrue(any(s.startswith("watch: #10 has waited") for s in said))

    def test_an_hour_in_line_is_enough(self):
        with tempfile.TemporaryDirectory() as tmp:
            world = fakes.World(tmp, [[pr(10), pr(11), pr(12)]], dict.fromkeys((10, 11, 12), outcomes.MERGED))
            world.clashes = {frozenset((10, 11)), frozenset((10, 12))}
            world.fx.turn = lambda n, tick: (setattr(world, "now", world.now + 61 * 60), fakes.World.turn(world, n))[1]
            loop.run(world.fx, fakes.opts())
        self.assertEqual(world.taken, [11, 10, 12])  # one pass, but an hour


if __name__ == "__main__":
    unittest.main()
