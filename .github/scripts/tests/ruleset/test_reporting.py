#!/usr/bin/env python3
"""Every check the ruleset requires reports on every kind of pull request (#491).

A required check that never reports on some pull request leaves it "Expected"
forever: GitHub will not merge it, and nothing says why (#218 wedged
Markdown-only PRs that way, back when the required checks were the build jobs).
So each required context must be a gate job of a gating workflow that runs on
every pull request event the queue merges after (no `paths` filter on the
trigger, `ready_for_review` among the types), and whose gate job runs whatever
its jobs did (`!cancelled()`), so a Markdown-only or draft run still reports.
"""

from __future__ import annotations

import json
import re
import unittest

import ruleset
from tests.test_workflow_hygiene import WORKFLOW_DIR, jobs

GATES = {"ci-gate": "ci.yml", "lint-gate": "lint.yml"}
GITHUB_ACTIONS = 15368


def required() -> list[dict]:
    rules = json.loads(ruleset.FILE.read_text())["rules"]
    checks = next(r for r in rules if r["type"] == "required_status_checks")
    return checks["parameters"]["required_status_checks"]


def trigger(workflow: str) -> str:
    """The workflow's `on:` block."""
    text = (WORKFLOW_DIR / workflow).read_text()
    return text[text.index("\non:\n") : text.index("\njobs:\n")]


class Reporting(unittest.TestCase):
    def test_the_required_checks_are_the_two_gates_from_github_actions(self):
        self.assertEqual({c["context"]: c["integration_id"] for c in required()},
                         dict.fromkeys(GATES, GITHUB_ACTIONS))

    def test_each_gate_is_a_job_that_runs_unless_cancelled(self):
        for gate, workflow in GATES.items():
            with self.subTest(gate=gate):
                body = jobs(workflow)[gate]
                self.assertRegex(body, r"(?m)^    if: \$\{\{ !cancelled\(\) \}\}$")
                self.assertNotRegex(body, r"(?m)^    name:")

    def test_each_gating_workflow_runs_on_every_pull_request(self):
        for workflow in set(GATES.values()):
            with self.subTest(workflow=workflow):
                on = trigger(workflow)
                self.assertNotRegex(on, r"paths(-ignore)?:")
                self.assertRegex(on, r"pull_request:\n    types: \[[^\]]*\bready_for_review\b")

    def test_not_strict(self):
        # Strict would refuse a head the queue left behind a Markdown-only `main` (#587).
        rules = json.loads(ruleset.FILE.read_text())["rules"]
        params = next(r for r in rules if r["type"] == "required_status_checks")["parameters"]
        self.assertIs(params["strict_required_status_checks_policy"], False)


class Flags(unittest.TestCase):
    """The checks above flag what they guard against."""

    def test_a_paths_filter_is_seen(self):
        self.assertRegex("on:\n  pull_request:\n    paths: ['src/**']\n", r"paths(-ignore)?:")

    def test_a_draft_skip_on_the_gate_is_seen(self):
        body = "    if: ${{ !cancelled() && !github.event.pull_request.draft }}\n"
        self.assertNotRegex(body, r"(?m)^    if: \$\{\{ !cancelled\(\) \}\}$")


if __name__ == "__main__":
    unittest.main()
