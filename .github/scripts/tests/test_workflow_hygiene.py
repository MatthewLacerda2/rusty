#!/usr/bin/env python3
"""The gating workflows keep the shape #930 gave them.

Each rule here protects the merge queue from a run it cannot judge: a hung job
holding a runner for GitHub's six-hour default, a runner image (and so the
lavapipe driver under the GPU tests) moving with nothing in any diff, or a
`ready_for_review` run cancelling a run of the very same commit. The last one
ties together the one list written in three places: which Markdown is code
because tests parse it (`docs/api/`, #525, #587).
"""

from __future__ import annotations

import importlib.util
import re
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]
WORKFLOW_DIR = SCRIPTS.parent / "workflows"
GATING = ("ci.yml", "lint.yml")

_spec = importlib.util.spec_from_file_location("merge_queue_hygiene", SCRIPTS / "merge-queue.py")
queue = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(queue)
mergeable = queue.mergeable


def jobs(workflow: str) -> dict[str, str]:
    """Each top-level job's name and its body, from the workflow's text."""
    text = (WORKFLOW_DIR / workflow).read_text().split("\njobs:\n", 1)[1]
    parts = re.split(r"^  ([\w-]+):\n", text, flags=re.M)
    return dict(zip(parts[1::2], parts[2::2]))


class Runners(unittest.TestCase):
    def test_every_gating_job_has_a_timeout(self):
        for workflow in GATING:
            for name, body in jobs(workflow).items():
                with self.subTest(workflow=workflow, job=name):
                    self.assertRegex(body, r"(?m)^    timeout-minutes: \d+$")

    def test_runner_images_are_pinned(self):
        for workflow in GATING:
            text = (WORKFLOW_DIR / workflow).read_text()
            with self.subTest(workflow=workflow):
                self.assertNotIn("-latest", text)

    def test_no_dead_merge_group_trigger(self):
        # GitHub's native merge queue is not enabled; `make queue` is the queue.
        for workflow in GATING:
            with self.subTest(workflow=workflow):
                self.assertNotIn("merge_group", (WORKFLOW_DIR / workflow).read_text())


class Concurrency(unittest.TestCase):
    def test_only_a_push_to_a_ready_pr_cancels(self):
        for workflow in GATING:
            text = (WORKFLOW_DIR / workflow).read_text()
            block = text.split("\nconcurrency:\n", 1)[1].split("\n\n", 1)[0]
            with self.subTest(workflow=workflow):
                self.assertIn("!github.event.pull_request.draft", block)
                self.assertIn("github.run_id", block)
                self.assertIn("github.event.action == 'synchronize'", block)


class DocsThatAreCode(unittest.TestCase):
    """`docs/api/` is code in ci.yml's path filter, `mergeable`, and the queue."""

    def test_ci_path_filter_reincludes_exactly_the_code_docs(self):
        body = jobs("ci.yml")["changes"]
        reincluded = re.findall(r"^\s+- '([^'!][^']*)'$", body, flags=re.M)
        self.assertEqual(sorted(reincluded), sorted(f"{p}**" for p in mergeable.CODE_DOCS))

    def test_mergeable_counts_them_as_code(self):
        for prefix in mergeable.CODE_DOCS:
            path = f"{prefix}Transform.md"
            with self.subTest(path=path):
                self.assertFalse(mergeable.inert([path]))
                self.assertTrue(mergeable.needs_drift_run([path]))

    def test_the_queue_rebases_over_them(self):
        for prefix in mergeable.CODE_DOCS:
            with self.subTest(prefix=prefix):
                self.assertFalse(queue.docs_advance(f">abc\n{prefix}Transform.md\n"))
        self.assertTrue(queue.docs_advance(">abc\nREADME.md\n"))


if __name__ == "__main__":
    unittest.main()
