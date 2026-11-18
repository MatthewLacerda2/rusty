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


def job(text: str, base: str) -> str:
    """The job called `base`, up to the next job."""
    start = text.index(f"\n  {base}:\n")
    end = re.search(r"\n  \S", text[start + 1:])
    return text[start:start + 1 + end.start()] if end else text[start:]


def step(text: str, name: str) -> str:
    """The step called `name`, up to the next step or job."""
    start = text.index(f"- name: {name}\n")
    end = re.search(r"\n\s*- (uses|name):|\n  \S", text[start + 1:])
    return text[start:start + 1 + end.start()] if end else text[start:]


class Wiring(unittest.TestCase):
    def test_built_names_match_the_workflows(self):
        for workflow, jobs in proved_tree.BUILT.items():
            text = body(workflow)
            for base, names in jobs.items():
                with self.subTest(workflow=workflow, job=base):
                    self.assertIn(f"\n  {base}:\n", text)
                    for name in names:
                        self.assertIn(f"- name: {name}\n", job(text, base))

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

    def test_a_ready_pull_request_asks_with_its_event_and_a_draft_does_not(self):
        # #2333: the script reads the merge on `pull_request` only when told the event.
        ready = "(github.event_name == 'pull_request' && !github.event.pull_request.draft)"
        for workflow in proved_tree.BUILT:
            with self.subTest(workflow=workflow):
                text = job(body(workflow), "changes")
                asks = step(text, "Was this tree already proved?")
                self.assertIn(f"if: (github.event_name == 'push' || {ready}) &&", asks)
                self.assertIn('--event "${{ github.event_name }}" --sha "${{ github.sha }}"', asks)
                self.assertIn(f"- uses: actions/checkout@v7\n        if: github.event_name == 'push' || {ready}\n",
                              text)

    def test_a_decider_change_never_asks_the_script(self):
        # #2333: a PR's step runs the PR's own copy, so the YAML holds this, not the script.
        paths = ("'.github/scripts/proved_tree.py'", "'.github/scripts/ghrest.py'",
                 "'.github/scripts/merging/verdict/**'", "'.github/workflows/ci.yml'",
                 "'.github/workflows/lint.yml'")
        for workflow in proved_tree.BUILT:
            with self.subTest(workflow=workflow):
                text = job(body(workflow), "changes")
                decider = re.search(r"\n            decider:\n((?:(?: {14}- | {12}#).*\n)+)", text).group(1)
                for path in paths:
                    self.assertIn(f"              - {path}\n", decider)
                asks = step(text, "Was this tree already proved?")
                self.assertIn("&& steps.filter.outputs.decider != 'true'\n", asks)

    def test_the_decider_lists_what_the_script_imports(self):
        source = (SCRIPTS / "proved_tree.py").read_text(encoding="utf-8")
        for module in re.findall(r"^(?:import|from) (ghrest|merging\S*)", source, re.M):
            with self.subTest(module=module):
                path = ".github/scripts/" + module.replace(".", "/") + ".py"
                self.assertIn(path, proved_tree.DECIDER)

    def test_each_workflow_has_a_nightly_its_built_jobs_run_on(self):
        for workflow, jobs in proved_tree.BUILT.items():
            text = body(workflow)
            with self.subTest(workflow=workflow):
                self.assertRegex(text, r"\n  schedule:\n    - cron: '")
            for base in jobs:
                with self.subTest(workflow=workflow, job=base):
                    self.assertIn("github.event_name == 'schedule'", job(text, base))


if __name__ == "__main__":
    unittest.main()
