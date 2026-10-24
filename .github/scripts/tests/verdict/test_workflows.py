#!/usr/bin/env python3
"""The verdict's gate table is held to the workflow files it describes.

The workflow files are read directly so the table cannot drift from them.
"""

from __future__ import annotations

import re
import subprocess
import sys
import unittest

from merging.verdict import changes, runlist
from tests import SCRIPTS

SCRIPT = SCRIPTS / "mergeable.py"
WORKFLOW_DIR = SCRIPTS.parent / "workflows"


def gate_needs(workflow_file: str, gate: str) -> list[str]:
    """The `needs:` of `gate` in a workflow file — flow-style list, one line."""
    text = (WORKFLOW_DIR / workflow_file).read_text()
    found = re.search(rf"^  {re.escape(gate)}:\n    needs: \[([^\]]*)\]", text, re.M)
    assert found, f"{workflow_file} has no `{gate}` job with a flow-style `needs:`"
    return [n.strip() for n in found.group(1).split(",")]


class Workflows(unittest.TestCase):
    """The table in `mergeable.py` is held to the workflow files it describes."""

    FILES = {"ci": "ci.yml", "lint": "lint.yml"}

    def test_each_gates_needs_are_changes_plus_the_gated_jobs(self):
        # A gating job added to a gate's `needs:` and not here would be one a
        # pass never asks about. `changes` is excluded on purpose (see WORKFLOWS).
        for workflow, (gate, gated) in runlist.WORKFLOWS.items():
            with self.subTest(workflow=workflow):
                needs = gate_needs(self.FILES[workflow], gate)
                self.assertEqual(sorted(needs), sorted(("changes", *gated)))

    def test_each_workflows_name_is_the_one_the_runs_api_reports(self):
        for workflow, file in self.FILES.items():
            with self.subTest(workflow=workflow):
                first = (WORKFLOW_DIR / file).read_text().splitlines()[0]
                self.assertEqual(first, f"name: {workflow}")

    def test_windows_is_a_signal_not_a_gate(self):
        # #741: the merge never waits on Windows. It is its own workflow, which
        # nothing here reads, and no `ci` job runs on a Windows runner.
        self.assertNotIn("windows", runlist.WORKFLOWS)
        self.assertNotIn("windows-latest", (WORKFLOW_DIR / "ci.yml").read_text())
        first = (WORKFLOW_DIR / "windows.yml").read_text().splitlines()[0]
        self.assertEqual(first, "name: windows")

    def test_windows_never_runs_on_pull_requests(self):
        # #1395: a per-PR Windows run starved the merge queue's gating runs of
        # runners. It runs nightly and on dispatch only.
        text = (WORKFLOW_DIR / "windows.yml").read_text()
        on = text[text.index("\non:\n") : text.index("\njobs:\n")]
        self.assertNotIn("pull_request", on)
        self.assertNotIn("push:", on)

    def test_macos_still_gates(self):
        # The other shipped platform: in `ci-gate`'s `needs:` and on its runner.
        self.assertIn("build-test-cross", gate_needs("ci.yml", "ci-gate"))
        self.assertRegex((WORKFLOW_DIR / "ci.yml").read_text(), r"os: \[macos-\d+\]")

    def test_build_test_still_has_the_step_the_drift_check_looks_for(self):
        text = (WORKFLOW_DIR / "ci.yml").read_text()
        body = text.split(f"\n  {changes.DRIFT_JOB}:\n", 1)[1].split("\n  build-test-cross:\n", 1)[0]
        names = [n.lower() for n in re.findall(r"- name: (.+)", body)]
        self.assertTrue(any(all(w in n for w in changes.DRIFT_STEP_WORDS) for n in names), names)


class Contract(unittest.TestCase):
    def test_it_asks_for_a_pull_request_number(self):
        done = subprocess.run([sys.executable, str(SCRIPT)], capture_output=True, text=True, check=False)
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("usage", done.stderr.lower())


if __name__ == "__main__":
    unittest.main()
