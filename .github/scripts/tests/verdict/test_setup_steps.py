"""#1932: a job that failed while installing a tool is the runners' red, not the code's.

Driven over the two runs the queue lost a round to on 2026-11-08
([`setup_runs`]); the queue's and the train's re-run is
`tests/queue/test_setup_rerun.py`, and the list against the workflows
`tests/verdict/test_setup_order.py`.
"""

from __future__ import annotations

import unittest

from merging.verdict import runners, setup
from tests.verdict import setup_runs as real
from tests.verdict.fixtures import built, run


def ci_red(*failed: dict) -> dict[int, list[dict]]:
    """A `ci` run's jobs: genuine builds, with `failed` in place of theirs and the gate red."""
    names = {f["name"] for f in failed}
    rest = [j for j in built("ci") if j["name"] not in names and j["name"] != "ci-gate"]
    return {1: [*rest, *failed, real.GATE_RED]}


class FailedInSetup(unittest.TestCase):
    def test_both_jobs_of_2026_11_08_failed_in_setup(self):
        self.assertTrue(setup.failed_in_setup(real.MACOS_NEXTEST))
        self.assertTrue(setup.failed_in_setup(real.DENY_IMAGE))

    def test_a_failed_build_lint_or_test_step_is_the_code(self):
        for step in ("Test (engine, default)", "Test (lint xtask)", "Run EmbarkStudios/cargo-deny-action@v2"):
            for job in (real.MACOS_NEXTEST, real.DENY_IMAGE):
                if any(s["name"] == step for s in job["steps"]):
                    with self.subTest(step=step):
                        self.assertFalse(setup.failed_in_setup(real.failed_at(job, step)))
        clippy = {"name": "lint", "conclusion": "failure", "steps": real.steps(
            ("Set up job", "success"), ("Clippy (hard gate, default features)", "failure"))}
        self.assertFalse(setup.failed_in_setup(clippy))

    def test_a_setup_step_failing_after_a_build_step_ran_is_the_code(self):
        # A setup name after a step that built something: the build may be what broke it.
        job = {"name": "lint", "conclusion": "failure", "steps": real.steps(
            ("Set up job", "success"), ("Format check (whole crate; rustfmt follows mod tree)", "success"),
            ("System libs for winit/wgpu builds", "failure"))}
        self.assertFalse(setup.failed_in_setup(job))

    def test_a_job_with_no_steps_listed_is_not_judged_setup(self):
        self.assertFalse(setup.failed_in_setup({"name": "deny", "conclusion": "failure", "steps": []}))
        self.assertFalse(setup.failed_in_setup({"name": "deny", "conclusion": "failure"}))


class RunnerRed(unittest.TestCase):
    def test_a_job_failed_in_install_cargo_nextest_is_runner_red(self):
        self.assertIs(runners.runner_red(run("ci", conclusion="failure"), ci_red(real.MACOS_NEXTEST), "ci"), True)

    def test_a_gate_failed_only_over_setup_failures_is_runner_red(self):
        jobs = ci_red(real.MACOS_NEXTEST, real.DENY_IMAGE)
        self.assertIs(runners.runner_red(run("ci", conclusion="failure"), jobs, "ci"), True)

    def test_a_setup_failure_beside_a_failed_test_is_the_code(self):
        tested = real.failed_at({**real.MACOS_NEXTEST, "name": "build-test"}, "Test (engine, default)")
        jobs = ci_red(real.DENY_IMAGE, tested)
        self.assertIs(runners.runner_red(run("ci", conclusion="failure"), jobs, "ci"), False)


class TheList(unittest.TestCase):
    def test_it_is_sorted_and_has_no_duplicates(self):
        self.assertEqual(list(setup.SETUP), sorted(set(setup.SETUP)))


if __name__ == "__main__":
    unittest.main()
