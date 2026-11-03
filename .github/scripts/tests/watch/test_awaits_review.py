#!/usr/bin/env python3
"""#1764: a pull request with no review decision is handed back before any push,
and comes back in line when a comment records one (or its labels change).

The take is driven with `gh` and `git` patched out where they are defined
(`gitio.git`, as `test_taking.py` does), so it never fetches; the loop by the
fake world.
"""

from __future__ import annotations

import tempfile
import unittest
from unittest import mock

from merging.queue import cli, gitio, outcomes, pulls, taking, update
from merging.verdict import reviewed
from merging.watch import ending, handbacks, loop
from tests.queue.fixtures import READY, REVIEW
from tests.watch import fakes
from tests.watch.fakes import pr

PULL = {**READY, "body": "Closes #1764\nGates: green", "headRefName": "1764-x", "files": [{"path": "src/a.rs"}]}


def take(*argv: str, comments=(), pull=PULL):
    with mock.patch.object(pulls, "look", return_value=pull), \
            mock.patch.object(taking.closing, "closed_by", return_value={}), \
            mock.patch.object(reviewed, "comments", return_value=list(comments)), \
            mock.patch.object(gitio, "git"), \
            mock.patch.object(update, "advance", return_value=(None, ["stop here"])) as advanced, \
            mock.patch("builtins.print") as printed:
        result = taking.take("o/r", 1, cli.parse(["1", *argv]))
    return result, advanced, " ".join(str(c.args) for c in printed.call_args_list)


class Taken(unittest.TestCase):
    def test_no_decision_is_a_review_hand_back_before_any_push(self):
        result, advanced, said = take()
        advanced.assert_not_called()
        self.assertEqual((result[1], result.claim), (outcomes.AWAITS_REVIEW, handbacks.claim(PULL)))
        self.assertIn("review decision", said)

    def test_a_dry_run_reports_the_refusal_and_does_nothing(self):
        result, advanced, said = take("--dry-run")
        advanced.assert_not_called()
        self.assertEqual(result[1], outcomes.AWAITS_REVIEW)
        self.assertIn("review decision", said)

    def test_a_recorded_review_goes_on_to_the_update(self):
        _, advanced, _ = take(comments=REVIEW)
        advanced.assert_called_once()

    def test_a_dry_run_hands_nothing_back_for_real(self):
        self.assertIn(outcomes.AWAITS_REVIEW, outcomes.AWAITING)


ONE = {**pr(1, head="pushed1"), "body": "b"}


def run(listings, ends, decided):
    """#1 handed back for its review, then taken by `ends` in turn."""
    with tempfile.TemporaryDirectory() as tmp:
        world = fakes.World(tmp, listings, {1: outcomes.HANDED_BACK})
        ends, said, asked = iter(ends), [], []
        world.fx.say = lambda line, *rest: said.append(line)
        world.fx.decided = lambda n: asked.append(n) or (decided.pop(0) if decided else None)

        def turn(number, tick=None):
            world.taken.append(number)
            world.verdicts[number] = state = next(ends)
            if state != outcomes.AWAITS_REVIEW:
                return number, state, "why"
            return outcomes.Awaiting(number, state, "no comment records its review decision", handbacks.claim(ONE))

        world.fx.turn = turn
        loop.run(world.fx, fakes.opts(idle=5, poll=60))
        stored = handbacks.recall(world.fx.memory.with_name(handbacks.REVIEWS_MEMORY))
        return world.taken, said, asked, stored


class Watched(unittest.TestCase):
    def test_a_comment_appearing_puts_it_back_in_line(self):
        taken, said, _, _ = run([[ONE]], [outcomes.AWAITS_REVIEW, outcomes.MERGED], [None, False, True])
        self.assertEqual(taken, [1, 1])
        self.assertIn("watch: #1 has its review decision recorded now; back in line.", said)

    def test_without_a_comment_it_stays_passed_over_and_remembered(self):
        taken, said, asked, stored = run([[ONE]], [outcomes.AWAITS_REVIEW], [])
        self.assertEqual((taken, stored), ([1], {1: outcomes.AWAITS_REVIEW}))
        self.assertTrue(asked)
        self.assertIn(ending.handed_back(1, "no comment records its review decision", outcomes.AWAITS_REVIEW), said)

    def test_only_a_remembered_review_hand_back_is_asked(self):
        self.assertEqual(handbacks.reviewed([pr(1)], {1: "sha1"}, {}, lambda n: True), [])
        self.assertEqual(handbacks.reviewed([pr(1, draft=True)], {1: "sha1"}, {1: "x"}, lambda n: True), [])
        self.assertEqual(handbacks.reviewed([pr(1)], {1: "sha1"}, {1: "x"}, lambda n: True), [1])

    def test_the_line_says_what_clears_it(self):
        line = ending.handed_back(1, "why", outcomes.AWAITS_REVIEW)
        self.assertIn("No review: <why>", line)
        self.assertIn("Take the `queue` label off while must-fix findings are open", line)


if __name__ == "__main__":
    unittest.main()
