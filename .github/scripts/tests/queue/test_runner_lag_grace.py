#!/usr/bin/env python3
"""#1182's review: a red run whose jobs listing still lags is judged as listed
once the grace is spent, so a lagging runner outage is still re-run (#869).

Within [`polling.GATE_SHOWS_SECONDS`] a red whose jobs do not
[`runlist.agrees`] it is over waits, whatever it concluded. Past it,
[`runners.runner_red`] reads the jobs the way it did before #1182: a gate that
concluded over a cancelled job is the runners', however many jobs are still
listed going.
"""

from __future__ import annotations

import unittest

from merging.queue import outcomes, polling
from merging.verdict import decide, runlist, runners

from tests.queue.fixtures import READY
from tests.queue.test_runner_outage import waited
from tests.verdict.fixtures import CODE, built, job, run

# The reviewer's shape: `changes` cancelled, the gate failed, `build-test`
# still listed `in_progress` with no conclusion, the rest skipped.
LAGGING_OUTAGE = [
    job("changes", "cancelled"),
    {"name": "build-test", "status": "in_progress", "conclusion": None, "steps": []},
    job("build-test-cross (macos-latest)", "skipped"),
    job("deny", "skipped"),
    job("ci-gate", "failure"),
]
FAILED = run("ci", conclusion="failure")


def polled(ci: dict, ci_jobs: list[dict], waited_s: float, since: float = 0.0):
    """One poll of `ci` with these jobs, `lint` green, the red first read at `since`."""
    jobs = {1: ci_jobs, 2: built("lint")}
    return polling.progress(READY, [ci, run("lint")], jobs, CODE, waited_s, red_since={(1, 1): since})


class RunnerLagGrace(unittest.TestCase):
    def test_a_lagging_outage_waits_within_the_grace(self):
        state, lines = polled(FAILED, LAGGING_OUTAGE, polling.GATE_SHOWS_SECONDS - 1)
        self.assertEqual(state, outcomes.WAIT, lines)
        self.assertIn("`build-test` going", " ".join(lines))

    def test_a_lagging_outage_is_re_run_past_the_grace(self):
        state, lines = polled(FAILED, LAGGING_OUTAGE, polling.GATE_SHOWS_SECONDS)
        self.assertEqual(state, outcomes.RERUN, lines)

    def test_the_wait_re_runs_a_lagging_outage_once(self):
        listing = ([FAILED, run("lint")], {1: LAGGING_OUTAGE, 2: built("lint")})
        state, lines, calls = waited([listing])
        self.assertEqual(calls, [["gh", "run", "rerun", "--failed", "1"]])
        self.assertEqual(state, outcomes.STOP, lines)

    def test_a_cancelled_run_with_a_job_still_listed_goes_through_agrees(self):
        cancelled = run("ci", conclusion="cancelled")
        jobs = {1: LAGGING_OUTAGE}
        self.assertIsNone(runners.runner_red(cancelled, jobs, "ci"))
        self.assertTrue(runners.runner_red(cancelled, jobs, "ci", lambda _run: True))
        state, lines = polled(cancelled, LAGGING_OUTAGE, 60.0)
        self.assertEqual(state, outcomes.WAIT, lines)

    def test_a_code_red_still_listed_going_is_the_codes_past_the_grace(self):
        code = [*LAGGING_OUTAGE[1:4], job("changes", "success"), job("deny", "failure"), job("ci-gate", "failure")]
        state, lines = polled(FAILED, code, polling.GATE_SHOWS_SECONDS)
        self.assertEqual(state, outcomes.STOP, lines)
        self.assertIn("concluded failure", lines[0])
        self.assertIn("Judged as listed, though its jobs still show `build-test` going", " ".join(lines))


class ListedWording(unittest.TestCase):
    def test_a_run_github_lists_completed_is_worded_so(self):
        jobs = {1: [*built("ci"), {"name": "deny", "status": "queued", "conclusion": None}]}
        state, lines = decide.assess(READY, "ci", [run("ci")], jobs)
        self.assertEqual(state, decide.RUNNING, lines)
        self.assertIn("is listed `completed` `success`", lines[0])

    def test_a_run_only_its_gate_settled_is_worded_as_github_lists_it(self):
        going = run("ci", status="in_progress", conclusion=None)
        jobs = {1: [*built("ci"), {"name": "build-test", "status": "in_progress", "conclusion": None}]}
        state, lines = decide.assess(READY, "ci", [going], jobs)
        self.assertEqual(state, decide.RUNNING, lines)
        self.assertIn("is listed `in_progress`, its gate concluded `success`", lines[0])
        self.assertNotIn("listed `completed`", lines[0])

    def test_a_run_settled_by_its_gate_keeps_githubs_status(self):
        settled = runlist.by_gate(run("ci", status="queued", conclusion=None), {1: built("ci")}, "ci")
        self.assertEqual(runlist.listed(settled), "is listed `queued`, its gate concluded `success`")


if __name__ == "__main__":
    unittest.main()
