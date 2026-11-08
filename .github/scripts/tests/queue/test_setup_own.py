"""#1932, PR #1933's review: what a setup red says, and a pull request that
edits its own setup.

The re-run and the hand-back name the failed step, not only "cancelled". A pull
request (or a train member) changing `rust-toolchain.toml` or
`.github/workflows/` is judged on its setup failures: re-run as the runners'
they would be handed back as `runners unavailable`, never as the code's. The
train's side is `test_setup_rerun.py`'s `Train`, and the members' files
reaching the wait is `tests/train/`'s.
"""

from __future__ import annotations

import unittest

from merging.queue import outcomes, polling
from merging.verdict import runners, setup

from tests.queue import test_listing_flicker as flicker
from tests.queue.test_setup_rerun import reruns, setup_red
from tests.verdict import setup_runs as real
from tests.verdict.fixtures import run
from tests.verdict.test_setup_steps import ci_red

NAMED = "`build-test-cross (macos-26)` failed in `Install cargo-nextest`"


class Wording(unittest.TestCase):
    def test_the_re_run_names_the_failed_step(self):
        runs, jobs = setup_red()
        state, lines = polling.rerun_step([runs[0]], {}, "abc1234", jobs)
        self.assertEqual(state, outcomes.RERUN)
        self.assertIn("failed in a setup step", lines[0])
        self.assertIn(NAMED, lines[0])
        self.assertNotIn("ci-gate", lines[0])

    def test_the_hand_back_names_the_failed_step(self):
        runs, jobs = setup_red(2)
        state, lines = polling.rerun_step([{**runs[0], "run_attempt": 2}], {1: 1}, "abc1234", jobs)
        self.assertEqual(state, outcomes.STOP)
        self.assertTrue(lines[0].startswith(outcomes.RUNNERS), lines)
        self.assertIn("failed in a setup step", lines[0])
        self.assertIn(NAMED, lines[0])

    def test_a_cancelled_job_is_said_cancelled(self):
        cancelled = {"name": "deny", "conclusion": "cancelled", "steps": []}
        self.assertEqual(runners.why(run("ci", conclusion="failure"), ci_red(cancelled)), "`deny` cancelled")


class Edited(unittest.TestCase):
    def test_the_toolchain_file_and_workflows_are_the_pull_requests_own_setup(self):
        self.assertTrue(setup.edited(["src/x.rs", "rust-toolchain.toml"]))
        self.assertTrue(setup.edited([".github/workflows/ci.yml"]))
        self.assertFalse(setup.edited(["src/x.rs", ".github/scripts/x.py", "docs/rust-toolchain.toml"]))
        self.assertFalse(setup.edited([]))

    def test_runner_red_reads_a_setup_failure_as_the_code_when_it_is_its_own(self):
        jobs = ci_red(real.MACOS_NEXTEST)
        self.assertIs(runners.runner_red(run("ci", conclusion="failure"), jobs, "ci", own_setup=True), False)

    def test_a_pull_request_editing_its_workflows_is_handed_back_at_once(self):
        pull = {**flicker.PULL, "files": [{"path": ".github/workflows/ci.yml"}]}
        calls, patch = reruns()
        with patch:
            state, _ = flicker.waited([setup_red()], pull)
        self.assertEqual((state, calls), (outcomes.STOP, []))


if __name__ == "__main__":
    unittest.main()
