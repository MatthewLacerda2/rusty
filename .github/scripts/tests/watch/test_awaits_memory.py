#!/usr/bin/env python3
"""#1279: what an `AWAITS_*` hand-back remembers, and the guards on that memory.

An issue hand-back (`planning`/`human`, a blocker, a closed issue) comes back
when the issue no longer refuses it, which no edit of the pull request does;
the claim is the one [`taking.take`] judged; and nothing outlives its hand-back:
a CI hand-back drops an old claim, a closed pull request's is not saved, and a
dry run writes no memory at all. Each test fails with its guard removed.
"""

from __future__ import annotations

import argparse
import tempfile
import unittest
from unittest import mock

from merging.queue import cli, outcomes, pulls, rebase, taking, world
from merging.verdict import github
from merging.watch import freshness, handbacks, loop
from tests.verdict.fixtures import READY
from tests.watch import fakes
from tests.watch.test_awaits import described, run

HEADS = handbacks.MEMORY.split("/")[-1]


class Issue(unittest.TestCase):
    def test_comes_back_once_the_issue_no_longer_blocks_it(self):
        same = described(1, "Closes #1265")
        answers = iter([[1265], None, []])  # still blocked, GitHub failed, cleared
        taken, said, _ = run([[same]] * 5, [outcomes.AWAITS_ISSUE, outcomes.MERGED], blocks=lambda issues: next(answers))
        self.assertEqual(taken, [1, 1])
        self.assertIn("watch: #1's issues no longer refuse it; back in line.", said)

    def test_github_failing_is_no_news(self):
        taken, _, _ = run([[described(1, "Closes #1265")]], [outcomes.AWAITS_ISSUE], blocks=lambda issues: None)
        self.assertEqual(taken, [1])

    def test_remembers_which_issues_to_ask_about(self):
        taken, _, issues = run([[described(1, "Closes #1265")]], [outcomes.AWAITS_ISSUE], memory=handbacks.ISSUES_MEMORY)
        self.assertEqual((taken, issues), ([1], {1: "1265"}))

    def test_a_draft_is_not_asked(self):
        asked = []
        handbacks.unblocked([fakes.pr(1, draft=True)], {1: "h"}, {1: "1265"}, lambda i: asked.append(i) or [])
        self.assertEqual(asked, [])


class Guards(unittest.TestCase):
    def test_a_ci_hand_back_drops_the_claim_an_earlier_edit_hand_back_left(self):
        pull = described(1, "x")  # the claim matches, so only the new hand-back can drop it
        recalled = {HEADS: {1: "old"}, handbacks.CLAIMS_MEMORY: {1: handbacks.claim(pull)}}
        taken, _, claims = run([[pull]], [outcomes.HANDED_BACK], recalled=recalled)
        self.assertEqual((taken, claims), ([1], {}))

    def test_a_closed_pull_requests_claim_is_not_saved(self):
        recalled = {HEADS: {1: "pushed1"}, handbacks.CLAIMS_MEMORY: {1: "c"}}
        taken, _, claims = run([[{**fakes.pr(2, draft=True), "body": "x"}]], [], recalled=recalled)
        self.assertEqual((taken, claims), ([], {}))

    def test_a_claim_without_a_remembered_head_is_not_reworded(self):
        self.assertEqual(handbacks.reworded([described(1, "new")], {}, {1: "old"}), [])

    def test_a_dry_run_writes_neither_the_heads_nor_the_claims(self):
        opts = argparse.Namespace(root=".", dry_run=True)
        done = mock.Mock(stdout=".git")
        with mock.patch.object(freshness, "launched", return_value={}), mock.patch.object(world.gitio, "git", return_value=done):
            ends = world.effects("o/r", opts).ends
        with tempfile.TemporaryDirectory() as tmp:
            dry = fakes.World(tmp, [[described(1, "x")]], {})
            dry.fx.ends = ends
            dry.fx.turn = lambda n, tick=None: outcomes.Awaiting(n, outcomes.AWAITS_EDIT, "why", "c")
            loop.run(dry.fx, fakes.opts("--dry-run", idle=5, poll=60))
            stored = [handbacks.recall(dry.fx.memory.with_name(m)) for m in (HEADS, handbacks.CLAIMS_MEMORY)]
        self.assertEqual(stored, [{}, {}])


class Taken(unittest.TestCase):
    """The claim and issues come from the pull request `take` read, not a listing."""

    def take(self, closes):
        pull = {**READY, "state": "OPEN", "title": "t", "author": {"login": "x"}, "headRefName": "1265-x",
                "body": "Closes #1265", "files": [{"path": "README.md"}]}
        with mock.patch.object(pulls, "look", return_value=pull), \
                mock.patch.object(taking.closing, "closed_by", return_value=closes), mock.patch.object(github, "gh"), \
                mock.patch.object(rebase, "advance") as rebased, mock.patch("builtins.print"):
            result = taking.take("o/r", 1, cli.parse(["1"]))
        rebased.assert_not_called()
        return result, handbacks.claim(pull)

    def test_a_wrong_number_waits_for_an_edit(self):
        result, claim = self.take({1265: {"state": "open", "pull_request": {}}})
        self.assertEqual((result[1], result.claim, result.issues), (outcomes.AWAITS_EDIT, claim, ()))

    def test_a_human_issue_waits_for_the_issue(self):
        result, claim = self.take({1265: {"state": "open", "labels": [{"name": "human"}]}})
        self.assertEqual((result[1], result.claim, result.issues), (outcomes.AWAITS_ISSUE, claim, (1265,)))

    def test_a_closed_issue_waits_for_the_issue(self):
        result, _ = self.take({1265: {"state": "closed"}})
        self.assertEqual((result[1], result.issues), (outcomes.AWAITS_ISSUE, (1265,)))


if __name__ == "__main__":
    unittest.main()
