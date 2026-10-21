#!/usr/bin/env python3
"""#869: a red that only GitHub's runners caused is re-run once, not handed back.

On 2026-10-05 hosted runners stopped being acquired and every run in flight came
back red with nothing compiled, in two shapes (run 37365886365: `lint-gate`
cancelled beside a green `lint`; run 37366446562: `changes` cancelled, so
`ci-gate` failed over skipped jobs). These drive [`waiting.wait_for`] over those
shapes with `gh` and the clock replaced; the watch's side, a re-run of a
handed-back head re-queueing it, is `tests/watch/test_rerun.py`.
"""

from __future__ import annotations

import subprocess
import unittest
from unittest import mock

from merging.queue import outcomes
from merging.verdict import runners

from tests.queue import test_listing_flicker as flicker
from tests.verdict import fixtures
from tests.verdict.fixtures import built, job, run

GATE_CANCELLED = [job("changes", "success"), job("lint", "success"), job("lint-gate", "cancelled")]
CHANGES_CANCELLED = [job("changes", "cancelled"), job("build-test", "skipped"), job("deny", "skipped"), job("ci-gate", "failure")]
CODE_RED = [job("changes", "success"), job("build-test", "failure"), job("deny", "skipped"), job("ci-gate", "failure")]


def lint(attempt=1, jobs=GATE_CANCELLED, **fields):
    """`ci` green, `lint` on `attempt` with these jobs, listed for that attempt."""
    return [run("ci"), run("lint", run_attempt=attempt, **fields)], {1: built("ci"), 2: [{**j, "run_attempt": attempt} for j in jobs]}


def ci_cancelled(attempt=1, **fields):
    """`ci` failed over a cancelled `changes` on `attempt`, its jobs listed for that attempt."""
    ci_jobs = [{**j, "run_attempt": attempt} for j in CHANGES_CANCELLED]
    return [run("ci", conclusion="failure", run_attempt=attempt, **fields), run("lint")], {1: ci_jobs, 2: built("lint")}


RERUNNING = lint(2, [job("changes", "success"), job("lint", None)], status="in_progress", conclusion=None)
PASSED = lint(2, built("lint"))


def waited(listings, rerun_status=0):
    calls = []

    def fake_run(argv, **_):
        calls.append(argv)
        return subprocess.CompletedProcess(argv, rerun_status, "", "HTTP 403: nope" if rerun_status else "")

    with mock.patch.object(subprocess, "run", side_effect=fake_run):
        state, lines = flicker.waited(listings)
    return state, lines, calls


class RunnerRed(unittest.TestCase):
    def test_both_shapes_seen_on_2026_10_05_are_the_runners(self):
        self.assertTrue(runners.runner_red(run("lint", conclusion="cancelled"), {2: GATE_CANCELLED}, "lint"))
        self.assertTrue(runners.runner_red(run("ci", conclusion="failure"), {1: CHANGES_CANCELLED}, "ci"))

    def test_a_failed_job_besides_the_gate_is_the_code(self):
        self.assertFalse(runners.runner_red(run("ci", conclusion="failure"), {1: CODE_RED}, "ci"))
        mixed = [*CHANGES_CANCELLED[:-1], job("build-test-cross (macos-latest)", "failure"), job("ci-gate", "failure")]
        self.assertFalse(runners.runner_red(run("ci", conclusion="failure"), {1: mixed}, "ci"))

    def test_a_run_with_no_jobs_is_the_runners_when_cancelled_and_unknown_when_failed(self):
        # No jobs listed has not said whose red it is (#1109, #1182); past the
        # grace a cancelled run is the runners' and a failure stays unknown.
        spent = lambda _run: True
        self.assertIsNone(runners.runner_red(run("ci", conclusion="cancelled"), {}, "ci"))
        self.assertTrue(runners.runner_red(run("ci", conclusion="cancelled"), {}, "ci", spent))
        self.assertIsNone(runners.runner_red(run("ci", conclusion="failure"), {}, "ci", spent))

    def test_one_code_red_beside_a_runner_red_is_no_outage(self):
        runs = [run("ci", conclusion="failure"), run("lint", conclusion="cancelled")]
        self.assertEqual(runners.outage(fixtures.READY, runs, {1: CODE_RED, 2: GATE_CANCELLED}), [])
        runs, jobs = lint(conclusion="cancelled")
        self.assertEqual([r["name"] for r in runners.outage(fixtures.READY, runs, jobs)], ["lint"])


class Rerun(unittest.TestCase):
    def test_an_outage_is_re_run_once_and_then_merges(self):
        listings = [lint(conclusion="cancelled"), lint(conclusion="cancelled"), RERUNNING, PASSED]
        state, lines, calls = waited(listings)
        self.assertEqual(state, outcomes.GO, lines)
        self.assertEqual(calls, [["gh", "run", "rerun", "--failed", "2"]])

    def test_a_gate_failed_over_a_cancelled_changes_job_is_re_run_before_handing_back(self):
        state, lines, calls = waited([ci_cancelled(), ci_cancelled(2)])
        self.assertEqual((state, calls), (outcomes.STOP, [["gh", "run", "rerun", "--failed", "1"]]))
        self.assertTrue(lines[0].startswith(outcomes.RUNNERS), lines)

    def test_cancelled_again_after_the_re_run_hands_back_as_runners_unavailable(self):
        state, lines, calls = waited([lint(conclusion="cancelled"), lint(2, conclusion="cancelled")])
        self.assertEqual(state, outcomes.STOP)
        self.assertEqual(len(calls), 1)
        self.assertTrue(lines[0].startswith(outcomes.RUNNERS), lines)
        self.assertIn("no push needed", " ".join(lines))

    def test_a_refused_re_run_hands_back_saying_so(self):
        state, lines, _ = waited([lint(conclusion="cancelled")], rerun_status=1)
        self.assertEqual(state, outcomes.STOP)
        self.assertIn("refused: HTTP 403", lines[0])

    def test_a_code_red_is_handed_back_at_once_and_never_re_run(self):
        state, _, calls = waited([([run("ci", conclusion="failure"), run("lint")], {1: CODE_RED, 2: built("lint")})])
        self.assertEqual((state, calls), (outcomes.STOP, []))


if __name__ == "__main__":
    unittest.main()
