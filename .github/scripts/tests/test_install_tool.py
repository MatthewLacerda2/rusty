#!/usr/bin/env python3
"""Workflows install cargo tools only through `.github/actions/install-tool` (#1973).

`taiki-e/install-action` called straight from a workflow downloads its tool from
GitHub releases on every job; a 504 there turned `main`'s `ci` red (#1936) and
cost PR #1972 a CI round. The composite action pins the version and restores
the tool from the Actions cache, so a direct call is flagged here. A job that
checks out a ref other than its own (`ref:`) may land on a tree from before the
action existed, so it must restore `.github/actions` from `$GITHUB_SHA` before
it calls one (#1973's review: a queued mutation request at an older commit).

An exception goes on the `uses:` line itself: `# install-tool: <reason>`.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

WORKFLOW_DIR = Path(__file__).resolve().parents[2] / "workflows"

DIRECT = re.compile(r"^\s*(?:-\s+)?uses:\s*taiki-e/install-action\b(?P<rest>.*)$", re.M)
ALLOW = re.compile(r"#\s*install-tool:\s*\S")
RESTORE = 'git checkout "$GITHUB_SHA" -- .github/actions'


def direct_installs(text: str) -> list[str]:
    """Each `uses: taiki-e/install-action` line without an inline reason."""
    return [m.group(0).strip() for m in DIRECT.finditer(text) if not ALLOW.search(m["rest"])]


def unrestored_local_actions(text: str) -> bool:
    """A checkout of another ref is followed by a local action with no restore before it."""
    lines = text.splitlines(keepends=True)
    start = None
    for i, line in enumerate(lines):
        if "uses: actions/checkout@" in line:
            # The step's own lines are indented deeper than its `- uses:` dash.
            indent = len(line) - len(line.lstrip(" -"))
            body = []
            for nxt in lines[i + 1:]:
                if nxt.strip() and len(nxt) - len(nxt.lstrip(" ")) < indent:
                    break
                body.append(nxt)
            if any(b.strip().startswith("ref:") for b in body):
                start = sum(map(len, lines[: i + 1 + len(body)]))
                break
    if start is None:
        return False
    after = text[start:]
    local = after.find("uses: ./.github/actions/")
    return local != -1 and RESTORE not in after[:local]


class InstallTool(unittest.TestCase):
    def test_no_workflow_calls_install_action_directly(self):
        for path in sorted(WORKFLOW_DIR.glob("*.yml")):
            with self.subTest(workflow=path.name):
                self.assertEqual(direct_installs(path.read_text()), [])

    def test_a_direct_call_is_flagged_and_a_reasoned_one_passes(self):
        self.assertEqual(len(direct_installs("      - uses: taiki-e/install-action@v2\n")), 1)
        self.assertEqual(len(direct_installs("        uses: taiki-e/install-action@v2  # pinned\n")), 1)
        self.assertEqual(direct_installs("        uses: ./.github/actions/install-tool\n"), [])
        self.assertEqual(direct_installs("        uses: taiki-e/install-action@v2  # install-tool: the action itself\n"), [])

    def test_a_workflow_on_another_ref_restores_the_actions_first(self):
        for path in sorted(WORKFLOW_DIR.glob("*.yml")):
            with self.subTest(workflow=path.name):
                self.assertFalse(unrestored_local_actions(path.read_text()))

    def test_a_missing_restore_is_flagged(self):
        checkout = "      - uses: actions/checkout@v7\n        with:\n          ref: ${{ inputs.commit }}\n"
        call = "      - uses: ./.github/actions/install-tool\n"
        self.assertTrue(unrestored_local_actions(checkout + call))
        self.assertFalse(unrestored_local_actions(checkout + f"      - run: {RESTORE}\n" + call))
        self.assertFalse(unrestored_local_actions("      - uses: actions/checkout@v7\n" + call))


if __name__ == "__main__":
    unittest.main()
