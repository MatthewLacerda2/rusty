#!/usr/bin/env python3
"""#2469: a scene's section says once, at its top, when `main`'s and the PR's editor rounds
drew the Scene view at other sizes; the same size, or a report from before the rows, says nothing."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from bench import table

SAID = "the Scene view is 1416x963 against 1306x963 on main"


def leg(root: Path, name: str, view: tuple[int, int] | None) -> Path:
    path = root / name / "yard-0123abcd" / "levels/a.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    rows = [{"metric": "editor.gpu_ms", "avg": 4.0}]
    if view is not None:
        rows += [{"metric": "editor.view_w", "avg": view[0]}, {"metric": "editor.view_h", "avg": view[1]}]
    path.write_text(json.dumps({"scene": "levels/a", "rows": rows, "warnings": []}))
    return root / name


class Views(unittest.TestCase):
    def section(self, main, pr, again=None) -> str:
        root = Path(tempfile.mkdtemp())
        legs = (leg(root, "main", main), leg(root, "pr", pr), leg(root, "main-again", again or main))
        return table.scene_table("levels/a", legs)

    def test_views_of_other_sizes_are_said_once_above_the_lead(self):
        text = self.section((1306, 963), (1416, 963))
        self.assertEqual(text.count(SAID), 1, text)
        head = text.split("Outside the floor")[0]
        self.assertIn(SAID, head, "above the lead")
        self.assertIn("not comparable pixel for pixel", text)
        self.assertIn("| editor.view_w | 1306 | 1416 |", text, "the rows are in the table too")

    def test_views_of_one_size_say_nothing(self):
        text = self.section((1306, 963), (1306, 963))
        self.assertNotIn("Scene view is", text)
        self.assertTrue(text.startswith("**levels/a**\n\nNothing moved"), text)

    def test_a_report_from_before_the_rows_reads_as_absent_with_no_line(self):
        self.assertNotIn("Scene view is", self.section(None, (1416, 963)))
        self.assertNotIn("Scene view is", self.section((1306, 963), None))


if __name__ == "__main__":
    unittest.main()
