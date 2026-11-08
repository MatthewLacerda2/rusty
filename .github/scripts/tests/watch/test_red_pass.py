#!/usr/bin/env python3
"""#1951's review: what the watch does with a CI wait's own mark, and one read a pass.

A plain CI stop (nothing built) now carries the verdict's runs as its mark
([`outcomes.Handed`]), so the watch keeps that mark rather than asking GitHub
again, whose answer during an outage is none. And a stall that ends red is
read once in the pass it ends, not again by [`handbacks.greened`].
"""

from __future__ import annotations

import tempfile
import unittest

from merging.queue import outcomes
from merging.watch import handbacks, loop, stalls
from tests.watch import fakes
from tests.watch.fakes import pr

RED = (stalls.ENDED, "a `ci` run for pushed1 concluded failure.")


def run(ends_in, readings=(), attempts=None):
    """Take #1 to `ends_in` in turn; return the marks kept and the event log
    (`p` a listing, `s` a stall read, `r` a red re-read)."""
    with tempfile.TemporaryDirectory() as tmp:
        world = fakes.World(tmp, [[pr(1, head="pushed1")]], {1: None})
        ends, seen, events = iter(ends_in), iter(readings), []
        listing = world.fx.pulls

        def pulls():
            events.append("p")
            return listing()

        def turn(number, tick=None):
            world.taken.append(number)
            state, mark = next(ends)
            world.verdicts[number] = state
            return outcomes.Handed(number, state, "why", mark) if mark is not None else (number, state, "why")

        def reading(tag):
            def ask(pull):
                events.append(tag)
                return next(seen, RED)
            return ask

        world.fx.pulls, world.fx.turn = pulls, turn
        world.fx.stalled, world.fx.reddened = reading("s"), reading("r")
        world.fx.attempts = attempts or (lambda n, sha: None)
        world.fx.say = lambda line, *rest: None
        loop.run(world.fx, fakes.opts(idle=5, poll=60))
        return handbacks.recall(world.fx.memory.with_name(handbacks.RUNS_MEMORY)), events


class RedPass(unittest.TestCase):
    def test_a_plain_ci_stop_keeps_the_verdicts_mark(self):
        # GitHub fails every read (the default): the mark is the verdict's own.
        marks, _ = run([(outcomes.HANDED_BACK, "")])
        self.assertEqual(marks, {1: ""})  # a run appearing later differs from it (#869)

    def test_a_hand_back_without_a_mark_still_asks_github(self):
        marks, _ = run([(outcomes.HANDED_BACK, None)], attempts=lambda n, sha: "5.1")
        self.assertEqual(marks, {1: "5.1"})

    def test_a_stall_ending_red_is_read_once_in_the_pass_it_ends(self):
        _, events = run([(outcomes.STALLED, None)], readings=[RED])
        after = events[events.index("s") - 1:]
        self.assertEqual(after[:4], ["p", "s", "p", "r"])


if __name__ == "__main__":
    unittest.main()
