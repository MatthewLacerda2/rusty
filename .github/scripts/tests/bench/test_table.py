#!/usr/bin/env python3
"""#2381: the floor column from legs one and three, a row inside it read as inside the floor; the lead; the load counts."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from bench import table


def leg(root: Path, name: str, rows: dict[str, float], scene="levels/a") -> Path:
    path = root / name / "yard-0123abcd" / f"{scene}.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({"scene": scene, "rows": [{"metric": m, "avg": v} for m, v in rows.items()]}))
    return root / name


class Table(unittest.TestCase):
    def setUp(self):
        root = Path(tempfile.mkdtemp())
        self.legs = (
            leg(root, "main", {"cpu_ms": 10.0, "gpu_ms": 4.0, "draw_calls": 0.0, "hiz_tested": 0.0}),
            leg(root, "pr", {"cpu_ms": 11.0, "gpu_ms": 3.0, "draw_calls": 0.0, "hiz_tested": 5.0}),
            leg(root, "main-again", {"cpu_ms": 11.5, "gpu_ms": 4.01, "draw_calls": 0.0, "hiz_tested": 0.0}),
        )

    def rows(self) -> dict[str, list[str]]:
        text = table.scene_table("levels/a", self.legs)
        cells = [line.strip("|").split(" | ") for line in text.splitlines() if line.startswith("| ")]
        return {c[0].strip(): [x.strip() for x in c[1:]] for c in cells[1:]}

    def test_a_change_inside_the_floor_says_so_beside_its_number(self):
        cpu = self.rows()["cpu_ms"]
        self.assertEqual(cpu[2], "inside the floor (+10.0%)")
        self.assertEqual(cpu[3], "+15.0%", "the floor: main again against main")

    def test_a_change_past_its_own_floor_is_shown(self):
        gpu = self.rows()["gpu_ms"]
        self.assertEqual(gpu[2], "-25.0%")
        self.assertEqual(gpu[3], "+0.2%")

    def test_a_row_from_zero_shows_both_values(self):
        rows = self.rows()
        self.assertEqual(rows["hiz_tested"][2], "0 → 5")
        self.assertEqual(rows["draw_calls"][2], "inside the floor (+0.0%)")

    def test_a_row_only_the_pr_has_is_listed_absent_on_main(self):
        root = Path(tempfile.mkdtemp())
        legs = (leg(root, "main", {"cpu_ms": 10.0}), leg(root, "pr", {"cpu_ms": 10.0, "render.hiz": 0.4}),
                leg(root, "main-again", {"cpu_ms": 10.0}))
        rows = self.rows_of(legs)
        self.assertEqual(rows["render.hiz"][:3], ["absent", "0.4", "absent on main"])

    def test_a_row_without_a_floor_shows_its_change(self):
        root = Path(tempfile.mkdtemp())
        legs = (leg(root, "main", {"gpu_ms": 4.0}), leg(root, "pr", {"gpu_ms": 2.0}), leg(root, "main-again", {}))
        gpu = self.rows_of(legs)["gpu_ms"]
        self.assertEqual(gpu[2], "-50.0% (floor unknown)")
        self.assertIn("unknown", gpu[3])

    def rows_of(self, legs):
        self.legs = legs
        return self.rows()

    def test_the_lead_lists_moves_outside_the_floor_largest_first_above_its_size(self):
        root = Path(tempfile.mkdtemp())
        legs = (leg(root, "main", {"cpu_ms": 10.0, "gpu_ms": 4.0, "tiny_ms": 0.001, "draw_calls": 0.0, "hiz": 2.0}),
                leg(root, "pr", {"cpu_ms": 11.0, "gpu_ms": 3.0, "tiny_ms": 0.002, "draw_calls": 3.0, "hiz": 3.0}),
                leg(root, "main-again", {"cpu_ms": 11.5, "gpu_ms": 4.01, "tiny_ms": 0.001, "draw_calls": 0.0,
                                         "hiz": 2.0}))
        text = table.scene_table("levels/a", legs)
        lead = text.split("| row |")[0].splitlines()
        moved = [line.split("`")[1] for line in lead if line.startswith("- ")]
        self.assertEqual(moved, ["draw_calls", "hiz", "gpu_ms"], "from zero first, then by size; "
                         "cpu_ms inside its floor, tiny_ms below the size")
        self.assertIn("0 → 3", text.split("\n- `draw_calls`")[1].splitlines()[0])
        self.assertIn("| cpu_ms |", text, "the full table is kept below")

    def test_the_lead_says_when_nothing_moved(self):
        root = Path(tempfile.mkdtemp())
        legs = tuple(leg(root, n, {"cpu_ms": 10.0}) for n in ("main", "pr", "main-again"))
        self.assertIn("Nothing moved outside the floor", table.scene_table("levels/a", legs))

    def test_load_counts_that_differ_between_legs_are_said(self):
        root = Path(tempfile.mkdtemp())
        counts = {"load.count.models_parsed": 3.0, "load_ms": 900.0}
        legs = (leg(root, "main", counts), leg(root, "pr", {**counts, "load.count.models_parsed": 0.0}),
                leg(root, "main-again", counts))
        said = table.count_changes(["levels/a"], legs)
        self.assertEqual(len(said), 1)
        self.assertIn("load.count.models_parsed main 3, PR 0, main again 3", said[0])
        self.assertEqual(table.count_changes(["levels/a"], (legs[0], legs[2], legs[2])), [])

    def test_scenes_and_a_missing_report(self):
        self.assertEqual(table.scenes_of_all(self.legs[:1]), ["levels/a"])
        self.assertIn("wrote a report", table.scene_table("other", self.legs))
        text = table.render(["levels/a"], self.legs, ["header line"])
        self.assertTrue(text.startswith("header line\n"))


if __name__ == "__main__":
    unittest.main()
