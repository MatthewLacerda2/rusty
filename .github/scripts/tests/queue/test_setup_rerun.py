"""#1932: a red only setup steps caused is re-run once, for a pull request and for a train.

The pull request's side drives [`waiting.wait_for`] the way #869's
`test_runner_outage.py` does; the train's drives [`ride.wait`]. Each reads
run 42466064410's shape: `build-test-cross (macos-26)` failed in
`Install cargo-nextest`, so `ci-gate` failed over it.
"""

from __future__ import annotations

import subprocess
import time
import unittest
from unittest import mock

from merging.queue import outcomes
from merging.train import ride
from merging.verdict import github

from tests.queue import test_listing_flicker as flicker
from tests.verdict import setup_runs as real
from tests.verdict.fixtures import SHA, built, run
from tests.verdict.test_setup_steps import ci_red


def setup_red(attempt=1):
    jobs = [{**j, "run_attempt": attempt} for j in ci_red(real.MACOS_NEXTEST)[1]]
    return [run("ci", conclusion="failure", run_attempt=attempt), run("lint")], {1: jobs, 2: built("lint")}


def tested_red():
    tested = real.failed_at({**real.MACOS_NEXTEST, "name": "build-test-cross (macos-26)"}, "Test (engine, default)")
    return [run("ci", conclusion="failure"), run("lint")], {1: ci_red(tested)[1], 2: built("lint")}


GREEN = ([run("ci", run_attempt=2), run("lint")], {1: [{**j, "run_attempt": 2} for j in built("ci")], 2: built("lint")})


def reruns():
    calls = []

    def fake(argv, **_):
        calls.append(argv)
        return subprocess.CompletedProcess(argv, 0, "", "")

    return calls, mock.patch.object(subprocess, "run", side_effect=fake)


class PullRequest(unittest.TestCase):
    def test_a_setup_red_is_re_run_once_and_then_merges(self):
        calls, patch = reruns()
        with patch:
            state, lines = flicker.waited([setup_red(), setup_red(), GREEN])
        self.assertEqual(state, outcomes.GO, lines)
        self.assertEqual(calls, [["gh", "run", "rerun", "--failed", "1"]])

    def test_a_second_setup_red_hands_back_as_runners_unavailable(self):
        calls, patch = reruns()
        with patch:
            state, lines = flicker.waited([setup_red(), setup_red(2)])
        self.assertEqual((state, len(calls)), (outcomes.STOP, 1))
        self.assertTrue(lines[0].startswith(outcomes.RUNNERS), lines)

    def test_a_failed_test_step_is_handed_back_at_once(self):
        calls, patch = reruns()
        with patch:
            state, _ = flicker.waited([tested_red()])
        self.assertEqual((state, calls), (outcomes.STOP, []))


class Train(unittest.TestCase):
    def ridden(self, listings):
        calls, patch = reruns()
        with patch, mock.patch.object(github, "evidence", side_effect=listings), \
                mock.patch.object(github, "rollup", return_value=[]), mock.patch.object(time, "sleep"), \
                mock.patch.object(time, "monotonic", side_effect=[0.0] * 2 * len(listings) + [9999.0] * 4):
            return ride.wait("o/r", SHA, 3600.0, 30, say=lambda *a: None)[0], calls

    def test_a_train_red_only_in_setup_is_re_run_not_split(self):
        state, calls = self.ridden([setup_red(), GREEN])
        self.assertEqual((state, calls), (outcomes.GO, [["gh", "run", "rerun", "--failed", "1"]]))

    def test_a_train_red_in_a_test_step_is_red(self):
        state, calls = self.ridden([tested_red()])
        self.assertEqual((state, calls), (ride.RED, []))


if __name__ == "__main__":
    unittest.main()
