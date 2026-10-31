#!/usr/bin/env python3
"""Every check the ruleset requires reports on every kind of pull request (#491).

A required check that never reports on some pull request leaves it "Expected"
forever: GitHub will not merge it, and nothing says why (#218 wedged
Markdown-only PRs that way, back when the required checks were the build jobs).
So each required context must be a gate job of a gating workflow that runs on
every pull request event the queue merges after (no `paths` filter on the
trigger; `opened`, `reopened`, `synchronize` and `ready_for_review` all among
the types), and whose gate job runs whatever
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


# The checks themselves, shared by the real tests and `Flags`, so weakening one
# turns `Flags` red.
EVENTS = {"opened", "reopened", "synchronize", "ready_for_review"}


def runs_unless_cancelled(gate: str) -> bool:
    """The gate job runs whatever its jobs did, and keeps the bare job id as its context."""
    unconditional = re.search(r"(?m)^    if: \$\{\{ !cancelled\(\) \}\}$", gate)
    return bool(unconditional) and not re.search(r"(?m)^    name:", gate)


def path_filtered(on: str) -> bool:
    return bool(re.search(r"paths(-ignore)?:", on))


def missing_events(on: str) -> set[str]:
    """The pull request events the queue merges after that this trigger leaves out."""
    types = re.search(r"pull_request:\n    types: \[([^\]]*)\]", on)
    return EVENTS - ({t.strip() for t in types[1].split(",")} if types else set())


class Reporting(unittest.TestCase):
    def test_the_required_checks_are_the_two_gates_from_github_actions(self):
        self.assertEqual({c["context"]: c["integration_id"] for c in required()},
                         dict.fromkeys(GATES, GITHUB_ACTIONS))

    def test_each_gate_is_a_job_that_runs_unless_cancelled(self):
        for gate, workflow in GATES.items():
            with self.subTest(gate=gate):
                self.assertTrue(runs_unless_cancelled(jobs(workflow)[gate]))

    def test_each_gating_workflow_runs_on_every_pull_request(self):
        for workflow in set(GATES.values()):
            with self.subTest(workflow=workflow):
                on = trigger(workflow)
                self.assertFalse(path_filtered(on))
                self.assertEqual(missing_events(on), set())

    def test_not_strict(self):
        # Strict would refuse a head the queue left behind a Markdown-only `main` (#587).
        rules = json.loads(ruleset.FILE.read_text())["rules"]
        params = next(r for r in rules if r["type"] == "required_status_checks")["parameters"]
        self.assertIs(params["strict_required_status_checks_policy"], False)


class Flags(unittest.TestCase):
    """The checks above flag what they guard against, and pass what they allow."""

    GATE = "    needs: [a]\n    if: ${{ !cancelled() }}\n"
    ON = "on:\n  pull_request:\n    types: [opened, reopened, synchronize, ready_for_review]\n"

    def test_the_real_shapes_pass(self):
        self.assertTrue(runs_unless_cancelled(self.GATE))
        self.assertFalse(path_filtered(self.ON))
        self.assertEqual(missing_events(self.ON), set())

    def test_a_paths_filter_is_seen(self):
        self.assertTrue(path_filtered(self.ON + "    paths: ['src/**']\n"))

    def test_a_draft_skip_on_the_gate_is_seen(self):
        self.assertFalse(runs_unless_cancelled("    if: ${{ !cancelled() && !github.event.pull_request.draft }}\n"))

    def test_a_renamed_gate_is_seen(self):
        self.assertFalse(runs_unless_cancelled(self.GATE + "    name: CI gate\n"))

    def test_a_trigger_cut_to_ready_for_review_is_seen(self):
        cut = "on:\n  pull_request:\n    types: [ready_for_review]\n"
        self.assertEqual(missing_events(cut), {"opened", "reopened", "synchronize"})

    def test_a_trigger_without_types_is_seen(self):
        self.assertEqual(missing_events("on:\n  push:\n"), EVENTS)


if __name__ == "__main__":
    unittest.main()
