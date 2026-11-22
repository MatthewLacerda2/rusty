#!/usr/bin/env python3
"""#2628: a review record's verdict line, the head it holds on, and what it still owes.

Must read: the newest trusted record's `Verdict:` line anywhere in it, a
`queue` on an older head followed only by merges, `Needs:` answered by a later
`Done:`. Must not: a verdict outside a record or by someone who cannot push, a
line with more on it, a `queue` on a head with a commit of its own after it, a
bare `No review:` (no head), a `Done:` in the same comment as its `Needs:`.
"""

from __future__ import annotations

import unittest

from merging.verdict import standing

HEAD = "c" * 40


def said(body: str, at: str = "2026-11-22T10:00:00Z", who: str = "OWNER") -> dict:
    return {"body": body, "created_at": at, "author_association": who, "user": {"login": "someone"}}


def record(*lines: str, at: str = "2026-11-22T10:00:00Z", who: str = "OWNER") -> dict:
    return said("\n".join(("Independent review (high)", "No findings.", *lines)), at, who)


class Verdict(unittest.TestCase):
    def test_the_line_anywhere_in_a_record(self):
        self.assertEqual(standing.said([record("Verdict: queue on abc1234")]), ("queue", "abc1234"))
        self.assertEqual(standing.said([said("No review: docs only\n\nVerdict: hold on ABC1234")]), ("hold", "abc1234"))

    def test_the_newest_record_wins_whatever_the_listing_order(self):
        old = record("Verdict: queue on abc1234", at="2026-11-22T09:00:00Z")
        new = record("Verdict: round on def5678", at="2026-11-22T11:00:00Z")
        self.assertEqual(standing.said([new, old]), ("round", "def5678"))

    def test_the_line_is_the_one_its_writer_is_given(self):
        self.assertEqual(standing.said([record(standing.line("queue", HEAD))]), ("queue", HEAD[:7]))

    def test_no_verdict(self):
        for comments in (
            [said("Verdict: queue on abc1234")],  # not a record
            [record("Verdict: queue on abc1234", who="CONTRIBUTOR")],  # cannot push
            [record("Verdict: queue on abc1234 once CI is green")],  # more on the line
            [record("**Verdict: queue on abc1234**")],
            [record("Verdict: queue on abc12")],  # too short a head
            [record("Verdict: merge on abc1234")],
            [said("No review: docs only")],  # a bare waiver names no head
            [record()],
        ):
            with self.subTest(comments=comments):
                self.assertIsNone(standing.said(comments))
                self.assertIsNone(standing.standing(comments)["verdict"])


def commit(sha: str, *parents: str) -> dict:
    return {"sha": sha, "parents": [{"sha": p} for p in parents]}


class MergesOnly(unittest.TestCase):
    """`main...head`'s commits: those on the branch, not on `main` (`m*`)."""

    def test_the_verdicts_own_head(self):
        self.assertTrue(standing.merges_only([], 0, "aaaaaaa", "aaaaaaa" + "0" * 33))

    def test_merges_of_main_after_it(self):
        commits = [commit("a1", "a0"), commit("v", "a1"), commit("x1", "v", "m1"), commit("x2", "x1", "m2")]
        self.assertTrue(standing.merges_only(commits, 4, "v", "x2"))

    def test_a_commit_of_its_own_after_it(self):
        commits = [commit("v", "a0"), commit("x1", "v", "m1"), commit("fix", "x1")]
        self.assertFalse(standing.merges_only(commits, 3, "v", "fix"))

    def test_a_merge_of_another_branch(self):
        commits = [commit("v", "a0"), commit("o", "m1"), commit("x1", "v", "o")]
        self.assertFalse(standing.merges_only(commits, 3, "v", "x1"))

    def test_a_head_the_branch_no_longer_holds(self):
        commits = [commit("w", "a0"), commit("x1", "w", "m1")]
        self.assertFalse(standing.merges_only(commits, 2, "v", "x1"))

    def test_a_list_github_cut_short(self):
        commits = [commit("v", "a0"), commit("x1", "v", "m1")]
        self.assertFalse(standing.merges_only(commits, 300, "v", "x1"))

    def test_a_queue_on_an_unreviewed_head_reads_round(self):
        comments = [record("Verdict: queue on abc1234")]
        self.assertEqual(standing.standing(comments, current=False)["verdict"], "round")
        self.assertEqual(standing.standing(comments, current=True)["verdict"], "queue")
        self.assertEqual(standing.standing([record("Verdict: hold on abc1234")], current=False)["verdict"], "hold")


class Owed(unittest.TestCase):
    def test_a_need_until_a_later_done(self):
        need = record("Verdict: queue on abc1234", "Needs: make test-gpu on the M2", at="2026-11-22T10:00:00Z")
        self.assertEqual(standing.owed([need]), ["make test-gpu on the M2"])
        done = said("Done:   make TEST-GPU on the M2.", at="2026-11-22T12:00:00Z")
        self.assertEqual(standing.owed([done, need]), [])

    def test_a_done_before_or_beside_its_need_answers_nothing(self):
        early = said("Done: desktop frame time", at="2026-11-22T09:00:00Z")
        need = record("Needs: desktop frame time", "Done: desktop frame time", at="2026-11-22T10:00:00Z")
        self.assertEqual(standing.owed([early, need]), ["desktop frame time"])

    def test_only_a_record_asks_and_only_someone_who_can_push_answers(self):
        self.assertEqual(standing.owed([said("Needs: a desktop look")]), [])
        need = record("Needs: a desktop look")
        stranger = said("Done: a desktop look", at="2026-11-22T12:00:00Z", who="NONE")
        self.assertEqual(standing.owed([need, stranger]), ["a desktop look"])


if __name__ == "__main__":
    unittest.main()
