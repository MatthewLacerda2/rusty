#!/usr/bin/env python3
"""A torn-down mutation job still says so on its pull request (#2230).

The job that needs it reads the run's jobs and annotations; these hold what
it makes of them, and the workflow's guards around it: it never runs on a
green run, a cancelled one, or one whose mutation job already posted.
"""

from __future__ import annotations

import io
import re
import unittest
from contextlib import redirect_stdout

from tests import SCRIPTS, load

torn = load("mutants-torn-down.py")
WORKFLOW = (SCRIPTS.parent / "workflows" / "mutants-on-request.yml").read_text()

LOST = {"annotation_level": "failure", "message": "The hosted runner lost communication with the server."}
CANCELED = {"annotation_level": "failure", "message": "The operation was canceled."}
NOTICE = {"annotation_level": "notice", "message": "The ubuntu-latest label will migrate."}


def job(*steps: tuple[str, str | None]) -> dict:
    return {"name": "mutants", "id": 1, "steps": [{"name": n, "conclusion": c} for n, c in steps]}


class Reason(unittest.TestCase):
    def test_names_the_step_and_what_github_said(self) -> None:
        text = torn.reason(job(("Plan", "success"), ("Standalone workspaces", None), ("Mutate", None)),
                           [LOST, CANCELED, NOTICE])
        self.assertIn("`Standalone workspaces` step", text)
        self.assertIn("lost communication", text)
        self.assertNotIn("canceled", text)
        self.assertNotIn("migrate", text)

    def test_a_cancelled_step_is_where_it_ended(self) -> None:
        self.assertEqual(torn.last_step(job(("Plan", "success"), ("Skipped", "skipped"),
                                            ("Mutate", "cancelled"), ("Render", "skipped"))), "Mutate")

    def test_an_unfinished_step_is_preferred_over_an_earlier_failure(self) -> None:
        self.assertEqual(torn.last_step(job(("Lint", "failure"), ("Mutate", None), ("Render", None))), "Mutate")

    def test_a_job_that_failed_with_its_steps_run_is_not_called_torn_down(self) -> None:
        text = torn.reason(job(("Checkout", "failure"), ("Mutate", "skipped")), [])
        self.assertTrue(text.startswith("the mutation job failed during its `Checkout` step with its steps run"))
        self.assertNotIn("torn down", text)

    def test_the_runner_going_away_reads_as_torn_down_even_with_steps_finished(self) -> None:
        self.assertIn("torn down", torn.reason(job(("Plan", "success")), [LOST]))
        self.assertIn("torn down", torn.reason(job(("Plan", "success")), [
            {"annotation_level": "failure", "message": "The runner has received a shutdown signal."}]))

    def test_no_step_and_no_cause_still_reads_as_unanswered(self) -> None:
        text = torn.reason(job(("Plan", "success")), [])
        self.assertTrue(text.startswith("the mutation job failed with its steps run"))


def report_job() -> str:
    return WORKFLOW.split("\n  report:\n", 1)[1]


class Guards(unittest.TestCase):
    """The `report` job, which `needs:` every other, answers for a job that
    left nothing (#2283 folded the `no-answer` job into it)."""

    def test_runs_after_any_red_or_torn_down_job_but_never_a_cancelled_run(self) -> None:
        job = report_job()
        [cond] = re.findall(r"\n    if: (.+)\n", job)
        # `!cancelled()`: a superseded run or a closed pull request is read by nobody;
        # a job killed at `timeout-minutes` reads `cancelled` but leaves the run uncancelled.
        self.assertEqual(cond.split(" && "), ["always()", "!cancelled()"])
        self.assertIn("needs: [plan, mutants]", job)

    def test_names_a_lost_shard_and_a_lost_plan_from_outside(self) -> None:
        render = report_job().split("- name: Render the report", 1)[1].split("\n      - name:", 1)[0]
        self.assertIn("mutants-torn-down.py", render)
        self.assertIn('why "mutants ($k)"', render)
        self.assertIn('why plan', render)

    def test_posts_an_unanswered_run_below_an_earlier_report_never_over_it(self) -> None:
        job = report_job()
        self.assertIn('echo "append=--append" >> "$GITHUB_OUTPUT"', job)
        self.assertIn("APPEND: ${{ steps.render.outputs.append }}", job)

    def test_only_the_report_job_may_comment(self) -> None:
        jobs = WORKFLOW.split("\njobs:\n", 1)[1]
        self.assertEqual(jobs.count("pull-requests: write"), 1)
        self.assertIn("pull-requests: write", report_job())


class Paging(unittest.TestCase):
    def test_reads_every_page_of_annotations(self) -> None:
        asked: list[str] = []

        def fake(*args: str) -> object:
            asked.append(args[0])
            if "/jobs" in args[0]:
                return {"jobs": [job(("Mutate", None))]}
            page = int(args[0].rsplit("page=", 1)[1])
            return [CANCELED] * 100 if page == 1 else [LOST]

        torn.gh, real = fake, torn.gh
        try:
            with redirect_stdout(io.StringIO()) as out:
                torn.main(["--repo", "o/r", "--run", "1", "--job", "mutants"])
        finally:
            torn.gh = real
        self.assertIn("lost communication", out.getvalue())
        self.assertEqual(sum("annotations" in a for a in asked), 2)


if __name__ == "__main__":
    unittest.main()
