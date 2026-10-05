#!/usr/bin/env python3
"""#869: a red that only GitHub's runners caused is re-run once, not handed back.

On 2026-10-05 hosted runners stopped being acquired and every run in flight came
back red with nothing compiled, in two shapes (run 37365886365: `lint-gate`
cancelled beside a green `lint`; run 37366446562: `changes` cancelled, so
`ci-gate` failed over skipped jobs). These drive [`queue.wait_for`] over those
shapes with `gh` and the clock replaced; the watch's side, a re-run of a
handed-back head re-queueing it, is `test_queue_watch_rerun.py`.
"""

from __future__ import annotations

import importlib.util
import subprocess
import unittest
from pathlib import Path
from unittest import mock

TESTS = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("flicker_outage", TESTS / "test_queue_listing_flicker.py")
flicker = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(flicker)
queue, fixtures = flicker.queue, flicker.fixtures
mergeable, run, job, built = queue.mergeable, fixtures.run, fixtures.job, fixtures.built

GATE_CANCELLED = [job("changes", "success"), job("lint", "success"), job("lint-gate", "cancelled")]
CHANGES_CANCELLED = [job("changes", "cancelled"), job("build-test", "skipped"), job("deny", "skipped"), job("ci-gate", "failure")]
CODE_RED = [job("changes", "success"), job("build-test", "failure"), job("deny", "skipped"), job("ci-gate", "failure")]


def lint(attempt=1, jobs=GATE_CANCELLED, **fields):
    """`ci` green, `lint` on `attempt` with these jobs."""
    return [run("ci"), run("lint", run_attempt=attempt, **fields)], {1: built("ci"), 2: jobs}


def ci_cancelled(attempt=1, **fields):
    return [run("ci", conclusion="failure", run_attempt=attempt, **fields), run("lint")], {1: CHANGES_CANCELLED, 2: built("lint")}


RERUNNING = lint(2, [job("changes", "success"), job("lint", None)], status="in_progress", conclusion=None)
PASSED = lint(2, built("lint"))


def waited(listings, rerun_status=0):
    calls = []

    def fake_run(argv, **_):
        calls.append(argv)
        return subprocess.CompletedProcess(argv, rerun_status, "", "HTTP 403: nope" if rerun_status else "")

    with mock.patch.object(queue.subprocess, "run", side_effect=fake_run):
        state, lines = flicker.waited(listings)
    return state, lines, calls


class RunnerRed(unittest.TestCase):
    def test_both_shapes_seen_on_2026_10_05_are_the_runners(self):
        self.assertTrue(mergeable.runner_red(run("lint", conclusion="cancelled"), {2: GATE_CANCELLED}, "lint"))
        self.assertTrue(mergeable.runner_red(run("ci", conclusion="failure"), {1: CHANGES_CANCELLED}, "ci"))

    def test_a_failed_job_besides_the_gate_is_the_code(self):
        self.assertFalse(mergeable.runner_red(run("ci", conclusion="failure"), {1: CODE_RED}, "ci"))
        mixed = [*CHANGES_CANCELLED[:-1], job("build-test-cross (macos-latest)", "failure"), job("ci-gate", "failure")]
        self.assertFalse(mergeable.runner_red(run("ci", conclusion="failure"), {1: mixed}, "ci"))

    def test_a_run_with_no_jobs_is_the_runners_only_when_cancelled(self):
        self.assertTrue(mergeable.runner_red(run("ci", conclusion="cancelled"), {}, "ci"))
        self.assertFalse(mergeable.runner_red(run("ci", conclusion="failure"), {}, "ci"))

    def test_one_code_red_beside_a_runner_red_is_no_outage(self):
        runs = [run("ci", conclusion="failure"), run("lint", conclusion="cancelled")]
        self.assertEqual(mergeable.outage(fixtures.READY, runs, {1: CODE_RED, 2: GATE_CANCELLED}), [])
        runs, jobs = lint(conclusion="cancelled")
        self.assertEqual([r["name"] for r in mergeable.outage(fixtures.READY, runs, jobs)], ["lint"])


class Rerun(unittest.TestCase):
    def test_an_outage_is_re_run_once_and_then_merges(self):
        listings = [lint(conclusion="cancelled"), lint(conclusion="cancelled"), RERUNNING, PASSED]
        state, lines, calls = waited(listings)
        self.assertEqual(state, queue.GO, lines)
        self.assertEqual(calls, [["gh", "run", "rerun", "--failed", "2"]])

    def test_a_gate_failed_over_a_cancelled_changes_job_is_re_run_before_handing_back(self):
        state, lines, calls = waited([ci_cancelled(), ci_cancelled(2)])
        self.assertEqual((state, calls), (queue.STOP, [["gh", "run", "rerun", "--failed", "1"]]))
        self.assertTrue(lines[0].startswith(queue.RUNNERS), lines)

    def test_cancelled_again_after_the_re_run_hands_back_as_runners_unavailable(self):
        state, lines, calls = waited([lint(conclusion="cancelled"), lint(2, conclusion="cancelled")])
        self.assertEqual(state, queue.STOP)
        self.assertEqual(len(calls), 1)
        self.assertTrue(lines[0].startswith(queue.RUNNERS), lines)
        self.assertIn("no push needed", " ".join(lines))

    def test_a_refused_re_run_hands_back_saying_so(self):
        state, lines, _ = waited([lint(conclusion="cancelled")], rerun_status=1)
        self.assertEqual(state, queue.STOP)
        self.assertIn("refused: HTTP 403", lines[0])

    def test_a_code_red_is_handed_back_at_once_and_never_re_run(self):
        state, _, calls = waited([([run("ci", conclusion="failure"), run("lint")], {1: CODE_RED, 2: built("lint")})])
        self.assertEqual((state, calls), (queue.STOP, []))


if __name__ == "__main__":
    unittest.main()
