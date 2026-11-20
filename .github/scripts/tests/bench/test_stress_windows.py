#!/usr/bin/env python3
"""#2450: a two-window yard run (`make bench`'s quiet window and `firefight`) through the stress section."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from bench import stress

MAIN, PR = Path("/t/bench-main"), Path("/t/pr-7-head")


def plan(root: Path, runs=None):
    return stress.plan((MAIN, MAIN / "target/release/play"), (PR, PR / "target/release/play"), root, runs)


def write(out: Path, rows: dict[str, float], windows=("firefight",)) -> None:
    """A run's kept `bench.json` as `dev::bench::window::Run` writes it: both windows in one."""
    out.mkdir(parents=True, exist_ok=True)
    metrics = [[m, {"avg": v, "p95": v}] for m, v in rows.items()]
    report = {"frames": 600, "metrics": metrics, "warnings": []}
    if windows:
        report["windows"] = list(windows)
    (out / "bench.json").write_text(json.dumps(report))


def sections(text: str) -> dict[str, str]:
    """Each section's text by its title line."""
    out, title = {}, None
    for line in text.splitlines():
        if line.startswith("**stress yard"):
            title = line
            out[title] = ""
        elif title:
            out[title] += line + "\n"
    return out


def rows(text: str) -> dict[str, list[str]]:
    cells = [line.strip("|").split(" | ") for line in text.splitlines() if line.startswith("| ")]
    return {c[0].strip(): [x.strip() for x in c[1:]] for c in cells[1:]}


def moved(text: str) -> list[str]:
    lead = text.split("| row |")[0]
    return [line.split("`")[1] for line in lead.splitlines() if line.startswith("- `")]


QUIET = {"cpu_ms": 2.0, "cpu.nav_ms": 1.0}
FIGHT = {"firefight.cpu_ms": 6.0, "firefight.cpu.nav_ms": 3.0}


class Windows(unittest.TestCase):
    def test_each_window_gets_its_own_section_its_rows_by_their_prefixed_names(self):
        legs = plan(Path(tempfile.mkdtemp()), 1)
        values = {"main": {**QUIET, **FIGHT}, "main-again": {**QUIET, **FIGHT},
                  "pr": {**QUIET, "firefight.cpu_ms": 9.0, "firefight.cpu.nav_ms": 3.0, "firefight.new": 1.0}}
        for leg in legs:
            write(leg.runs[0], values[leg.name])
        parts = sections(stress.section(legs))
        quiet, fight = list(parts)
        self.assertTrue(quiet.startswith("**stress yard** ("))
        self.assertEqual(fight, "**stress yard, `firefight` window**")
        self.assertEqual(list(rows(parts[quiet])), ["cpu_ms", "cpu.nav_ms"], "the quiet window holds no firefight row")
        self.assertEqual(moved(parts[quiet]), [], "nothing moved in the quiet window")
        self.assertEqual(list(rows(parts[fight])), ["firefight.cpu_ms", "firefight.cpu.nav_ms", "firefight.new"])
        self.assertEqual(rows(parts[fight])["firefight.cpu_ms"][2:], ["+50.0%", "+0.0%"], "the same floor rule")
        self.assertEqual(moved(parts[fight]), ["firefight.cpu_ms", "firefight.new"], "the lead names the window's row")

    def test_a_firefight_row_inside_its_floor_is_not_led(self):
        legs = plan(Path(tempfile.mkdtemp()), 1)
        values = {"main": {**QUIET, **FIGHT}, "pr": {**QUIET, "firefight.cpu_ms": 6.5, "firefight.cpu.nav_ms": 3.0},
                  "main-again": {**QUIET, "firefight.cpu_ms": 7.0, "firefight.cpu.nav_ms": 3.0}}
        for leg in legs:
            write(leg.runs[0], values[leg.name])
        fight = sections(stress.section(legs))["**stress yard, `firefight` window**"]
        self.assertEqual(rows(fight)["firefight.cpu_ms"][2], "inside the floor (+8.3%)")
        self.assertEqual(moved(fight), [])

    def test_a_prefix_no_run_declares_stays_with_the_first_window(self):
        legs = plan(Path(tempfile.mkdtemp()), 1)
        for leg in legs:
            write(leg.runs[0], {**QUIET, **FIGHT}, windows=())
        parts = sections(stress.section(legs))
        self.assertEqual(len(parts), 1, "no window declared: one section, as before the firefight window")
        self.assertIn("firefight.cpu_ms", rows(next(iter(parts.values()))))

    def test_a_window_only_the_pr_declares_reads_absent_on_main(self):
        legs = plan(Path(tempfile.mkdtemp()), 1)
        write(legs[0].runs[0], QUIET, windows=())
        write(legs[1].runs[0], {**QUIET, **FIGHT})
        write(legs[2].runs[0], QUIET, windows=())
        fight = sections(stress.section(legs))["**stress yard, `firefight` window**"]
        self.assertEqual(rows(fight)["firefight.cpu_ms"][2], "absent on main")


if __name__ == "__main__":
    unittest.main()
