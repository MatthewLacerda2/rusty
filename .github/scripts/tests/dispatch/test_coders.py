"""#2382: `make coders` prints who runs and who waits, and launches from the line as slots free."""

from __future__ import annotations

import unittest

from dispatch import waiting
from tests.dispatch.coder_fakes import ROUND, Board, brief_on, busy, drafts


def queued(*issues: int, rounds=()) -> Board:
    """A full board where `issues` asked in turn (`rounds` with a `Round:` brief), then a slot freed."""
    fake = Board({}, routines=busy(1, 2, 3, 4, 5), pulls=drafts(1, 2, 3, 4, 5))
    for number in issues:
        fake.open(number, comments=[brief_on(number, body=ROUND if number in rounds else "Brief\n\nx")])
        fake.run(str(number))
    return fake


def free(fake: Board, slots: int) -> None:
    fake.pulls = drafts(*range(1, 6 - slots))
    fake.routines = busy(*range(1, 6 - slots))


class Launches(unittest.TestCase):
    def test_a_freed_slot_launches_the_first_in_line(self):
        fake = queued(9, 10)
        free(fake, 1)
        code, out = fake.coders()
        self.assertEqual((code, fake.launched()), (0, [9]))
        self.assertIn("running 4 of 5", out)
        self.assertEqual([e["issue"] for e in waiting.load(fake.line)], [10])

    def test_a_round_launches_before_a_first_launch_queued_earlier(self):
        fake = queued(10, 9, rounds=(9,))
        free(fake, 1)
        fake.coders()
        self.assertEqual(fake.launched(), [9])

    def test_two_slots_launch_two_and_never_pass_the_limit(self):
        fake = queued(9, 10, 11)
        free(fake, 2)
        fake.coders()
        self.assertEqual(fake.launched(), [9, 10])

    def test_at_the_limit_nothing_launches_and_the_line_is_said(self):
        fake = queued(9)
        code, out = fake.coders()
        self.assertEqual((code, fake.created()), (0, []))
        self.assertIn("running 5 of 5", out)
        self.assertIn("1. #9 (first launch", out)

    def test_a_dry_run_launches_and_writes_nothing(self):
        fake = queued(9)
        free(fake, 1)
        code, out = fake.coders("--dry-run")
        self.assertEqual((code, fake.created()), (0, []))
        self.assertIn("would launch #9", out)
        self.assertEqual([e["issue"] for e in waiting.load(fake.line)], [9])


class Stale(unittest.TestCase):
    def test_a_closed_or_stage_labelled_issue_is_dropped_and_said(self):
        fake = queued(9, 10)
        fake.open(9, state="closed")
        fake.open(10, labels=["human"])
        free(fake, 2)
        _, out = fake.coders()
        self.assertEqual((fake.created(), waiting.load(fake.line)), ([], []))
        self.assertIn("#9: dropped from the line: #9 is closed", out)
        self.assertIn("#10: dropped from the line", out)

    def test_a_newer_brief_launches_instead_once(self):
        fake = queued(9)
        newer = brief_on(9, at="2026-10-29T12:00:00Z", comment=2)
        fake.open(9, comments=[brief_on(9), newer])
        free(fake, 1)
        _, out = fake.coders()
        self.assertIn("a newer brief since it was queued", out)
        [prompt] = fake.briefs()
        self.assertIn(newer["html_url"], prompt)

    def test_an_unreadable_issue_keeps_its_place(self):
        fake = queued(9)
        del fake.issues[9]
        fake.issue = None
        free(fake, 1)
        _, out = fake.coders()
        self.assertEqual(fake.created(), [])
        self.assertIn("kept in line", out)
        self.assertEqual([e["issue"] for e in waiting.load(fake.line)], [9])


if __name__ == "__main__":
    unittest.main()
