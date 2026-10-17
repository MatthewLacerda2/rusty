#!/usr/bin/env python3
"""#869: a new run attempt on a handed-back head re-queues it, no push needed.

The watch remembers a hand-back's head (#751) and, beside it, the runs on that
head ([`handbacks.attempts`]). `gh run rerun` after a runner outage bumps an
attempt on the same head, so the head has not moved but its runs have: that
reads as movement. Driven by the loop tests' fake world and clock.
"""

from __future__ import annotations

import tempfile
import unittest

from merging.queue import outcomes
from merging.watch import handbacks, loop
from tests.watch import fakes
from tests.watch.fakes import pr


def run(runs_seen, ends_in, listings=None, recalled=None):
    """Hand #1 back, then report `runs_seen` in turn as the runs on its head."""
    with tempfile.TemporaryDirectory() as tmp:
        world = fakes.World(tmp, listings or [[pr(1, head="pushed1")]], {1: outcomes.HANDED_BACK})
        seen, ends, said = iter(runs_seen), iter(ends_in), []
        last = [None]
        world.fx.attempts = lambda n, sha: last.__setitem__(0, next(seen, last[0])) or last[0]
        world.fx.say = lambda line, *rest: said.append(line)

        def turn(number, tick=None):
            world.taken.append(number)
            world.verdicts[number] = next(ends)  # a merge drops it from the listing
            return number, world.verdicts[number], "why"

        world.fx.turn = turn
        for path, entries in (recalled or {}).items():
            handbacks.remember(world.fx.memory.with_name(path), entries)
        _, why, status = loop.run(world.fx, fakes.opts(idle=5, poll=60))
        marks = handbacks.recall(world.fx.memory.with_name(handbacks.RUNS_MEMORY))
        return world.taken, said, marks


class Attempts(unittest.TestCase):
    def test_the_spelling_changes_with_an_attempt_or_a_new_run(self):
        one = handbacks.attempts([{"id": 7, "run_attempt": 1}, {"id": 3}])
        self.assertEqual(one, "3.1 7.1")
        self.assertNotEqual(one, handbacks.attempts([{"id": 7, "run_attempt": 2}, {"id": 3}]))
        self.assertNotEqual(one, handbacks.attempts([{"id": 7}, {"id": 3}, {"id": 9}]))


class Requeue(unittest.TestCase):
    def test_a_re_run_on_the_same_head_is_taken_again(self):
        taken, said, _ = run(["1.1", "1.1", "1.2"], [outcomes.HANDED_BACK, outcomes.MERGED])
        self.assertEqual(taken, [1, 1])
        self.assertIn("watch: #1 has run again on its handed-back head; back in line.", said)

    def test_the_same_runs_keep_it_passed_over_and_are_remembered(self):
        taken, _, marks = run(["1.1"], [outcomes.HANDED_BACK])
        self.assertEqual((taken, marks), ([1], {1: "1.1"}))

    def test_github_not_answering_is_no_news(self):
        taken, _, _ = run(["1.1", None], [outcomes.HANDED_BACK])
        self.assertEqual(taken, [1])

    def test_a_draft_is_not_asked_about(self):
        asked = []
        memory, marks = {1: "sha1"}, {1: "1.1"}
        again = handbacks.rerun_heads([pr(1, draft=True)], memory, marks, lambda n, sha: asked.append(n) or "1.2")
        self.assertEqual((again, asked), ([], []))

    def test_an_entry_from_before_869_takes_todays_runs_as_its_mark(self):
        marks = {}
        again = handbacks.rerun_heads([pr(1)], {1: "sha1"}, marks, lambda n, sha: "1.1")
        self.assertEqual((again, marks), ([], {1: "1.1"}))
        self.assertEqual(handbacks.rerun_heads([pr(1)], {1: "sha1"}, marks, lambda n, sha: "1.2"), [1])

    def test_a_recalled_hand_back_re_run_while_no_watch_ran_is_taken(self):
        recalled = {handbacks.MEMORY.split("/")[-1]: {1: "sha1"}, handbacks.RUNS_MEMORY: {1: "1.1"}}
        taken, _, _ = run(["1.2"], [outcomes.MERGED], listings=[[pr(1)]], recalled=recalled)
        self.assertEqual(taken, [1])


if __name__ == "__main__":
    unittest.main()
