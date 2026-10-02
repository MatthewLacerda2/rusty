#!/usr/bin/env python3
"""#664: which ready pull request `--watch` takes, and when it has nothing left.

Pure functions over `gh`-shaped dictionaries: CLAUDE.md's label priority read
from the issues a pull request closes, Dependabot last, drafts and remembered
hand-backs passed over, and the idle exit.
"""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent
_spec = importlib.util.spec_from_file_location("queue_watch_select", HERE / "queue_watch.py")
watch = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(watch)

BOTS = ("app/dependabot",)


def pr(number: int, *, draft=False, closes=(), labels=(), bot=False, head=None, base="main") -> dict:
    return {
        "number": number, "isDraft": draft, "headRefOid": head or f"sha{number}", "baseRefName": base,
        "author": {"login": "app/dependabot" if bot else "someone"},
        "labels": [{"name": n} for n in labels],
        "closingIssuesReferences": [{"number": n} for n in closes],
    }


def pick(pulls, issues=None, memory=None, done=()):
    chosen = watch.pick(pulls, issues or {}, memory or {}, set(done), BOTS)
    return chosen and chosen["number"]


class Priority(unittest.TestCase):
    ISSUES = {1: {"feature"}, 2: {"bug"}, 3: {"infrastructure"}, 4: {"architecture"}, 5: {"foundation"}}

    def test_label_order_comes_from_the_closed_issues(self):
        pulls = [pr(10, closes=[1]), pr(11, closes=[5]), pr(12, closes=[2]), pr(13, closes=[4]), pr(14, closes=[3])]
        taken = []
        while pulls:
            n = pick(pulls, self.ISSUES)
            taken.append(n)
            pulls = [p for p in pulls if p["number"] != n]
        self.assertEqual(taken, [14, 13, 12, 11, 10])

    def test_the_pull_requests_own_labels_count_too(self):
        self.assertEqual(pick([pr(20, closes=[1]), pr(21, labels=["bug"])], self.ISSUES), 21)

    def test_the_best_label_of_several_issues_wins(self):
        self.assertEqual(pick([pr(30, closes=[2]), pr(31, closes=[1, 3])], self.ISSUES), 31)

    def test_unlabelled_goes_after_feature_and_ties_go_oldest_first(self):
        self.assertEqual(pick([pr(41), pr(42, closes=[1])], self.ISSUES), 42)
        self.assertEqual(pick([pr(44), pr(43)], self.ISSUES), 43)

    def test_dependabot_goes_after_every_other(self):
        self.assertEqual(pick([pr(5, bot=True, labels=["infrastructure"]), pr(50)], self.ISSUES), 50)
        self.assertEqual(pick([pr(5, bot=True)], self.ISSUES), 5)


class PassedOver(unittest.TestCase):
    def test_drafts_are_never_taken(self):
        self.assertIsNone(pick([pr(1, draft=True)]))
        self.assertEqual(pick([pr(1, draft=True, labels=["infrastructure"]), pr(2)]), 2)

    def test_a_base_other_than_main_is_never_taken(self):
        self.assertIsNone(pick([pr(1, base="other")]))

    def test_a_remembered_hand_back_waits_for_its_head_to_move(self):
        self.assertIsNone(pick([pr(1, head="a")], memory={1: "a"}))
        self.assertEqual(pick([pr(1, head="b")], memory={1: "a"}), 1)

    def test_one_this_run_already_ended_is_not_taken_again(self):
        self.assertIsNone(pick([pr(1)], done=[1]))

    def test_memory_forgets_what_is_no_longer_open(self):
        self.assertEqual(watch.forget_closed({1: "a", 2: "b"}, [pr(2)]), {2: "b"})

    def test_memory_round_trips_and_a_bad_file_is_empty(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp, watch.MEMORY)
            self.assertEqual(watch.recall(path), {})
            watch.remember(path, {7: "abc"})
            self.assertEqual(watch.recall(path), {7: "abc"})
            path.write_text("not json")
            self.assertEqual(watch.recall(path), {})


class Finished(unittest.TestCase):
    IDLE = watch.IDLE_MINUTES * 60

    def test_no_open_pull_request_is_finished_at_once(self):
        self.assertIn("no open pull request", watch.finished([], 0, self.IDLE))

    def test_drafts_keep_it_watching_until_idle(self):
        drafts = [pr(1, draft=True), pr(2, draft=True)]
        self.assertIsNone(watch.finished(drafts, self.IDLE - 1, self.IDLE))
        self.assertIn("#1, #2", watch.finished(drafts, self.IDLE, self.IDLE))

    def test_a_push_or_a_flip_is_movement(self):
        before = watch.heads([pr(1, draft=True, head="a")])
        self.assertNotEqual(before, watch.heads([pr(1, draft=True, head="b")]))
        self.assertNotEqual(before, watch.heads([pr(1, draft=False, head="a")]))
        self.assertEqual(before, watch.heads([pr(1, draft=True, head="a")]))


if __name__ == "__main__":
    unittest.main()
