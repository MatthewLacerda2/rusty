#!/usr/bin/env python3
"""#881: a run is judged by its latest attempt, not the one the runs listing shows.

On 2026-10-06 the watch handed #864 back over run 37365886365, "concluded
failure": the runs listing still described its cancelled attempt 2 while the
run's jobs already showed attempt 3 green, and nothing new appeared afterwards
for #869's re-queue to notice. Here the run says attempt 2 and its jobs say 3.
"""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

TESTS = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("outage_latest", TESTS / "test_queue_runner_outage.py")
outage = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(outage)
queue, fixtures, mergeable = outage.queue, outage.fixtures, outage.mergeable
run, built = fixtures.run, fixtures.built

STALE = {"run_attempt": 2, "conclusion": "failure"}


def on(attempt, jobs):
    """These jobs, as GitHub lists them for `attempt` of their run."""
    return [{**j, "run_attempt": attempt} for j in jobs]


def lint(jobs, **fields):
    """`ci` green; `lint` listed as `fields`, its jobs as given."""
    return [run("ci"), run("lint", **fields)], {1: built("ci"), 2: jobs}


GREEN_3 = lint(on(3, built("lint")), **STALE)
GOING_3 = lint(on(3, [fixtures.job("changes", "success"), fixtures.job("lint", None)]), **STALE)
CANCELLED_2 = lint(on(2, outage.GATE_CANCELLED), run_attempt=2, conclusion="cancelled")


class Latest(unittest.TestCase):
    def test_attempt_2_cancelled_and_attempt_3_green_is_mergeable(self):
        ok, lines = mergeable.judge(fixtures.READY, *GREEN_3, fixtures.CODE)
        self.assertTrue(ok, lines)

    def test_the_run_reads_as_its_latest_attempt(self):
        runs, jobs = GREEN_3
        seen = mergeable.by_gate(runs[1], jobs, "lint")
        self.assertEqual((seen["run_attempt"], seen["conclusion"]), (3, "success"))

    def test_a_latest_attempt_still_going_is_a_run_still_going(self):
        runs, jobs = GOING_3
        state, _ = mergeable.assess(fixtures.READY, "lint", [runs[1]], jobs)
        self.assertEqual(state, mergeable.RUNNING)

    def test_a_code_red_on_the_latest_attempt_still_refuses(self):
        red = on(3, [fixtures.job("changes", "success"), fixtures.job("lint", "failure"), fixtures.job("lint-gate", "failure")])
        ok, lines = mergeable.judge(fixtures.READY, *lint(red, run_attempt=2, conclusion="success"), fixtures.CODE)
        self.assertFalse(ok)
        self.assertIn("concluded failure", lines[0])

    def test_jobs_no_newer_than_the_run_change_nothing(self):
        runs, jobs = CANCELLED_2
        self.assertIs(mergeable.latest(runs[1], jobs), runs[1])


class Queue(unittest.TestCase):
    def test_the_864_shape_merges_without_a_hand_back_or_a_re_run(self):
        state, lines, calls = outage.waited([GREEN_3])
        self.assertEqual((state, calls), (queue.GO, []), lines)

    def test_attempt_3_going_waits_then_merges(self):
        state, lines, calls = outage.waited([GOING_3, GOING_3, GREEN_3])
        self.assertEqual((state, calls), (queue.GO, []), lines)

    def test_our_own_re_run_merges_while_the_listing_still_says_attempt_2(self):
        state, lines, calls = outage.waited([CANCELLED_2, GREEN_3])
        self.assertEqual(state, queue.GO, lines)
        self.assertEqual(calls, [["gh", "run", "rerun", "--failed", "2"]])


if __name__ == "__main__":
    unittest.main()
