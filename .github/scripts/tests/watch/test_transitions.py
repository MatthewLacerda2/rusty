#!/usr/bin/env python3
"""#909: the `--watch` loop says what changed on the board, one line each.

A pull request opening (draft or ready), turning ready, going back to draft or
closing gets its own line, so one `Monitor` on the watch's output replaces the
orchestrator's separate `gh pr list` poller. The first listing says nothing (a
relaunch does not replay the board), and a pull request the watch merged itself
is not reported closed. Driven by the loop tests' fake world and clock.
"""

from __future__ import annotations

import tempfile
import unittest

from merging.queue import outcomes
from merging.watch import board, loop
from tests.watch import fakes
from tests.watch.fakes import pr


def listed(*pulls) -> dict:
    return board.heads(list(pulls))


class Lines(unittest.TestCase):
    def test_the_first_listing_announces_nothing(self):
        self.assertEqual(board.transitions(None, listed(pr(1), pr(2, draft=True)), set()), [])

    def test_each_transition_has_its_line(self):
        before = listed(pr(1, draft=True), pr(2), pr(3))
        after = listed(pr(1), pr(2, draft=True), pr(4, draft=True), pr(5))
        self.assertEqual(board.transitions(before, after, set()), [
            "watch: #1 turned ready.",
            "watch: #2 back to draft.",
            "watch: #3 closed, not merged by this watch.",
            "watch: #4 opened as a draft.",
            "watch: #5 opened as ready.",
        ])

    def test_the_queue_label_coming_and_going_has_its_line(self):
        before = listed(pr(1, queued=False), pr(2), pr(3, draft=True, queued=False))
        after = listed(pr(1), pr(2, queued=False), pr(3, queued=True))
        self.assertEqual(board.transitions(before, after, set()), [
            "watch: #1 queued.",
            "watch: #2 unqueued.",
            "watch: #3 turned ready.",
            "watch: #3 queued.",
        ])

    def test_a_push_alone_is_not_a_transition(self):
        self.assertEqual(board.transitions(listed(pr(1, head="a")), listed(pr(1, head="b")), set()), [])

    def test_one_the_watch_merged_is_not_reported_closed(self):
        self.assertEqual(board.transitions(listed(pr(1), pr(2)), listed(), {1}), ["watch: #2 closed, not merged by this watch."])


class Loop(unittest.TestCase):
    def test_a_ready_pull_request_waits_for_its_label(self):
        listings = [[pr(1, queued=False)], [pr(1, queued=False)], [pr(1)]]
        with tempfile.TemporaryDirectory() as tmp:
            world = fakes.World(tmp, listings, {1: outcomes.MERGED})
            said = []
            world.fx.say = lambda line, *rest: said.append(line)
            loop.run(world.fx, fakes.opts(idle=1, poll=30))
        self.assertEqual(world.taken, [1])
        self.assertIn("watch: #1 queued.", said)

    def test_the_watch_prints_the_board_changes_as_it_polls(self):
        listings = [
            [pr(1, draft=True), pr(2, draft=True)],
            [pr(1, draft=True), pr(2, draft=True), pr(3, draft=True)],
            [pr(1), pr(3, draft=True)],
        ]
        with tempfile.TemporaryDirectory() as tmp:
            world = fakes.World(tmp, listings, {1: outcomes.MERGED})
            said = []
            world.fx.say = lambda line, *rest: said.append(line)
            loop.run(world.fx, fakes.opts(idle=1, poll=30))
        self.assertEqual(world.taken, [1])
        self.assertEqual([s for s in said if s.startswith("watch: #")], [
            "watch: #3 opened as a draft.",
            "watch: #1 turned ready.",
            "watch: #2 closed, not merged by this watch.",
        ])


if __name__ == "__main__":
    unittest.main()
