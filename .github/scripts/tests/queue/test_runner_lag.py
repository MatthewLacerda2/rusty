#!/usr/bin/env python3
"""#1109: a red run whose jobs lag the runs listing waits; it is not a code failure.

The runs listing can show a run `completed/failure` before its jobs listing
shows the gate concluded (the lag #1095 met on the pass side). Judged then,
[`runners.runner_red`] read no failed job at all and the queue handed an outage
back as the code's. Until the gate shows on the run's own attempt, whose red it
is waits.
"""

from __future__ import annotations

import subprocess
import unittest
from unittest import mock

from merging.queue import outcomes
from merging.verdict import runners

from tests.queue.fixtures import SPENT, asked
from tests.queue.test_runner_outage import CHANGES_CANCELLED, CODE_RED, ci_cancelled, waited
from tests.verdict import fixtures
from tests.verdict.fixtures import built, job, run

FAILED = run("ci", conclusion="failure")


def gate_going(jobs: list[dict]) -> list[dict]:
    """`jobs` with the gate's conclusion not yet listed."""
    return [job("ci-gate", None) if j["name"] == "ci-gate" else j for j in jobs]


def polled(ci_jobs: list[dict]) -> tuple[str, list[str]]:
    """One poll of a `ci` run listed failed, `lint` green, long after the push."""
    with mock.patch.object(subprocess, "run") as rerun:
        state, lines = asked(FAILED, run("lint"), jobs={1: ci_jobs, 2: built("lint")}, waited=SPENT)
    return state, lines + [f"reruns={rerun.call_count}"]


class RunnerLag(unittest.TestCase):
    def test_a_failed_run_whose_gate_is_unconcluded_is_not_yet_known(self):
        self.assertIsNone(runners.runner_red(FAILED, {1: gate_going(CHANGES_CANCELLED)}, "ci"))
        self.assertIsNone(runners.runner_red(FAILED, {1: gate_going(CODE_RED)}, "ci"))

    def test_a_gate_listed_only_for_an_older_attempt_is_not_yet_known(self):
        old = [{**j, "run_attempt": 1} for j in CODE_RED]
        self.assertIsNone(runners.runner_red(run("ci", conclusion="failure", run_attempt=2), {1: old}, "ci"))

    def test_the_queue_waits_while_the_gate_is_unconcluded(self):
        state, lines = polled(gate_going(CHANGES_CANCELLED))
        self.assertEqual(state, outcomes.WAIT, lines)
        self.assertIn("`ci-gate` still going", " ".join(lines))
        self.assertIn("reruns=0", lines)

    def test_the_queue_waits_while_no_job_is_listed(self):
        state, lines = polled([])
        self.assertEqual(state, outcomes.WAIT, lines)
        self.assertIn("no `ci-gate` job is listed yet", " ".join(lines))

    def test_a_concluded_gate_failure_over_a_code_red_is_judged_as_today(self):
        state, lines = polled(CODE_RED)
        self.assertEqual(state, outcomes.STOP, lines)
        self.assertIn("concluded failure", lines[0])

    def test_a_code_red_elsewhere_hands_back_without_waiting_on_the_lag(self):
        runs = [FAILED, run("lint", conclusion="failure")]
        lint_red = [job("changes", "success"), job("lint", "failure"), job("lint-gate", "failure")]
        self.assertEqual(runners.unread(fixtures.READY, runs, {1: [], 2: lint_red}), [])

    def test_an_outage_listed_for_the_current_attempt_is_still_re_run(self):
        # The lag catches up: the cancelled jobs show for the run's attempt, and
        # #869's re-run follows, once.
        lagging = ([FAILED, run("lint")], {1: gate_going(CHANGES_CANCELLED), 2: built("lint")})
        state, lines, calls = waited([lagging, ci_cancelled(), ci_cancelled(2)])
        self.assertEqual(calls, [["gh", "run", "rerun", "--failed", "1"]])
        self.assertTrue(lines[0].startswith(outcomes.RUNNERS), lines)
        self.assertEqual(state, outcomes.STOP)


if __name__ == "__main__":
    unittest.main()
