#!/usr/bin/env python3
"""#1723: the record of every conflict the queue meets, and the totals it prints."""

from __future__ import annotations

import tempfile
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path

from merging.queue.conflict import record

NOW = datetime(2026, 11, 2, 12, tzinfo=timezone.utc)


class Record(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.file = Path(tmp.name, "merge-queue", "conflicts.jsonl")

    def test_a_pair_is_written_once_and_a_bad_line_is_skipped(self):
        self.assertTrue(record.note(self.file, 1, "h", "m", {"a.rs": True}, record.RESOLVED, NOW))
        self.assertFalse(record.note(self.file, 1, "h", "m", {"a.rs": True}, record.RESOLVED, NOW))
        with self.file.open("a") as out:
            out.write("not json\n")
        self.assertTrue(record.note(self.file, 1, "h", "m2", {"a.rs": False}, record.HANDED_BACK, NOW))
        self.assertEqual([e["main"] for e in record.read(self.file)], ["m", "m2"])

    def test_a_record_that_is_not_text_reads_as_none(self):
        self.file.parent.mkdir(parents=True)
        self.file.write_bytes(b"\xff\xfe not utf-8\n")
        self.assertEqual(record.read(self.file), [])

    def test_an_unwritable_record_is_no_line_never_a_stop(self):
        self.file.parent.parent.joinpath("merge-queue").write_text("a file where the folder goes")
        self.assertFalse(record.note(self.file, 1, "h", "m", {}, record.RESOLVED, NOW))
        self.assertEqual(record.read(self.file), [])

    def test_totals(self):
        self.assertEqual(record.totals([]), ["conflicts with `main`: none recorded."])
        entries = [
            {"pull": 1, "paths": {"a": True, "b": True}, "ended": record.RESOLVED},
            {"pull": 1, "paths": {"a": True, "c": False}, "ended": record.HANDED_BACK},
            {"pull": 2, "paths": {"d": False}, "ended": record.HANDED_BACK},
        ]
        first, second = record.totals(entries)
        self.assertIn("3 on 2 pull request(s); 1 resolved by the sorted-list rule, 2 handed back", first)
        self.assertIn("1 of 3 had every conflicted file inside a `sorted` list; 3 of 5", second)

    def test_since_keeps_what_was_written_at_or_after(self):
        for hours in (3, 1):
            record.note(self.file, hours, f"h{hours}", "m", {}, record.RESOLVED, NOW - timedelta(hours=hours))
        kept = record.since(record.read(self.file), record.when("2h", NOW))
        self.assertEqual([e["pull"] for e in kept], [1])
        self.assertEqual(record.when("2026-11-02", NOW), datetime(2026, 11, 2, tzinfo=timezone.utc))
        self.assertEqual(record.when("2026-11-02T10:00Z", NOW), NOW - timedelta(hours=2))
        with self.assertRaises(ValueError):
            record.when("yesterday", NOW)


if __name__ == "__main__":
    unittest.main()
