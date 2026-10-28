#!/usr/bin/env python3
"""A mutation run is one per branch and dies with its pull request (#1530).

`mutants-on-request.yml` groups its runs by ref and cancels the older one;
`mutants-cancel.yml` joins that same group when a pull request closes unmerged,
which is the only thing that stops a run on an abandoned branch. A merged
branch's run is never cancelled: its report is what the orchestrator triages.
On `main` every run is its own group, so re-asks never cancel each other. The
two group strings live in two files, so these tests hold them equal, and keep
both apart from the gating workflows' groups so neither can ever cancel a `ci`
or `lint` run.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

WORKFLOWS = Path(__file__).resolve().parents[3] / "workflows"
GATING = ("ci.yml", "lint.yml")


def groups(text: str) -> list[tuple[str, str]]:
    """Each `concurrency:` block's group and cancel-in-progress, in order."""
    found = re.findall(r"concurrency:\n\s+group: (.+)\n\s+cancel-in-progress: (.+)\n", text)
    return [(g.strip(), c.strip()) for g, c in found]


def resolve(group: str, **context: str) -> str:
    """The group's string once its `${{ … }}` expressions are given values.

    Knows the two shapes the mutation groups use: a bare context value, and
    GitHub's ternary idiom `a == 'x' && b || c`.
    """
    def value(expr: str) -> str:
        ternary = re.fullmatch(r"([\w.]+) == '([^']*)' && ([\w.]+) \|\| ([\w.]+)", expr)
        if ternary:
            test, literal, then, other = ternary.groups()
            return context[then] if context[test] == literal else context[other]
        return context[expr]
    return re.sub(r"\$\{\{\s*(.+?)\s*\}\}", lambda m: value(m.group(1)), group)


def on_ref(ref: str, run_id: str = "1") -> dict[str, str]:
    return {"github.ref": ref, "github.run_id": run_id}


class Concurrency(unittest.TestCase):
    def setUp(self):
        self.request = groups((WORKFLOWS / "mutants-on-request.yml").read_text())
        self.cancel = groups((WORKFLOWS / "mutants-cancel.yml").read_text())

    def test_a_newer_dispatch_cancels_the_older_run(self):
        [(request, cancel)] = self.request
        self.assertEqual(cancel, "true")
        ref = "refs/heads/1530-mutants-concurrency"
        self.assertEqual(resolve(request, **on_ref(ref, "1")), resolve(request, **on_ref(ref, "2")))

    def test_re_asks_on_main_never_cancel_each_other(self):
        [(request, _)] = self.request
        self.assertNotEqual(resolve(request, **on_ref("refs/heads/main", "1")),
                            resolve(request, **on_ref("refs/heads/main", "2")))

    def test_closing_a_pull_request_joins_its_branchs_group(self):
        [(group, cancel)] = self.cancel
        self.assertEqual(cancel, "true")
        [(request, _)] = self.request
        branch = "1530-mutants-concurrency"
        self.assertEqual(resolve(group, **{"github.head_ref": branch}),
                         resolve(request, **on_ref(f"refs/heads/{branch}")))

    def test_a_different_branch_is_a_different_group(self):
        [(group, _)] = self.cancel
        [(request, _)] = self.request
        self.assertNotEqual(resolve(group, **{"github.head_ref": "a"}),
                            resolve(request, **on_ref("refs/heads/b")))

    def test_the_closer_fires_only_on_close_and_holds_no_token(self):
        text = (WORKFLOWS / "mutants-cancel.yml").read_text()
        on = text[text.index("\non:\n"):text.index("\npermissions:")]
        self.assertEqual(re.findall(r"^  (\w+):", on, re.M), ["pull_request"])
        self.assertIn("types: [closed]", on)
        self.assertIn("\npermissions: {}\n", text)

    def test_only_an_unmerged_close_cancels(self):
        # A skipped job never joins its concurrency group, so the `if:` is the
        # whole guard that keeps a merge from cancelling the report.
        text = (WORKFLOWS / "mutants-cancel.yml").read_text()
        job = text[text.index("\n  cancel:\n"):text.index("\n    concurrency:") + 1]
        self.assertIn("\n    if: github.event.pull_request.merged == false && ", job)

    def test_a_forks_pull_request_cancels_nothing(self):
        # A fork's branch can share a name with one here; its close is skipped.
        text = (WORKFLOWS / "mutants-cancel.yml").read_text()
        [cond] = re.findall(r"\n    if: (.+)\n", text)
        self.assertIn("github.event.pull_request.head.repo.full_name == github.repository",
                      cond.split(" && "))

    def test_no_gating_group_can_be_reached(self):
        # The gating groups start with the workflow's name (`ci-`, `lint-`).
        for workflow in GATING:
            text = (WORKFLOWS / workflow).read_text()
            name = text.splitlines()[0].removeprefix("name: ")
            with self.subTest(workflow=workflow):
                # None found would pass vacuously, e.g. after a reordered block.
                self.assertTrue(groups(text), f"no concurrency group read from {workflow}")
                for group, _ in groups(text):
                    self.assertTrue(group.startswith("${{ github.workflow }}-"), group)
                self.assertFalse(f"{name}-".startswith("mutants-on-request-"))


if __name__ == "__main__":
    unittest.main()
