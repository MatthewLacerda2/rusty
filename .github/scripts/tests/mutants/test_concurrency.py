#!/usr/bin/env python3
"""A mutation run is one per branch and dies with its pull request (#1530).

`mutants-on-request.yml` groups its runs by ref and cancels the older one;
`mutants-cancel.yml` joins that same group when a pull request closes, which is
the only thing that stops a run on a merged branch. The two group strings live
in two files, so these tests hold them equal, and keep both apart from the
gating workflows' groups so neither can ever cancel a `ci` or `lint` run.
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
    """The group's string once its `${{ … }}` expressions are given values."""
    return re.sub(r"\$\{\{\s*([\w.]+)\s*\}\}", lambda m: context[m.group(1)], group)


class Concurrency(unittest.TestCase):
    def setUp(self):
        self.request = groups((WORKFLOWS / "mutants-on-request.yml").read_text())
        self.cancel = groups((WORKFLOWS / "mutants-cancel.yml").read_text())

    def test_a_newer_dispatch_cancels_the_older_run(self):
        self.assertEqual(self.request, [("mutants-on-request-${{ github.ref }}", "true")])

    def test_closing_a_pull_request_joins_its_branchs_group(self):
        [(group, cancel)] = self.cancel
        self.assertEqual(cancel, "true")
        [(request, _)] = self.request
        branch = "1530-mutants-concurrency"
        self.assertEqual(resolve(group, **{"github.head_ref": branch}),
                         resolve(request, **{"github.ref": f"refs/heads/{branch}"}))

    def test_a_different_branch_is_a_different_group(self):
        [(group, _)] = self.cancel
        [(request, _)] = self.request
        self.assertNotEqual(resolve(group, **{"github.head_ref": "a"}),
                            resolve(request, **{"github.ref": "refs/heads/b"}))

    def test_the_closer_fires_only_on_close_and_holds_no_token(self):
        text = (WORKFLOWS / "mutants-cancel.yml").read_text()
        on = text[text.index("\non:\n"):text.index("\npermissions:")]
        self.assertEqual(re.findall(r"^  (\w+):", on, re.M), ["pull_request"])
        self.assertIn("types: [closed]", on)
        self.assertIn("\npermissions: {}\n", text)

    def test_no_gating_group_can_be_reached(self):
        # The gating groups start with the workflow's name (`ci-`, `lint-`).
        for workflow in GATING:
            text = (WORKFLOWS / workflow).read_text()
            name = text.splitlines()[0].removeprefix("name: ")
            with self.subTest(workflow=workflow):
                for group, _ in groups(text):
                    self.assertTrue(group.startswith("${{ github.workflow }}-"), group)
                self.assertFalse(f"{name}-".startswith("mutants-on-request-"))


if __name__ == "__main__":
    unittest.main()
