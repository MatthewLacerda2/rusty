"""The workflows wire `proved_tree.py` the way its skip relies on (#1784).

Text checks over the workflow files: the job and step names `BUILT` reads
exist, each `changes` job asks and folds the answer into `code`, a broken
lookup cannot fail `changes`, and every workflow that skips on a push has a
nightly that runs it in full (the only other run that re-saves `main`'s cache).
"""

from __future__ import annotations

import re
import unittest

import proved_tree
from tests import SCRIPTS


def body(workflow: str) -> str:
    return (SCRIPTS.parent / "workflows" / workflow).read_text(encoding="utf-8")


def step(text: str, name: str) -> str:
    """The step called `name`, up to the next step or job."""
    start = text.index(f"- name: {name}\n")
    end = re.search(r"\n\s*- (uses|name):|\n  \S", text[start + 1:])
    return text[start:start + 1 + end.start()] if end else text[start:]


class Wiring(unittest.TestCase):
    def test_built_names_match_the_workflows(self):
        for workflow, jobs in proved_tree.BUILT.items():
            text = body(workflow)
            for base, name in jobs.items():
                with self.subTest(workflow=workflow, job=base):
                    self.assertIn(f"\n  {base}:\n", text)
                    if name:
                        self.assertIn(f"- name: {name}\n", text)

    def test_each_changes_job_asks_and_folds_the_answer_into_code(self):
        for workflow in proved_tree.BUILT:
            with self.subTest(workflow=workflow):
                text = body(workflow)
                self.assertIn(f"proved_tree.py --workflow {workflow}", text)
                self.assertIn("code: ${{ steps.proved.outputs.proved == 'true' && 'false' ||", text)

    def test_a_broken_lookup_fails_the_step_not_the_job(self):
        for workflow in proved_tree.BUILT:
            with self.subTest(workflow=workflow):
                asks = step(body(workflow), "Was this tree already proved?")
                self.assertIn("continue-on-error: true", asks)
                self.assertRegex(asks, r"timeout-minutes: [1-4]\n")
                self.assertIn("steps.filter.outputs.code != 'false'", asks)

    def test_each_workflow_has_a_nightly_its_built_jobs_run_on(self):
        for workflow, jobs in proved_tree.BUILT.items():
            text = body(workflow)
            with self.subTest(workflow=workflow):
                self.assertRegex(text, r"\n  schedule:\n    - cron: '")
            for base in jobs:
                job = text[text.index(f"\n  {base}:\n"):]
                job = job[:re.search(r"\n  \S", job[1:]).start() + 1]
                with self.subTest(workflow=workflow, job=base):
                    self.assertIn("github.event_name == 'schedule'", job)


if __name__ == "__main__":
    unittest.main()
