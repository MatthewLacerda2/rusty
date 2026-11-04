#!/usr/bin/env python3
"""#1799: a head that moves during the CI wait goes back in line, remembered
against nothing, and is taken again; a named take still hands it back.

On 2026-11-04 #1788 and #1773 were parked on the very fix pushed to them,
because the watch remembered the head GitHub reported after the move.
"""

from __future__ import annotations

import tempfile
import unittest
from unittest import mock

from merging.queue import cli, gitio, landing, outcomes, pulls, taking, update, waiting
from merging.verdict import reviewed
from merging.watch import handbacks, loop
from tests.queue.fixtures import READY, REVIEW
from tests.queue.test_listing_flicker import PULL, RUNNING, waited
from tests.verdict.fixtures import SHA
from tests.watch import fakes
from tests.watch.fakes import pr

MOVED = (outcomes.HEAD_MOVED, ["the head is 2222222, not the 1111111 this queue is watching."])


class Watched(unittest.TestCase):
    def run_watch(self, ends, listings, memory=None):
        with tempfile.TemporaryDirectory() as tmp:
            world = fakes.World(tmp, listings, {})
            if memory:
                handbacks.remember(world.fx.memory, memory)
            said, left = [], iter(ends)
            world.fx.say = lambda line, *rest: said.append(line)

            def turn(number, tick=None):
                world.taken.append(number)
                world.verdicts[number] = next(left)  # what the fake's `merged()` reads
                return number, world.verdicts[number], "the head is fixed11, not the pushed1 this queue is watching."

            world.fx.turn = turn
            loop.run(world.fx, fakes.opts(idle=5, poll=60))
            return world, said, handbacks.recall(world.fx.memory)

    def test_a_moved_head_is_not_remembered_and_is_taken_again(self):
        listings = [[pr(1, head="pushed1")], [pr(1, head="fixed1")], []]
        world, said, memory = self.run_watch([outcomes.REQUEUED, outcomes.MERGED], listings, {1: "older1"})
        self.assertEqual(world.taken, [1, 1])
        self.assertNotIn(1, memory)
        self.assertTrue(any(s.startswith("watch: #1 goes back in line: the head is fixed11") for s in said))

    def test_every_other_hand_back_still_remembers_its_head(self):
        # The listing's head differs from what GitHub reports now (#1645: that one is kept).
        listings = [[pr(1, head="listed1")], [pr(1, head="pushed1")]]
        world, _, memory = self.run_watch([outcomes.HANDED_BACK], listings)
        self.assertEqual((world.taken, memory), ([1], {1: "pushed1"}))


class Taken(unittest.TestCase):
    def take(self, watch):
        opts = cli.parse(["524", "--poll", "0"])
        opts.watch = watch  # the watch's own opts name no pull request
        with mock.patch.object(pulls, "look", return_value=READY), \
            mock.patch.object(reviewed, "comments", return_value=REVIEW), \
            mock.patch.object(gitio, "git"), mock.patch.object(gitio, "remote"), \
            mock.patch.object(update, "advance", return_value=(SHA, [])), \
            mock.patch.object(waiting, "wait_for", return_value=MOVED), \
            mock.patch("builtins.print"):
            _, state, why = taking.take("o/r", 524, opts)
        return state, why

    def test_under_the_watch_a_moved_head_goes_back_in_line(self):
        state, why = self.take(True)
        self.assertEqual(state, outcomes.REQUEUED)
        self.assertIn("Back in line on its new head", why)

    def test_a_named_take_still_hands_it_back(self):
        self.assertEqual(self.take(False)[0], outcomes.HANDED_BACK)


class LabelOff(unittest.TestCase):
    def take(self, watch, labels):
        opts = cli.parse(["524", "--poll", "0"])
        opts.watch = watch
        now = {**READY, "labels": labels}
        with mock.patch.object(pulls, "look", side_effect=[{**READY, "labels": [{"name": "queue"}]}, now]), \
            mock.patch.object(reviewed, "comments", return_value=REVIEW), \
            mock.patch.object(gitio, "git"), mock.patch.object(gitio, "remote"), \
            mock.patch.object(update, "advance", return_value=(SHA, [])), \
            mock.patch.object(waiting, "wait_for", return_value=(outcomes.GO, ["ci passed"])), \
            mock.patch.object(landing, "squash", return_value=(outcomes.MERGED, "")) as squash, \
            mock.patch("builtins.print"):
            _, state, _ = taking.take("o/r", 524, opts)
        return state, squash.called

    def test_a_label_taken_off_during_the_wait_stops_the_merge(self):
        self.assertEqual(self.take(True, []), (outcomes.REQUEUED, False))

    def test_a_label_kept_merges(self):
        self.assertEqual(self.take(True, [{"name": "queue"}]), (outcomes.MERGED, True))


class Waiting(unittest.TestCase):
    def test_a_third_sha_ends_the_wait_as_moved(self):
        state, lines = waited([RUNNING], {**PULL, "headRefOid": "9" * 40})
        self.assertEqual(state, outcomes.HEAD_MOVED, lines)


if __name__ == "__main__":
    unittest.main()
