#!/usr/bin/env python3
"""The quiet counters: a row only when main and the PR differ, the rest counted."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from bench import table


def leg(root: Path, name: str, rows: dict[str, tuple[float, bool]]) -> Path:
    path = root / name / "yard-0123abcd" / "levels/a.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    out = [{"metric": m, "avg": v, **({"quiet": True} if q else {})} for m, (v, q) in rows.items()]
    path.write_text(json.dumps({"scene": "levels/a", "rows": out}))
    return root / name


class Quiet(unittest.TestCase):
    def setUp(self):
        root = Path(tempfile.mkdtemp())
        main = {"draw_calls": (5.0, False), "ui_meshed": (1.0, True), "entities": (40.0, True),
                "bones": (3.0, True)}
        pr = {"draw_calls": (5.0, False), "ui_meshed": (0.25, True), "entities": (40.0, True),
              "bones": (3.0, True), "ui_laid_out": (0.5, True)}
        self.text = table.scene_table("levels/a", (leg(root, "main", main), leg(root, "pr", pr),
                                                   leg(root, "again", main)))
        cells = [line.strip("|").split(" | ") for line in self.text.splitlines() if line.startswith("| ")]
        self.rows = {c[0].strip(): [x.strip() for x in c[1:]] for c in cells[1:]}

    def test_a_quiet_counter_that_moved_prints(self):
        self.assertEqual(self.rows["ui_meshed"][:2], ["1", "0.25"])

    def test_identical_quiet_counters_are_counted_not_printed(self):
        self.assertNotIn("entities", self.rows)
        self.assertNotIn("bones", self.rows)
        self.assertIn("The other 2 counters are identical between main and the PR", self.text)

    def test_a_quiet_counter_absent_on_main_prints_so(self):
        self.assertEqual(self.rows["ui_laid_out"][:3], ["absent", "0.5", "absent on main"])

    def test_a_shown_counter_prints_even_unchanged(self):
        self.assertIn("draw_calls", self.rows)


if __name__ == "__main__":
    unittest.main()
