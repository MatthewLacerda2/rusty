#!/usr/bin/env python3
"""#2463's review: the yard's quiet counters (`dev::bench::quiet`) are hushed in its sections as in a
scene's: a quiet row prints only when main and the PR differ, in the window it moved in."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from bench import stress
from tests.bench.test_stress_windows import plan, rows, sections


def write(out: Path, loud: dict[str, float], hushed: dict[str, float]) -> None:
    """A two-window run's `bench.json`, the `hushed` rows marked quiet as `play` keeps them."""
    out.mkdir(parents=True, exist_ok=True)
    metrics = [[m, {"avg": v, "p95": v}] for m, v in loud.items()]
    metrics += [[m, {"avg": v, "p95": v, "quiet": True}] for m, v in hushed.items()]
    (out / "bench.json").write_text(json.dumps({"frames": 600, "metrics": metrics, "warnings": [],
                                                "windows": ["firefight"]}))


LOUD = {"cpu_ms": 2.0, "firefight.cpu_ms": 6.0}
HUSHED = {"bones": 3.0, "ui_meshed": 1.0, "firefight.bones": 3.0, "firefight.ui_meshed": 1.0}


class Quiet(unittest.TestCase):
    def test_a_quiet_counter_prints_only_in_the_window_where_main_and_the_pr_differ(self):
        legs = plan(Path(tempfile.mkdtemp()), 1)
        for leg in legs:
            moved = {"firefight.ui_meshed": 0.5} if leg.name == "pr" else {}
            write(leg.runs[0], LOUD, {**HUSHED, **moved})
        parts = sections(stress.section(legs))
        first, fight = parts.values()
        self.assertEqual(list(rows(first)), ["cpu_ms"], first)
        self.assertIn("The other 2 counters are identical", first)
        self.assertEqual(list(rows(fight)), ["firefight.cpu_ms", "firefight.ui_meshed"], fight)
        self.assertIn("The other 1 counters are identical", fight)

    def test_a_quiet_counter_absent_on_one_leg_is_shown(self):
        legs = plan(Path(tempfile.mkdtemp()), 1)
        for leg in legs:
            write(leg.runs[0], LOUD, HUSHED if leg.name != "pr" else {"bones": 3.0})
        first = next(iter(sections(stress.section(legs)).values()))
        self.assertEqual(rows(first)["ui_meshed"][1], "absent")
        self.assertNotIn("bones", rows(first))


if __name__ == "__main__":
    unittest.main()
