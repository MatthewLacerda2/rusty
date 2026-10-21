#!/usr/bin/env python3
"""A completed run with any job still listed going is lagging, not a verdict (#1182).

PR #1174, head `ca0ade3`, 2026-10-21: run 39623311398 was listed `completed
success` and its `lint-gate` job `success`, while the jobs listing still showed
the `lint` job `in_progress` with conclusion `null`, every step of it finished.
The queue handed the green pull request back as "no `lint` run ran a gated job".
"""

from __future__ import annotations

import unittest

from merging.queue import outcomes
from merging.verdict import decide, runlist, runners
from tests.queue.fixtures import SPENT, asked
from tests.verdict.fixtures import READY, built, drafted, run

STEPS = ["Set up job", "Checkout", "Lint", "Complete job"]


def listed(name: str, status: str, conclusion: str | None, job_id: int) -> dict:
    """A job as GitHub's jobs listing returns it, every step `completed success`."""
    return {
        "id": job_id,
        "run_id": 39623311398,
        "run_attempt": 1,
        "name": name,
        "status": status,
        "conclusion": conclusion,
        "steps": [
            {"name": s, "status": "completed", "conclusion": "success", "number": n}
            for n, s in enumerate(STEPS, 1)
        ],
    }


def lint_1174(lint_status: str, lint_conclusion: str | None) -> list[dict]:
    """#1174's `lint` jobs: `changes` and the gate `success`, `lint` as given."""
    return [
        listed("changes", "completed", "success", 1),
        listed("lint", lint_status, lint_conclusion, 2),
        listed("lint-gate", "completed", "success", 3),
    ]


def assessed(lint_run: dict, lint_jobs: list[dict]) -> tuple[str, list[str]]:
    return decide.assess(READY, "lint", [lint_run], {lint_run["id"]: lint_jobs})


class ListingAgrees(unittest.TestCase):
    def test_1174s_gated_job_still_listed_going_is_running(self):
        state, lines = assessed(run("lint", run_attempt=1), lint_1174("in_progress", None))
        self.assertEqual(state, decide.RUNNING, lines)
        self.assertIn("`lint` going after `lint-gate` concluded", " ".join(lines))

    def test_1174s_run_passes_once_the_job_reads_success(self):
        state, lines = assessed(run("lint", run_attempt=1), lint_1174("completed", "success"))
        self.assertEqual(state, decide.PASSED, lines)

    def test_a_drafts_run_is_still_unbuilt(self):
        state, lines = assessed(run("lint"), drafted("lint"))
        self.assertEqual(state, decide.UNBUILT, lines)

    def test_any_job_counts_on_a_run_github_listed_completed(self):
        jobs = [*built("ci"), {"name": "docs-signal", "status": "queued", "conclusion": None}]
        self.assertEqual(decide.assess(READY, "ci", [run("ci")], {1: jobs})[0], decide.RUNNING)

    def test_a_run_settled_by_its_gate_still_lets_its_signals_run(self):
        going = run("ci", status="in_progress", conclusion=None)
        jobs = [*built("ci"), {"name": "mutants-pr", "status": "in_progress", "conclusion": None}]
        self.assertEqual(decide.assess(READY, "ci", [going], {1: jobs})[0], decide.PASSED)

    def test_a_run_settled_by_its_gate_waits_on_a_gated_job_still_listed(self):
        going = run("lint", status="in_progress", conclusion=None, run_attempt=1)
        self.assertEqual(assessed(going, lint_1174("in_progress", None))[0], decide.RUNNING)

    def test_whose_red_waits_for_every_job_to_show(self):
        red = run("lint", conclusion="failure", run_attempt=1)
        jobs = [listed("changes", "completed", "cancelled", 1), listed("lint", "in_progress", None, 2),
                listed("lint-gate", "completed", "failure", 3)]
        self.assertIsNone(runners.runner_red(red, {red["id"]: jobs}, "lint"))
        jobs[1] = listed("lint", "completed", "failure", 2)
        self.assertIs(runners.runner_red(red, {red["id"]: jobs}, "lint"), False)

    def test_an_older_attempts_jobs_are_not_the_runs(self):
        rerun = run("lint", run_attempt=2)
        jobs = [{**j, "run_attempt": 2} for j in lint_1174("completed", "success")]
        jobs.append(listed("lint", "in_progress", None, 9))  # attempt 1's, stale
        self.assertTrue(runlist.agrees(rerun, {rerun["id"]: jobs}, "lint"))

    def test_the_queue_waits_on_1174_rather_than_handing_back(self):
        jobs = {1: built("ci"), 2: lint_1174("in_progress", None)}
        state, lines = asked(run("ci"), run("lint", run_attempt=1), jobs=jobs, waited=SPENT)
        self.assertEqual(state, outcomes.WAIT, lines)


if __name__ == "__main__":
    unittest.main()
