"""#2017, #2630: a ready pull request holds its coder's slot until it has the go-ahead.

On 2026-11-10 the slot freed when #2009 and #2015 turned ready went to new
issues, and each review then sent its PR back with no slot for the re-brief.
"""

from __future__ import annotations

import unittest

from dispatch import running
from tests.dispatch.test_running import FIRED, WAITING, hub, note, placed, pr

AT = "2026-11-07T05:00:00Z"


def thread(*comments: dict) -> dict:
    """Comments on `pr(2, …)`'s conversation."""
    return {102: list(comments)}


class InReview(unittest.TestCase):
    def test_a_ready_pull_request_with_no_record_holds_a_slot(self):
        self.assertEqual(running.state(FIRED, hub([pr(2, False)])), ("running", running.REVIEWING))
        chat = thread(note("Looks fine to me", AT), note("Independent review pending", AT))
        self.assertEqual(placed(FIRED, hub([pr(2, False)], chat)), "running")

    def test_a_queue_verdict_or_the_queue_label_frees_it(self):
        for body in ("## Independent review (high)\n\nVerdict: queue on abc1234", "**No review:** data only\n"
                     "Verdict: queue on abc1234"):
            with self.subTest(body):
                self.assertEqual(placed(FIRED, hub([pr(2, False)], thread(note(body, AT)))), "ended")
        labelled = {**pr(2, False), "labels": [{"name": "queue"}]}
        self.assertEqual(placed(FIRED, hub([labelled])), "ended")

    def test_a_record_that_sends_it_back_keeps_the_slot_for_its_round(self):
        for body in ("## Independent review (high)\n\nMust fix: x", "Independent review\nVerdict: round on abc1234",
                     "**No review:** data only"):
            with self.subTest(body):
                self.assertEqual(running.state(FIRED, hub([pr(2, False)], thread(note(body, AT)))),
                                 ("running", running.REVIEWING))
        later = thread(note("Independent review\nVerdict: queue on abc1234", AT),
                       note("Independent review (round 2)\nVerdict: round on def5678", "2026-11-07T06:00:00Z"))
        self.assertEqual(placed(FIRED, hub([pr(2, False)], later)), "running", "the newest record decides")

    def test_a_record_by_someone_who_cannot_push_does_not_count(self):
        stranger = thread(note("Independent review\nVerdict: queue on abc1234", AT, who="NONE"))
        self.assertEqual(placed(FIRED, hub([pr(2, False)], stranger)), "running")

    def test_a_record_on_the_issue_is_not_one_on_the_pull_request(self):
        self.assertEqual(placed(FIRED, hub([pr(2, False)], {2: [note("Independent review\nVerdict: queue on abc1234", AT)]})), "running")

    def test_comments_that_cannot_be_read_are_unknown(self):
        blind = running.GitHub(pulls=[pr(2, False)], blockers=lambda n: [], comments=lambda n: None, issue=lambda n: {"state": "open"},
                               now=running.when(AT), closed=lambda n: [])
        self.assertEqual(placed(FIRED, blind), "unknown")

    def test_a_rebrief_of_the_issue_in_review_is_not_counted_against_itself(self):
        counted, _ = running.coders([WAITING, FIRED], 2, hub([pr(2, False)]))
        self.assertEqual(counted, [(WAITING, "not fired")])

    def test_the_refusal_names_it_in_review(self):
        self.assertIn(", in review)", running.describe(FIRED, running.REVIEWING))
        self.assertNotIn("in review", running.describe(FIRED, "draft pull request"))


if __name__ == "__main__":
    unittest.main()
