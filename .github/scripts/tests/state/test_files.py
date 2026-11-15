#!/usr/bin/env python3
"""#2215: the queue's state files are written whole and read tolerantly.

Each of the three writers under `.git/merge-queue/` goes through
`merging.state.write`, and a file cut short (a power cut mid-write, before
#2215 possible for `handed-back.json`) is read as absent by its own reader,
never as a crash.
"""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from merging import state
from merging.after import slots
from merging.watch import handbacks, kept

HEADS, OPENED = {7: (False, "abc", True)}, {7: "2026-11-15T05:00:00+00:00"}


class Truncated(unittest.TestCase):
    def test_a_file_cut_short_is_read_as_absent_by_each_reader(self):
        writers = {
            "handed-back.json": (lambda p: handbacks.remember(p, {7: "abc"}), handbacks.recall, {}),
            kept.BOARD_MEMORY: (lambda p: kept.remember(p, HEADS, OPENED), kept.recall, None),
            "mutants.json": (lambda p: slots.save(p, {**slots.empty(), "unread": [{"pr": 7}]}), slots.load, slots.empty()),
        }
        for name, (write, read, absent) in writers.items():
            with tempfile.TemporaryDirectory() as tmp:
                path = Path(tmp, "merge-queue", name)
                self.assertIsNone(write(path), name)
                self.assertNotEqual(read(path), absent, name)  # written whole, read back
                whole = path.read_text()
                for cut in (0, 1, len(whole) // 2, len(whole) - 3):
                    path.write_text(whole[:cut])
                    self.assertEqual(read(path), absent, (name, cut))


class Writer(unittest.TestCase):
    def test_it_replaces_whole_and_leaves_no_temporary(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp, "a", "b.json")
            self.assertIsNone(state.write(path, {"x": 1}))
            self.assertIsNone(state.write(path, {"x": 2}))
            self.assertEqual((json.loads(path.read_text()), [p.name for p in path.parent.iterdir()]), ({"x": 2}, ["b.json"]))

    def test_a_cut_write_leaves_one_fixed_temporary_the_next_write_overwrites(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp, "b.json")
            for cut in range(3):  # what a write cut before its rename leaves, three times
                state.part(path).write_text("{" * cut)
            self.assertIsNone(state.write(path, {"x": 3}))
            self.assertEqual((state.read(path), sorted(p.name for p in Path(tmp).iterdir())), ({"x": 3}, ["b.json"]))

    def test_a_write_that_cannot_happen_says_why_and_keeps_the_old_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp, "b.json")
            state.write(path, {"x": 1})
            self.assertIsNotNone(state.write(Path(tmp, "b.json", "c"), 1))  # its parent is a file
            self.assertEqual(state.read(path), {"x": 1})

    def test_missing_or_not_json_reads_none(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertIsNone(state.read(Path(tmp, "none.json")))
            Path(tmp, "bad.json").write_text("{")
            self.assertIsNone(state.read(Path(tmp, "bad.json")))


if __name__ == "__main__":
    unittest.main()
