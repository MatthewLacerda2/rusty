"""#2615: the waiting line drains by the project's priority order, not by ask time alone."""

from __future__ import annotations

import unittest
from datetime import datetime, timedelta, timezone

from dispatch import waiting
from tests.dispatch.coder_fakes import ROUND, Board, brief_on, busy, drafts

START = datetime(2026, 11, 1, tzinfo=timezone.utc)
RED = "<!-- main-health:workflow=ci -->\nThe `ci` workflow went red on `main`."


def issue(number: int, *labels: str, body: str = "") -> dict:
    return {"number": number, "state": "open", "body": body, "labels": [{"name": n} for n in labels]}


def asked(*rows: tuple) -> list[dict]:
    """The line after each (number, issue, brief) row was asked for, an hour apart, in turn."""
    line: list[dict] = []
    for hour, (number, found, said) in enumerate(rows):
        line, _ = waiting.enqueue(line, number, said, START + timedelta(hours=hour), found)
    return line


def order(line: list[dict]) -> list[int]:
    return [e["issue"] for e in waiting.ordered(line)]


class Order(unittest.TestCase):
    def test_a_round_then_red_main_then_docs_then_infrastructure_then_feature_whatever_the_ask_order(self):
        line = asked((10, issue(10, "feature"), brief_on(10)),
                     (11, issue(11, "infrastructure"), brief_on(11)),
                     (14, issue(14, "documentation", "feature"), brief_on(14)),
                     (12, issue(12, "bug", body=RED), brief_on(12)),
                     (13, issue(13, "feature"), brief_on(13, body=ROUND)))
        self.assertEqual(order(line), [13, 12, 14, 11, 10])
        self.assertEqual(waiting.ahead(line, 10), 4)

    def test_red_main_is_told_by_the_marker_not_the_title_or_the_bug_label(self):
        titled = {**issue(9, "bug"), "title": "Red main: the `ci` workflow is failing"}
        self.assertEqual(waiting.priority(titled), "bug")
        self.assertEqual(waiting.priority(issue(9, "bug", body=RED)), waiting.RED_MAIN)

    def test_several_labels_take_the_highest_and_none_is_lowest(self):
        self.assertEqual(waiting.priority(issue(9, "feature", "agent", "architecture")), "architecture")
        self.assertEqual(waiting.priority(issue(9, "agent")), waiting.UNLABELLED)
        self.assertIsNone(waiting.priority(None))

    def test_equal_priority_keeps_the_ask_order(self):
        line = asked((11, issue(11, "bug"), brief_on(11)), (10, issue(10, "bug"), brief_on(10)))
        self.assertEqual(order(line), [11, 10])

    def test_an_entry_without_a_priority_sorts_with_the_lowest_in_ask_order(self):
        line = asked((10, issue(10, "agent"), brief_on(10)), (11, issue(11, "feature"), brief_on(11)))
        old = {"issue": 9, "brief": "u", "round": False, "asked": "2026-11-01T00:30:00Z"}
        self.assertEqual(order([*line, old]), [11, 10, 9])
        self.assertIn("priority unread", waiting.described(old))

    def test_a_second_ask_takes_the_newer_labels_and_keeps_its_ask_time(self):
        line = asked((10, issue(10, "feature"), brief_on(10)), (11, issue(11, "bug"), brief_on(11)))
        line, added = waiting.enqueue(line, 10, brief_on(10), START + timedelta(days=1), issue(10, "infrastructure"))
        self.assertFalse(added)
        self.assertEqual((order(line), line[0]["asked"]), ([10, 11], "2026-11-01T00:00:00Z"))


class Drain(unittest.TestCase):
    def full(self) -> Board:
        fake = Board({}, routines=busy(1, 2, 3, 4, 5), pulls=drafts(1, 2, 3, 4, 5))
        for number, labels in ((9, ["feature"]), (10, ["infrastructure"])):
            fake.open(number, labels=labels)
            fake.run(str(number))
        return fake

    def test_a_freed_slot_launches_the_higher_priority_asked_later(self):
        fake = self.full()
        fake.pulls, fake.routines = drafts(1, 2, 3, 4), busy(1, 2, 3, 4)
        code, out = fake.coders("--dry-run")
        self.assertIn("1. #10 (first launch, infrastructure", out)
        self.assertIn("2. #9 (first launch, feature", out)
        fake.coders()
        self.assertEqual(fake.launched(), [10])

    def test_a_pass_writes_back_a_label_that_changed(self):
        fake = self.full()
        fake.open(10, labels=["feature"])  # relabelled after it was asked for
        fake.pulls, fake.routines, fake.create = drafts(1, 2, 3, 4), busy(1, 2, 3, 4), (500, {"error": "down"})
        fake.coders()  # #10 is read, its create fails, so it stays in line with the label read
        self.assertEqual([(e["issue"], e["priority"]) for e in waiting.load(fake.line)],
                         [(9, "feature"), (10, "feature")])

    def test_a_round_asked_during_a_pass_keeps_its_round_and_goes_first(self):
        fake = self.full()
        fake.pulls, fake.routines, fake.create = drafts(1, 2, 3, 4), busy(1, 2, 3, 4), (500, {"error": "down"})
        read, again = fake.gh, brief_on(10, comment=2, body=ROUND)

        def gh(path: str):  # #10 re-asked on a `Round:` brief right after this pass read it
            got = read(path)
            if "/issues/10/comments" in path and "page=1" in path:
                on_disk = waiting.load(fake.line)
                waiting.save(fake.line, waiting.enqueue(on_disk, 10, again, START, issue(10, "infrastructure"))[0])
            return got

        fake.gh = gh
        fake.coders()
        kept = waiting.load(fake.line)
        self.assertEqual([(e["issue"], e["brief"], e["round"]) for e in waiting.ordered(kept)],
                         [(10, again["html_url"], True), (9, brief_on(9)["html_url"], False)])

if __name__ == "__main__":
    unittest.main()
