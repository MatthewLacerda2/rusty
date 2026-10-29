#!/usr/bin/env python3
"""What a pull request claims, refused before any run is read (#927).

`planning` is the one rule nothing overrides, so the check is tested where it
decides — [`closing.refusal`] and [`protocol.refusal`], and the two callers that
ask them, [`decide.judge`] and the queue's take — on plain dictionaries.
"""

from __future__ import annotations

import subprocess
import unittest
from unittest import mock

from merging.queue import cli, outcomes, pulls, taking, update
from merging.verdict import closing, github, protocol
from tests.verdict.fixtures import READY, both, verdict

GATED = "## Gates\n`make gates` green on abc1234."


def issue(*labels: str, state: str = "open", **fields: object) -> dict:
    """GitHub's REST answer for an issue carrying these labels."""
    return {"state": state, "labels": [{"name": name} for name in labels], **fields}


def answers(issues: dict[int, dict], blockers: dict[int, list[dict]] | None = None):
    """A fake `github.gh` answering `issues/N` and `issues/N/dependencies/blocked_by`."""
    def gh(_api: str, path: str, missing=None):
        number = int(path.split("/issues/")[1].split("/")[0].split("?")[0])
        if "blocked_by" in path:
            return (blockers or {}).get(number, [])
        return issues.get(number, missing)
    return gh


def claiming(closes: dict[int, dict], body: str = f"Closes #927\n\n{GATED}", branch: str = "927-checks") -> dict:
    return {**READY, "headRefName": branch, "body": body, "closes": closes}


class Referenced(unittest.TestCase):
    def test_every_keyword_and_every_link_is_read(self):
        pull = {"body": "Closes #1\nfixes #2, Resolved: #3\nrefs #4 and see #5", "closingIssuesReferences": [{"number": 6}]}
        self.assertEqual(closing.referenced(pull), [1, 2, 3, 6])

    def test_a_markdown_link_is_not_a_keyword(self):
        # How Dependabot quotes an upstream changelog: a link, never `#N` bare.
        self.assertEqual(closing.referenced({"body": "Fixes [#123](https://x/y/issues/123)"}), [])


class Refusal(unittest.TestCase):
    def test_an_open_issue_without_a_stage_label_is_fine(self):
        self.assertEqual(closing.refusal({7: issue("bug", "agent")}), [])
        self.assertEqual(closing.refusal({}), [])

    def test_each_stage_label_refuses_by_issue_and_label(self):
        for stage in closing.STAGES:
            lines = closing.refusal({957: issue("feature", stage)})
            self.assertEqual(lines[0], f"it closes #957, which is labelled `{stage}`.")
            self.assertIn("Only the operator removes the label", lines[-1])

    def test_a_wrong_number_is_named(self):
        lines = closing.refusal({1: issue(pull_request={}), 2: issue(state="closed"), 3: closing.MISSING})
        self.assertIn("#1, which is a pull request", lines[0])
        self.assertIn("#2, which is already closed", lines[1])
        self.assertIn("#3, which does not exist", lines[2])
        self.assertIn("Refs #N", lines[-1])


class Protocol(unittest.TestCase):
    def test_a_branch_named_for_an_issue_must_name_it(self):
        lines = protocol.unnamed({"headRefName": "927-checks", "body": "Ports the checks."})
        self.assertIn("does not close #927", lines[0])

    def test_closing_or_refs_names_it(self):
        for body in ("Closes #927", "fixes: #927", "Refs #927"):
            self.assertEqual(protocol.unnamed({"headRefName": "927-checks", "body": body}), [], body)

    def test_a_branch_not_named_for_an_issue_or_an_unread_body_is_not_judged(self):
        self.assertEqual(protocol.unnamed({"headRefName": "document-ai-parity", "body": ""}), [])
        self.assertEqual(protocol.unnamed({"headRefName": "927-checks"}), [])

    def test_a_gates_line_in_any_common_shape_is_found(self):
        for body in ("Gates: all green", "**Gates**: green", "## Gates", "- **Gates** green"):
            self.assertEqual(protocol.gates({"body": body}), [], body)
        self.assertIn("no **Gates** line", protocol.gates({"body": "Closes #1"})[0])


class Judged(unittest.TestCase):
    def test_a_green_pull_request_closing_a_planning_issue_is_refused(self):
        ok, lines = verdict(*both(), pull=claiming({927: issue("infrastructure", "planning")}))
        self.assertFalse(ok)
        self.assertEqual(lines[0], "it closes #927, which is labelled `planning`.")

    def test_it_passes_once_the_label_is_removed(self):
        ok, lines = verdict(*both(), pull=claiming({927: issue("infrastructure")}))
        self.assertTrue(ok, lines)
        self.assertFalse(any("Warning" in line for line in lines))

    def test_a_branch_whose_issue_goes_unnamed_is_refused(self):
        ok, lines = verdict(*both(), pull=claiming({}, body=GATED))
        self.assertFalse(ok)
        self.assertIn("does not close #927", lines[0])

    def test_a_missing_gates_line_warns_and_still_passes(self):
        ok, lines = verdict(*both(), pull=claiming({927: issue()}, body="Closes #927"))
        self.assertTrue(ok)
        self.assertIn("no **Gates** line", lines[-1])


class Taken(unittest.TestCase):
    def test_the_queue_hands_back_before_any_rebase(self):
        pull = {**claiming({}), "state": "OPEN", "title": "t", "author": {"login": "x"}}
        with mock.patch.object(pulls, "look", return_value=pull), \
            mock.patch.object(github, "gh", side_effect=answers({927: issue("human")})), \
            mock.patch.object(update, "advance") as rebased, mock.patch("builtins.print"):
            _, state, why = taking.take("o/r", 927, cli.parse(["927"]))
        self.assertEqual(state, outcomes.AWAITS_ISSUE)  # only the issue can change (#1279)
        self.assertEqual(why, "it closes #927, which is labelled `human`.")
        rebased.assert_not_called()


class Missing(unittest.TestCase):
    def test_a_404_is_the_callers_missing_value_not_an_exit(self):
        gone = subprocess.CompletedProcess([], 1, "", "gh: Not Found (HTTP 404)")
        self.assertIs(github.gh("api", "x", runner=lambda *a, **k: gone, missing=closing.MISSING), closing.MISSING)
        with self.assertRaises(SystemExit):
            github.gh("api", "x", runner=lambda *a, **k: gone)


if __name__ == "__main__":
    unittest.main()
