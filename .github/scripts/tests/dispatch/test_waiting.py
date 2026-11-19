"""#2382: `make coder` at the limit puts the issue in the waiting line instead of forgetting it."""

from __future__ import annotations

import unittest
from datetime import datetime, timezone

from dispatch import waiting
from tests.dispatch.coder_fakes import ROUND, Board, brief_on, busy, drafts

FULL = dict(routines=busy(1, 2, 3, 4, 5), pulls=drafts(1, 2, 3, 4, 5))


def board(**kwargs) -> Board:
    found = Board({}, **{**FULL, **kwargs})
    for number in (9, 10, 11):
        found.open(number)
    return found


class AtTheLimit(unittest.TestCase):
    def test_a_launch_at_the_limit_waits_exits_0_and_creates_nothing(self):
        fake = board()
        code, _, err = fake.run("9")
        self.assertEqual((code, fake.created()), (0, []))
        self.assertIn("#9 waiting for a slot, 0 ahead", err)
        self.assertIn("rusty #5 x", err)
        self.assertEqual([e["issue"] for e in waiting.load(fake.line)], [9])

    def test_a_second_ask_does_not_queue_twice_or_move(self):
        fake = board()
        fake.run("9")
        fake.run("10")
        _, _, err = fake.run("9")
        self.assertIn("#9 already waiting for a slot, 0 ahead", err)
        self.assertEqual([e["issue"] for e in waiting.load(fake.line)], [9, 10])

    def test_no_wait_refuses_and_queues_nothing(self):
        fake = board()
        self.assertEqual(fake.run("9", "--no-wait")[0], 1)
        self.assertEqual(waiting.load(fake.line), [])

    def test_a_dry_run_says_where_it_would_wait_and_writes_nothing(self):
        fake = board()
        code, _, err = fake.run("9", "--dry-run")
        self.assertEqual(code, 0)
        self.assertIn("#9 would wait for a slot, 0 ahead", err)
        self.assertFalse(fake.line.exists())

    def test_over_limit_still_launches_now(self):
        fake = board()
        self.assertEqual((fake.run("9", "--over-limit")[0], fake.launched()), (0, [9]))
        self.assertEqual(waiting.load(fake.line), [])

    def test_other_refusals_never_queue(self):
        fake = board()
        fake.open(9, labels=["planning"])
        fake.open(10, comments=[])
        for number in ("9", "10"):
            with self.subTest(number):
                self.assertEqual(fake.run(number)[0], 1)
        self.assertEqual(waiting.load(fake.line), [])

    def test_a_launch_by_hand_takes_the_issue_out_of_the_line(self):
        fake = board()
        fake.run("9")
        fake.run("9", "--over-limit")
        self.assertEqual(waiting.load(fake.line), [])


class Order(unittest.TestCase):
    def test_a_round_goes_ahead_of_a_first_launch_asked_earlier(self):
        early, late = datetime(2026, 11, 1, tzinfo=timezone.utc), datetime(2026, 11, 2, tzinfo=timezone.utc)
        line, _ = waiting.enqueue([], 10, brief_on(10), early)
        line, _ = waiting.enqueue(line, 11, brief_on(11), early.replace(hour=1))
        line, _ = waiting.enqueue(line, 9, brief_on(9, body=ROUND), late)
        self.assertEqual([e["issue"] for e in waiting.ordered(line)], [9, 10, 11])
        self.assertEqual(waiting.ahead(line, 11), 2)

    def test_an_unreadable_file_or_entry_is_an_empty_line(self):
        fake = board()
        fake.line.parent.mkdir(parents=True)
        fake.line.write_text('[{"issue": "x"}, {"issue": 9, "brief": "u", "asked": "t"}')
        self.assertEqual(waiting.load(fake.line), [])
        fake.line.write_text('[{"issue": "x"}, {"issue": 9, "brief": "u", "asked": "t"}]')
        self.assertEqual([e["issue"] for e in waiting.load(fake.line)], [9])


if __name__ == "__main__":
    unittest.main()
