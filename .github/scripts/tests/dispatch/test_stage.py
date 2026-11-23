"""#2629: where a pull request's review loop stands, read from GitHub alone, and the record the shepherd posts.

Must answer `review` with no record, `reread` when a verdict's head has commits of its own since,
and `stop` for a closed, draft or held pull request, a `hold` verdict, a verdict that holds for the
head, a record with no verdict line, an unread compare and a non-code pull request; must count the
`round` verdicts. The record must end in the verdict line the queue reads, and refuse what it would not.
"""

from __future__ import annotations

import unittest

from dispatch import stage
from merging.verdict import reviewed, standing

HEAD = "c" * 40


def pull(**over) -> dict:
    return {"number": 7, "state": "OPEN", "isDraft": False, "headRefOid": HEAD, "labels": [],
            "files": [{"path": "src/a.rs"}], **over}


def note(body: str, who: str = "OWNER", at: str = "2026-11-20T10:00:00Z") -> dict:
    return {"body": body, "author_association": who, "created_at": at}


def verdict(word: str, at: str = "2026-11-20T10:00:00Z") -> dict:
    return note(f"Independent review\n\nNo findings.\n\nVerdict: {word} on abc1234", at=at)


class Where(unittest.TestCase):
    def test_no_record_is_a_review(self):
        self.assertEqual(stage.where(pull(), [note("Thanks!", "NONE")], None).what, stage.REVIEW)

    def test_a_record_by_someone_who_cannot_push_is_none(self):
        found = stage.where(pull(), [note("Independent review\nVerdict: queue on abc1234", "NONE")], None)
        self.assertEqual(found.what, stage.REVIEW)

    def test_a_round_that_came_back_is_a_reread_and_counts(self):
        found = stage.where(pull(), [verdict("round")], current=False)
        self.assertEqual((found.what, found.rounds), (stage.REREAD, 1))

    def test_a_queue_with_new_code_since_is_a_reread(self):
        self.assertEqual(stage.where(pull(), [verdict("queue")], current=False).what, stage.REREAD)

    def test_a_verdict_on_this_head_already_stops(self):
        for word in ("queue", "round"):
            found = stage.where(pull(), [verdict(word)], current=True)
            self.assertEqual(found.what, stage.STOP, word)
            self.assertIn("holds for", found.why)

    def test_a_hold_stops_whatever_followed(self):
        found = stage.where(pull(), [verdict("hold")], current=False)
        self.assertEqual(found.what, stage.STOP)
        self.assertIn("decision for the user", found.why)

    def test_an_unread_compare_stops(self):
        self.assertIn("did not show", stage.where(pull(), [verdict("round")], None).why)

    def test_a_newest_record_without_a_verdict_stops(self):
        found = stage.where(pull(), [verdict("queue"), note("Independent review\nlooked by hand",
                                                            at="2026-11-21T10:00:00Z")], None)
        self.assertEqual(found.what, stage.STOP)
        self.assertIn("by hand", found.why)

    def test_closed_draft_held_and_inert_stop(self):
        for shaped, why in ((pull(state="MERGED"), "merged"), (pull(isDraft=True), "draft"),
                            (pull(labels=[{"name": "hold"}]), "`hold` label"),
                            (pull(files=[{"path": "README.md"}]), "no code")):
            found = stage.where(shaped, [], None)
            self.assertEqual(found.what, stage.STOP, why)
            self.assertIn(why, found.why)

    def test_rounds_count_only_round_verdicts(self):
        comments = [verdict("round", "2026-11-20T10:00:00Z"), verdict("round", "2026-11-21T10:00:00Z"),
                    verdict("queue", "2026-11-22T10:00:00Z")]
        self.assertEqual(stage.rounds(comments), 2)


class Record(unittest.TestCase):
    URL = "https://github.com/o/r/issues/9#issuecomment-501"

    def test_it_ends_in_the_verdict_line_the_queue_reads(self):
        made = stage.record("Independent review (medium)\n\nNo findings.\n", "queue", HEAD)
        self.assertTrue(reviewed.decides(made))
        self.assertEqual(standing.said([note(made)]), ("queue", HEAD[:7]))
        self.assertTrue(made.endswith(f"Verdict: queue on {HEAD[:7]}\n"))

    def test_a_round_names_its_brief_on_its_own_line(self):
        made = stage.record("Independent review\n\n- a.rs `f`: wrong", "round", HEAD, self.URL)
        self.assertEqual(stage.ROUND_BRIEF.search(made).group(2), "9")
        self.assertEqual(standing.said([note(made)]), ("round", HEAD[:7]))

    def test_a_waiver_is_a_record(self):
        made = stage.record("No review: documentation only", "queue", HEAD)
        self.assertEqual(standing.said([note(made)]), ("queue", HEAD[:7]))

    def test_it_refuses_what_the_queue_would_not_read(self):
        for text, word, brief in (("Looks good", "queue", None), ("Independent review", "merge", None),
                                  ("Independent review", "round", None), ("Independent review", "queue", self.URL),
                                  ("Independent review\nVerdict: queue on abc1234", "queue", None),
                                  ("Independent review", "round", "https://example.com/x")):
            with self.assertRaises(ValueError, msg=(text, word, brief)):
                stage.record(text, word, HEAD, brief)


if __name__ == "__main__":
    unittest.main()
