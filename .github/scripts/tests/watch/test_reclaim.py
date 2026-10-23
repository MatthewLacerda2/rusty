#!/usr/bin/env python3
"""#1279: a hand-back an edit fixes is re-read when the description or labels change.

#1275 was handed back for a description that did not close its issue; the line
said "editing the description needs no push", but the watch passed it over until
its head moved, so the fixed pull request sat until the daemon was restarted.
Now a [`outcomes.RECLAIM`] hand-back remembers what the body and labels were
([`handbacks.claim`]) and comes back in line when they change; a red run or a
conflict still waits for a new head. Driven by the loop tests' fake world.
"""

from __future__ import annotations

import tempfile
import unittest
from unittest import mock

from merging.queue import cli, outcomes, pulls, rebase, taking
from merging.verdict import github
from merging.watch import ending, handbacks, loop
from tests.verdict.fixtures import READY
from tests.watch import fakes
from tests.watch.fakes import pr

UNNAMED = "the branch is 1265-x, but the description does not close #1265."


def described(number: int, body: str, head: str = "pushed1") -> dict:
    return {**pr(number, head=head), "body": body}


def run(listings, ends, recalled=None):
    """Take #1 through `ends` in turn over `listings`; what was taken and said."""
    with tempfile.TemporaryDirectory() as tmp:
        world = fakes.World(tmp, listings, {1: outcomes.HANDED_BACK})
        ends, said = iter(ends), []
        world.fx.say = lambda line, *rest: said.append(line)

        def turn(number, tick=None):
            world.taken.append(number)
            world.verdicts[number] = next(ends)  # a merge drops it from the listing
            return number, world.verdicts[number], UNNAMED

        world.fx.turn = turn
        for path, entries in (recalled or {}).items():
            handbacks.remember(world.fx.memory.with_name(path), entries)
        loop.run(world.fx, fakes.opts(idle=5, poll=60))
        return world.taken, said, handbacks.recall(world.fx.memory.with_name(handbacks.CLAIMS_MEMORY))


class Reworded(unittest.TestCase):
    def test_a_missing_closing_keyword_clears_after_a_body_edit_with_no_push(self):
        before, after = described(1, "no keyword"), described(1, "Closes #1265")
        taken, said, _ = run([[before], [before], [after]], [outcomes.RECLAIM, outcomes.MERGED])
        self.assertEqual(taken, [1, 1])
        self.assertIn("watch: #1's description or labels changed since its hand-back; back in line.", said)

    def test_the_same_description_stays_passed_over_and_is_remembered(self):
        same = described(1, "no keyword")
        taken, said, claims = run([[same]], [outcomes.RECLAIM])
        self.assertEqual((taken, claims), ([1], {1: handbacks.claim(same)}))
        self.assertIn(ending.handed_back(1, UNNAMED, edit=True), said)

    def test_a_ci_hand_back_still_waits_for_a_new_head(self):
        before, after = described(1, "Closes #1265"), described(1, "Closes #1265, reworded")
        taken, said, claims = run([[before], [before], [after]], [outcomes.HANDED_BACK])
        self.assertEqual((taken, claims), ([1], {}))
        self.assertIn(ending.handed_back(1, UNNAMED), said)

    def test_a_label_change_counts(self):
        before = described(1, "x")
        after = {**before, "labels": [*before["labels"], {"name": "bug"}]}
        self.assertEqual(handbacks.reworded([after], {1: "pushed1"}, {1: handbacks.claim(before)}), [1])

    def test_a_listing_without_a_body_is_no_news(self):
        self.assertEqual(handbacks.reworded([pr(1)], {1: "pushed1"}, {1: "anything"}), [])

    def test_a_recalled_hand_back_edited_while_no_watch_ran_is_taken(self):
        recalled = {handbacks.MEMORY.split("/")[-1]: {1: "pushed1"}, handbacks.CLAIMS_MEMORY: {1: "stale"}}
        taken, _, _ = run([[described(1, "Closes #1265")]], [outcomes.MERGED], recalled=recalled)
        self.assertEqual(taken, [1])


class Lines(unittest.TestCase):
    def test_each_line_says_which_fix_it_waits_for(self):
        self.assertIn("no push needed", ending.handed_back(1, "why", edit=True))
        self.assertIn("Push a fix", ending.handed_back(1, "why"))


class Taken(unittest.TestCase):
    def take(self, files):
        pull = {**READY, "state": "OPEN", "title": "t", "author": {"login": "x"}, "headRefName": "1265-x",
                "body": "no keyword", "files": files}
        with mock.patch.object(pulls, "look", return_value=pull), mock.patch.object(github, "gh"), \
                mock.patch.object(rebase, "advance") as rebased, mock.patch("builtins.print"):
            _, state, _ = taking.take("o/r", 1, cli.parse(["1"]))
        rebased.assert_not_called()
        return state

    def test_a_description_refusal_is_an_edit_hand_back(self):
        self.assertEqual(self.take([{"path": "README.md"}]), outcomes.RECLAIM)

    def test_an_image_needs_a_push_so_it_waits_for_the_head(self):
        self.assertEqual(self.take([{"path": "shot.png", "status": "added"}]), outcomes.HANDED_BACK)


if __name__ == "__main__":
    unittest.main()
