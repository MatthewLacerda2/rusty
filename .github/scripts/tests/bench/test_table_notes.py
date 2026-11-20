#!/usr/bin/env python3
"""#2420: each leg's `bench-scenes` warnings head the scene's section, once, and a note not every leg wrote says so."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from bench import table

UNBAKED = "not baked: lightmaps (43 of 43 pages not on disk) (its lighting cost is not the shipped one)"


def leg(root: Path, name: str, warnings: list[str], scene="levels/a") -> Path:
    path = root / name / "yard-0123abcd" / f"{scene}.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    report = {"scene": scene, "rows": [{"metric": "gpu_ms", "avg": 4.0}], "warnings": warnings}
    path.write_text(json.dumps(report))
    return root / name


class Notes(unittest.TestCase):
    def section(self, per_leg: list[list[str]]) -> str:
        root = Path(tempfile.mkdtemp())
        legs = tuple(leg(root, n, w) for n, w in zip(("main", "pr", "main-again"), per_leg))
        return table.scene_table("levels/a", legs)

    def test_a_note_every_leg_wrote_heads_the_section_once(self):
        text = self.section([[UNBAKED]] * 3)
        head = text.split("Nothing moved")[0]
        self.assertIn(f"- {UNBAKED}\n", head, "above the lead")
        self.assertEqual(text.count("not baked"), 1)
        self.assertNotIn("disagree", text)

    def test_a_note_only_some_legs_wrote_names_them(self):
        text = self.section([[UNBAKED], [], [UNBAKED]])
        self.assertIn("(only main, main again: the legs disagree)", text)

    def test_a_path_inside_a_leg_reads_the_same_in_every_leg(self):
        root = Path(tempfile.mkdtemp())
        legs = []
        for n in ("main", "pr", "main-again"):
            kept = root / n / "yard-0123abcd" / "levels/a.console.log"
            legs.append(leg(root, n, [f"1 console error during the bench (first: x); the console is in {kept}"]))
        text = table.scene_table("levels/a", tuple(legs))
        self.assertIn("the console is in <leg>/yard-0123abcd/levels/a.console.log\n", text)
        self.assertNotIn("disagree", text)

    def test_a_scene_with_no_warnings_has_no_notes(self):
        text = self.section([[], [], []])
        self.assertNotIn("Warnings", text)
        self.assertTrue(text.startswith("**levels/a**\n\nNothing moved"))


class NoFloorLead(unittest.TestCase):
    def test_with_no_floor_the_lead_lists_the_rows_largest_change_first(self):
        main, pr = {"cpu_ms": 4.0, "gpu_ms": 2.0, "draws": 0.0}, {"cpu_ms": 4.4, "gpu_ms": 1.0, "draws": 3.0}
        lines = table.lead(main, pr, {})
        self.assertTrue(lines[0].startswith("No floor"))
        self.assertEqual([line.split("`")[1] for line in lines[2:]], ["draws", "gpu_ms", "cpu_ms"],
                         "from zero first, then -50%, then +10%")


if __name__ == "__main__":
    unittest.main()
