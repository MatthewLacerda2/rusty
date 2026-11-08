#!/usr/bin/env python3
"""#1831: a stalled hand-back in the watch, driven by the fake world.

#1820's and #1827's runs went green after the queue had handed them back for
outwaiting them, and nothing brought them back. A stall now returns once
its runs are all green, and only a stall: a conflict hand-back waits for its
head however its runs end, and a stall ending cancelled or red is said once,
with no re-run made, and is a red hand-back from then on (#1951).
"""

from __future__ import annotations

import tempfile
import unittest

from merging.queue import outcomes
from merging.watch import ending, handbacks, loop, stalls
from tests.watch import fakes
from tests.verdict.fixtures import READY, built, gates_running
from tests.verdict.fixtures import run as listed
from tests.watch.fakes import pr

GREEN, OUT = (stalls.GREEN, ""), (stalls.OUT, "")
ENDED = (stalls.ENDED, "a `ci` run for pushed1 concluded cancelled.")
BACK = "watch: #1's runs, which its hand-back outwaited, went green; back in line."


def run(readings, ends_in, stale=None, recalled=None):
    """Take #1 to `ends_in` in turn, its head's runs reading `readings` in turn
    (the last repeats). Returns what was taken, said and asked, and the stalls kept."""
    with tempfile.TemporaryDirectory() as tmp:
        world = fakes.World(tmp, [[pr(1, head="pushed1")]], {1: None})
        world.stale = dict(stale or {})
        seen, ends, said, asked = iter(readings), iter(ends_in), [], []
        last = [None]

        def stalled(pull):
            asked.append(pull["number"])
            last[0] = next(seen, last[0])
            return last[0]

        def turn(number, tick=None):
            world.taken.append(number)
            world.verdicts[number] = next(ends)
            return number, world.verdicts[number], "why"

        def conflicts(pulls):
            found = {p["number"]: world.stale.pop(p["number"]) for p in pulls if p["number"] in world.stale}
            return found

        world.fx.stalled, world.fx.turn, world.fx.main_conflicts = stalled, turn, conflicts
        world.fx.attempts = lambda n, sha: "1.1"  # nobody re-runs anything
        world.fx.say = lambda line, *rest: said.append(line)
        for path, entries in (recalled or {}).items():
            handbacks.remember(world.fx.memory.with_name(path), entries)
        loop.run(world.fx, fakes.opts(idle=5, poll=60))
        kept = handbacks.recall(world.fx.memory.with_name(handbacks.STALLS_MEMORY))
        return world.taken, said, asked, kept


class StallLoop(unittest.TestCase):
    def test_a_stall_going_green_returns_once_and_merges(self):
        taken, said, _, kept = run([OUT, OUT, GREEN], [outcomes.STALLED, outcomes.MERGED])
        self.assertEqual((taken, said.count(BACK), kept), ([1, 1], 1, {}))

    def test_ci_dropping_out_of_one_listing_does_not_park_it(self):
        # The review's reproduction, through the real reader: out, `ci` missing
        # once, out, green. Read as an ending, the blink popped the stall.
        out = ([listed("ci", status="in_progress", conclusion=None), listed("lint")], {1: gates_running(), 2: built("lint")})
        blink, green = ([listed("lint")], {2: built("lint")}), ([listed("ci"), listed("lint")], {1: built("ci"), 2: built("lint")})
        listings = [stalls.reading(READY, *got) for got in (out, blink, out, green)]
        taken, said, _, _ = run(listings, [outcomes.STALLED, outcomes.MERGED])
        self.assertEqual((taken, said.count(BACK)), ([1, 1], 1))
        self.assertFalse([s for s in said if "ended without going green" in s])

    def test_its_hand_back_line_asks_for_no_re_run(self):
        _, said, _, kept = run([OUT], [outcomes.STALLED])
        line = next(s for s in said if s.startswith("HANDED BACK #1"))
        self.assertIn("comes back by itself", line)
        self.assertNotIn("gh run rerun", line)
        self.assertEqual(kept, {1: outcomes.STALLED})

    def test_a_stall_ending_cancelled_stays_passed_over_and_is_said_once(self):
        taken, said, asked, kept = run([ENDED], [outcomes.STALLED])
        ended = [s for s in said if "ended without going green" in s]
        # A red hand-back from here (#1951): its runs are read each pass, said once.
        self.assertEqual((taken, len(ended), asked[:2], kept), ([1], 1, [1, 1], {}))
        self.assertIn("concluded cancelled", ended[0])
        self.assertTrue(ended[0].startswith(ending.HANDED_BACK_LINE + " #1"), ended[0])

    def test_a_stall_ending_red_and_later_green_comes_back(self):
        # #1951: a stall that ended red is a red hand-back, re-read on state.
        red = (stalls.ENDED, "a `ci` run for pushed1 concluded failure.")
        taken, said, _, _ = run([red, GREEN], [outcomes.STALLED, outcomes.MERGED])
        self.assertEqual(taken, [1, 1])
        self.assertIn("watch: #1's runs on its handed-back head read green now; back in line.", said)

    def test_a_run_still_out_keeps_it_passed_over(self):
        taken, said, _, kept = run([OUT], [outcomes.STALLED])
        self.assertEqual((taken, kept), ([1], {1: outcomes.STALLED}))
        self.assertNotIn(BACK, said)

    def test_github_not_answering_is_no_news(self):
        taken, said, _, kept = run([None], [outcomes.STALLED])
        self.assertEqual((taken, kept), ([1], {1: outcomes.STALLED}))
        self.assertFalse([s for s in said if s.startswith("watch: #1's runs")])

    def test_a_red_hand_back_does_not_return_when_its_runs_go_green(self):
        taken, _, asked, _ = run([GREEN], [outcomes.HANDED_BACK])
        self.assertEqual((taken, asked), ([1], []))

    def test_a_conflict_hand_back_does_not_return_when_its_runs_go_green(self):
        taken, said, asked, _ = run([GREEN], [], stale={1: ["src/a.rs"]})
        self.assertEqual((taken, asked), ([], []))
        self.assertTrue(any(s.startswith("HANDED BACK #1") for s in said))

    def test_a_recalled_stall_whose_runs_went_green_while_no_watch_ran_is_taken(self):
        recalled = {handbacks.MEMORY.split("/")[-1]: {1: "pushed1"}, handbacks.STALLS_MEMORY: {1: outcomes.STALLED}}
        taken, said, _, _ = run([GREEN], [outcomes.MERGED], recalled=recalled)
        self.assertEqual(taken, [1])
        self.assertIn(BACK, said)


if __name__ == "__main__":
    unittest.main()
