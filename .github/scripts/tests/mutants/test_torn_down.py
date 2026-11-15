#!/usr/bin/env python3
"""A torn-down mutation job still says so on its pull request (#2230).

The job that needs it reads the run's jobs and annotations; these hold what
it makes of them, and the workflow's guards around it: it never runs on a
green run, a cancelled one, or one whose mutation job already posted.
"""

from __future__ import annotations

import re
import unittest

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

    def test_no_step_and_no_cause_still_reads_as_unanswered(self) -> None:
        text = torn.reason(job(("Plan", "success")), [])
        self.assertTrue(text.startswith("the mutation job was torn down, so no step"))


def no_answer_job() -> str:
    return WORKFLOW.split("\n  no-answer:\n", 1)[1]


class Guards(unittest.TestCase):
    def test_runs_only_for_a_red_or_timed_out_run_nobody_answered(self) -> None:
        [cond] = re.findall(r"\n    if: (.+)\n", no_answer_job())
        parts = cond.split(" && ")
        # `!cancelled()`: a superseded run or a closed pull request is read by nobody;
        # a job killed at `timeout-minutes` reads `cancelled` but leaves the run uncancelled.
        for want in ("always()", "!cancelled()", "needs.mutants.result != 'success'",
                     "needs.mutants.result != 'skipped'", "needs.mutants.outputs.answered != 'true'"):
            self.assertIn(want, parts)

    def test_the_mutation_job_marks_its_answer_where_the_guard_reads_it(self) -> None:
        mutants = WORKFLOW.split("\n  mutants:\n", 1)[1].split("\n  no-answer:\n", 1)[0]
        self.assertIn("answered: ${{ steps.post.outputs.answered }}", mutants)
        post = mutants.split("- name: Post the report on the pull request", 1)[1].split("\n      - name:", 1)[0]
        self.assertIn("id: post", post)
        self.assertIn('echo answered=true >> "$GITHUB_OUTPUT"', post)

    def test_posts_below_an_earlier_report_never_over_it(self) -> None:
        self.assertIn("mutants-pr-comment.py report/report.md --append", no_answer_job())


if __name__ == "__main__":
    unittest.main()
