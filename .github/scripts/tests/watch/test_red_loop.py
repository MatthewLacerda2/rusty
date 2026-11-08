#!/usr/bin/env python3
"""#1951: a red hand-back is re-admitted on its head's state, not on a change of mark.

PR #1920 was handed back on 2026-11-08 for a `ci` run that failed during
GitHub's 504 outage. GitHub failed to give the #869 mark at the hand-back, the
run was re-run green, and the first read after that became the mark, so
nothing ever differed from it: a ready, green, labelled pull request sat
passed over for seven hours. Now a red hand-back carries the runs its verdict
read, and comes back whenever its unmoved head reads green.
"""

from __future__ import annotations

import tempfile
import unittest

from merging.queue import outcomes, waiting
from merging.watch import handbacks, loop, stalls
from tests.verdict.fixtures import READY, built
from tests.verdict.fixtures import run as listed
from tests.watch import fakes
from tests.watch.fakes import pr

GREEN, RED = (stalls.GREEN, ""), (stalls.ENDED, "a `ci` run for pushed1 concluded failure.")
BACK = "watch: #1's runs on its handed-back head read green now; back in line."


def run(readings, ends_in, attempts=None, queued=True, recalled=None):
    """Take #1 to `ends_in` in turn (an `outcomes.RED` one carries mark "9.1"),
    its head's runs reading `readings` in turn (the last repeats). Returns what
    was taken, said and asked, and the memory left behind."""
    with tempfile.TemporaryDirectory() as tmp:
        world = fakes.World(tmp, [[pr(1, head="pushed1", queued=queued)]], {1: None})
        seen, ends, said, asked, last = iter(readings), iter(ends_in), [], [], [None]

        def stalled(pull):
            asked.append(pull["number"])
            last[0] = next(seen, last[0])
            return last[0]

        def turn(number, tick=None):
            world.taken.append(number)
            state = next(ends)
            world.verdicts[number] = state
            return outcomes.Red(number, "a `ci` run concluded failure.", "9.1") if state == outcomes.RED else (number, state, "why")

        world.fx.stalled, world.fx.turn = stalled, turn
        world.fx.attempts = attempts or (lambda n, sha: None)  # GitHub fails every mark read
        world.fx.say = lambda line, *rest: said.append(line)
        for path, entries in (recalled or {}).items():
            handbacks.remember(world.fx.memory.with_name(path), entries)
        loop.run(world.fx, fakes.opts(idle=5, poll=60))
        kept = {name: handbacks.recall(world.fx.memory.with_name(name)) for name in
                (handbacks.RED_MEMORY, handbacks.RUNS_MEMORY, handbacks.WHY_MEMORY)}
        return world.taken, said, asked, kept


class RedLoop(unittest.TestCase):
    def test_1920_a_red_re_run_green_comes_back_with_github_failing_every_mark_read(self):
        taken, said, _, _ = run([RED, RED, GREEN], [outcomes.RED, outcomes.MERGED])
        self.assertEqual((taken, said.count(BACK)), ([1, 1], 1))

    def test_1920_recalled_with_the_green_attempt_as_its_mark_comes_back(self):
        # The memory #1920 was found with: its mark is the green re-run itself.
        recalled = {
            handbacks.MEMORY.split("/")[-1]: {1: "pushed1"},
            handbacks.RUNS_MEMORY: {1: "42468896745.2 42468896769.1"},
            handbacks.RED_MEMORY: {1: "42468896745.1 42468896769.1"},
        }
        taken, said, _, _ = run([GREEN], [outcomes.MERGED], attempts=lambda n, sha: "42468896745.2 42468896769.1", recalled=recalled)
        self.assertEqual(taken, [1])
        self.assertIn(BACK, said)

    def test_a_red_head_that_stays_red_stays_handed_back(self):
        taken, said, asked, kept = run([RED], [outcomes.RED])
        self.assertEqual((taken, said.count(BACK)), ([1], 0))
        self.assertGreater(len(asked), 1)  # asked again each pass, never re-admitted
        self.assertEqual(kept[handbacks.RED_MEMORY], {1: "9.1"})

    def test_the_mark_is_the_verdicts_runs_so_a_re_run_still_out_re_admits_it(self):
        # #869 stays: GitHub's first read already shows attempt 2 (still running);
        # taken as the mark, it would hide the re-run. The verdict's "9.1" does not.
        taken, said, _, _ = run([(stalls.OUT, "")], [outcomes.RED, outcomes.MERGED], attempts=lambda n, sha: "9.2")
        self.assertEqual(taken, [1, 1])
        self.assertIn("watch: #1 has run again on its handed-back head; back in line.", said)

    def test_its_hand_back_line_says_it_comes_back_on_green(self):
        _, said, _, _ = run([RED], [outcomes.RED])
        line = next(s for s in said if s.startswith("HANDED BACK #1"))
        self.assertIn("back by itself once its runs read green", line)

    def test_its_time_and_reason_are_kept_for_queue_status(self):
        _, _, _, kept = run([RED], [outcomes.RED])
        self.assertEqual(kept[handbacks.WHY_MEMORY][1]["why"], "a `ci` run concluded failure.")

    def test_one_without_the_queue_label_is_not_asked(self):
        taken, _, asked, _ = run([GREEN], [], queued=False, recalled={
            handbacks.MEMORY.split("/")[-1]: {1: "pushed1"}, handbacks.RED_MEMORY: {1: "9.1"}})
        self.assertEqual((taken, asked), ([], []))

    def test_github_not_answering_is_no_news(self):
        taken, said, _, _ = run([None], [outcomes.RED])
        self.assertEqual((taken, said.count(BACK)), ([1], 0))

    def test_a_plain_hand_back_does_not_return_when_its_runs_go_green(self):
        # A conflict, a landing refusal, a missing run: green CI fixes none of them.
        taken, _, asked, _ = run([GREEN], [outcomes.HANDED_BACK])
        self.assertEqual((taken, asked), ([1], []))


class RedMark(unittest.TestCase):
    def test_a_red_run_marks_the_runs_the_verdict_read(self):
        runs = [listed("ci", conclusion="failure"), listed("lint")]
        self.assertEqual(waiting.red_mark(READY, runs, {1: [], 2: built("lint")}), "1.1 2.1")

    def test_a_stop_with_nothing_red_is_not_a_red_hand_back(self):
        runs = [listed("ci"), listed("lint")]
        self.assertIsNone(waiting.red_mark(READY, runs, {1: built("ci"), 2: built("lint")}))


if __name__ == "__main__":
    unittest.main()
