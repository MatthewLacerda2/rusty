#!/usr/bin/env python3
"""#869: a new run attempt on a handed-back head re-queues it, no push needed.

The watch remembers a hand-back's head (#751) and, beside it, the runs on that
head ([`watch.attempts`]). `gh run rerun` after a runner outage bumps an
attempt on the same head, so the head has not moved but its runs have: that
reads as movement. Driven by the loop tests' fake world and clock.
"""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path

_spec = importlib.util.spec_from_file_location("queue_watch_loop_rerun", Path(__file__).with_name("test_queue_watch_loop.py"))
loop = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(loop)
queue, watch, pr = loop.queue, loop.watch, loop.pr


def run(runs_seen, outcomes, listings=None, recalled=None):
    """Hand #1 back, then report `runs_seen` in turn as the runs on its head."""
    with tempfile.TemporaryDirectory() as tmp:
        world = loop.World(tmp, listings or [[pr(1, head="pushed1")]], {1: queue.HANDED_BACK})
        seen, ends, said = iter(runs_seen), iter(outcomes), []
        last = [None]
        world.fx.attempts = lambda n, sha: last.__setitem__(0, next(seen, last[0])) or last[0]
        world.fx.say = lambda line, *rest: said.append(line)

        def turn(number):
            world.taken.append(number)
            world.verdicts[number] = next(ends)  # a merge drops it from the listing
            return number, world.verdicts[number], "why"

        world.fx.turn = turn
        for path, entries in (recalled or {}).items():
            watch.remember(world.fx.memory.with_name(path), entries)
        _, why, status = watch.run(world.fx, loop.opts(idle=5, poll=60))
        marks = watch.recall(world.fx.memory.with_name(watch.RUNS_MEMORY))
        return world.taken, said, marks


class Attempts(unittest.TestCase):
    def test_the_spelling_changes_with_an_attempt_or_a_new_run(self):
        one = watch.attempts([{"id": 7, "run_attempt": 1}, {"id": 3}])
        self.assertEqual(one, "3.1 7.1")
        self.assertNotEqual(one, watch.attempts([{"id": 7, "run_attempt": 2}, {"id": 3}]))
        self.assertNotEqual(one, watch.attempts([{"id": 7}, {"id": 3}, {"id": 9}]))


class Requeue(unittest.TestCase):
    def test_a_re_run_on_the_same_head_is_taken_again(self):
        taken, said, _ = run(["1.1", "1.1", "1.2"], [queue.HANDED_BACK, queue.MERGED])
        self.assertEqual(taken, [1, 1])
        self.assertIn("watch: #1 has run again on its handed-back head; back in line.", said)

    def test_the_same_runs_keep_it_passed_over_and_are_remembered(self):
        taken, _, marks = run(["1.1"], [queue.HANDED_BACK])
        self.assertEqual((taken, marks), ([1], {1: "1.1"}))

    def test_github_not_answering_is_no_news(self):
        taken, _, _ = run(["1.1", None], [queue.HANDED_BACK])
        self.assertEqual(taken, [1])

    def test_a_draft_is_not_asked_about(self):
        asked = []
        memory, marks = {1: "sha1"}, {1: "1.1"}
        again = watch.rerun_heads([pr(1, draft=True)], memory, marks, lambda n, sha: asked.append(n) or "1.2")
        self.assertEqual((again, asked), ([], []))

    def test_an_entry_from_before_869_takes_todays_runs_as_its_mark(self):
        marks = {}
        again = watch.rerun_heads([pr(1)], {1: "sha1"}, marks, lambda n, sha: "1.1")
        self.assertEqual((again, marks), ([], {1: "1.1"}))
        self.assertEqual(watch.rerun_heads([pr(1)], {1: "sha1"}, marks, lambda n, sha: "1.2"), [1])

    def test_a_recalled_hand_back_re_run_while_no_watch_ran_is_taken(self):
        recalled = {watch.MEMORY.split("/")[-1]: {1: "sha1"}, watch.RUNS_MEMORY: {1: "1.1"}}
        taken, _, _ = run(["1.2"], [queue.MERGED], listings=[[pr(1)]], recalled=recalled)
        self.assertEqual(taken, [1])


if __name__ == "__main__":
    unittest.main()
