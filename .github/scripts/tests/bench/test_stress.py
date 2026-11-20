#!/usr/bin/env python3
"""#2450: the stress yard's three legs, their runs and the table's stress section, with a stub runner."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from bench import stress, table

MAIN, PR = Path("/t/bench-main"), Path("/t/pr-7-head")


def plan(root: Path, runs=None):
    return stress.plan((MAIN, MAIN / "target/release/play"), (PR, PR / "target/release/play"), root, runs)


def write(out: Path, rows: dict[str, float], warnings=()) -> None:
    out.mkdir(parents=True, exist_ok=True)
    metrics = [[m, {"avg": v, "p95": v}] for m, v in rows.items()]
    (out / "bench.json").write_text(json.dumps({"frames": 600, "metrics": metrics, "warnings": list(warnings)}))


class Plan(unittest.TestCase):
    def test_three_legs_of_runs_each_its_own_trees_yard_into_its_own_folder(self):
        root = Path(tempfile.mkdtemp())
        legs = plan(root)
        self.assertEqual([leg.name for leg in legs], ["main", "pr", "main-again"])
        self.assertEqual(legs[1].runs, tuple(root / "pr" / f"stress-{i}" for i in range(stress.RUNS)))
        argv = stress.command(legs[1], legs[1].runs[0])
        self.assertEqual(argv, [str(PR / "target/release/play"), "--project", str(PR / "tests/fixtures/project"),
                                str(PR / "tests/fixtures/project/scenarios/bench/bench.lua"), str(root / "pr/stress-0")])
        self.assertEqual(legs[2].tree, MAIN, "main again runs main's yard")
        self.assertEqual(len(plan(root, 5)[0].runs), 5)

    def test_every_run_in_order_in_the_scope_each_folder_emptied_first(self):
        root = Path(tempfile.mkdtemp())
        legs = plan(root, 2)
        write(legs[0].runs[0], {"stale": 1.0})
        ran, asked = [], []
        done = stress.run(legs, ["cap", "--"], lambda argv: ran.append(argv) or 0, lambda leg: asked.append(leg.name))
        self.assertEqual([(leg.name, code) for leg, code in done], [("main", 0), ("pr", 0), ("main-again", 0)])
        self.assertEqual([argv[-1] for argv in ran], [str(out) for leg in legs for out in leg.runs])
        self.assertTrue(all(argv[:2] == ["cap", "--"] for argv in ran))
        self.assertEqual(asked, ["main", "main", "pr", "pr", "main-again", "main-again"], "the busy check every run")
        self.assertFalse((legs[0].runs[0] / "bench.json").exists(), "no older run is read")

    def test_a_failed_run_stops_the_legs_and_a_refusal_before_one_raises(self):
        legs = plan(Path(tempfile.mkdtemp()), 2)
        codes = iter([0, 0, 0, 3])
        done = stress.run(legs, [], lambda argv: next(codes))
        self.assertEqual([(leg.name, code) for leg, code in done], [("main", 0), ("pr", 3)])
        with self.assertRaisesRegex(RuntimeError, "stress pr leg: cargo"):
            stress.run(legs, [], lambda argv: 0, lambda leg: "cargo (PID 9)" if leg.name == "pr" else None)


class Section(unittest.TestCase):
    def rows(self, text: str) -> dict[str, list[str]]:
        cells = [line.strip("|").split(" | ") for line in text.splitlines() if line.startswith("| ")]
        return {c[0].strip(): [x.strip() for x in c[1:]] for c in cells[1:]}

    def test_the_runs_are_averaged_with_the_change_the_floor_and_the_absent_and_zero_rows(self):
        legs = plan(Path(tempfile.mkdtemp()), 2)
        values = {"main": ({"cpu.nav_ms": 2.0, "gpu_ms": 4.0, "hiz": 0.0}, {"cpu.nav_ms": 4.0, "gpu_ms": 4.0, "hiz": 0.0}),
                  "pr": ({"cpu.nav_ms": 1.0, "gpu_ms": 4.4, "hiz": 6.0, "firefight.cpu_ms": 9.0},
                         {"cpu.nav_ms": 1.0, "gpu_ms": 4.4, "hiz": 6.0, "firefight.cpu_ms": 9.0}),
                  "main-again": ({"cpu.nav_ms": 3.3, "gpu_ms": 4.0, "hiz": 0.0}, {"cpu.nav_ms": 2.7, "gpu_ms": 4.0,
                                                                                   "hiz": 0.0})}
        for leg in legs:
            for out, rows in zip(leg.runs, values[leg.name]):
                write(out, rows, ["the CPU clock spread"] if leg.name == "pr" else ())
        text = stress.section(legs)
        self.assertTrue(text.startswith("**stress yard** (`make bench`'s scenario, 2 runs a leg, averaged)"))
        rows = self.rows(text)
        self.assertEqual(rows["cpu.nav_ms"], ["3", "1", "-66.7%", "+0.0%"], "each leg's runs averaged")
        self.assertEqual(rows["gpu_ms"][2:], ["+10.0%", "+0.0%"])
        self.assertEqual(rows["hiz"][2], "0 → 6", "a row from zero shows its two values")
        self.assertEqual(rows["firefight.cpu_ms"][:3], ["absent", "9", "absent on main"], "a window prefix kept")
        lead = text.split("| row |")[0]
        moved = [line.split("`")[1] for line in lead.splitlines() if line.startswith("- `")]
        self.assertEqual(moved, ["hiz", "cpu.nav_ms", "gpu_ms", "firefight.cpu_ms"], "largest change first")
        self.assertIn("the CPU clock spread (only PR: the legs disagree)", text)

    def test_a_malformed_report_is_skipped_not_a_traceback(self):
        legs = plan(Path(tempfile.mkdtemp()), 2)
        write(legs[0].runs[0], {"cpu_ms": 2.0})
        (legs[0].runs[1] / "bench.json").parent.mkdir(parents=True)
        (legs[0].runs[1] / "bench.json").write_text('{"metrics": [["cpu_ms", {"p95": 1}], 7, ["gpu_ms", {"avg": 1')
        self.assertEqual(stress.averages(legs[0]), {"cpu_ms": 2.0})
        (legs[0].runs[1] / "bench.json").write_text('{"metrics": [["cpu_ms", {"p95": 1}], 7, ["x"]]}')
        self.assertEqual(stress.averages(legs[0]), {"cpu_ms": 2.0})

    def test_a_yard_that_wrote_no_report_says_so(self):
        self.assertIn("neither the main leg nor the PR's wrote a report", stress.section(plan(Path(tempfile.mkdtemp()))))

    def test_a_stress_run_is_never_read_as_a_project(self):
        root = Path(tempfile.mkdtemp())
        write(root / "main" / "stress-0", {"cpu_ms": 1.0})
        scene = root / "main" / "yard-0123abcd" / "levels" / "a.json"
        scene.parent.mkdir(parents=True)
        scene.write_text(json.dumps({"rows": []}))
        self.assertEqual(table.scenes_of_all([root / "main"]), ["levels/a"])
        self.assertIsNone(table.read(root / "main", "bench"))


if __name__ == "__main__":
    unittest.main()
