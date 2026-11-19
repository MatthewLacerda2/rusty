#!/usr/bin/env python3
"""#2381: the three legs in order, each against the first `main` leg, with a stub runner."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from bench import legs

MAIN, PR = Path("/t/main/bench-scenes"), Path("/t/pr/bench-scenes")


class Legs(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        self.plan = legs.plan(MAIN, PR, self.root)
        self.ask = legs.Ask(Path("/games/yard"), "a,b", 3, None, ("gpu_occlusion_culling=true",))

    def test_three_legs_in_order_the_second_and_third_against_the_first(self):
        ran = []
        done = legs.run(self.plan, self.ask, ["cap", "--"], lambda argv: ran.append(argv) or 0)
        self.assertEqual([leg.name for leg, _ in done], ["main", "pr", "main-again"])
        self.assertEqual([argv[2] for argv in ran], [str(MAIN), str(PR), str(MAIN)])
        first = str(self.root / "main")
        self.assertNotIn("--before", ran[0])
        for argv in ran[1:]:
            self.assertEqual(argv[argv.index("--before") + 1], first)
        self.assertEqual([argv[-1] for argv in ran], [first, str(self.root / "pr"), str(self.root / "main-again")])
        for argv in ran:
            self.assertEqual(argv[:2], ["cap", "--"], "each leg in the scope")
            self.assertEqual(argv[argv.index("--set") + 1], "gpu_occlusion_culling=true")
            self.assertEqual(argv[argv.index("--scenes") + 1], "a,b")

    def test_load_runs_pass_through_only_when_asked(self):
        self.assertNotIn("--load-runs", legs.command(self.plan[0], self.ask))
        ask = legs.Ask(Path("/games/yard"), load_runs=5)
        argv = legs.command(self.plan[0], ask)
        self.assertEqual(argv[argv.index("--load-runs") + 1], "5")

    def test_a_failed_leg_stops_the_run_and_a_stale_leg_folder_is_emptied(self):
        stale = self.root / "pr" / "old.json"
        stale.parent.mkdir(parents=True)
        stale.write_text("{}")
        codes = iter([0, 1, 0])
        done = legs.run(self.plan, self.ask, [], lambda argv: next(codes))
        self.assertEqual([(leg.name, code) for leg, code in done], [("main", 0), ("pr", 1)])
        self.assertFalse(stale.exists())

    def test_a_refusal_before_a_leg_stops_it(self):
        said = lambda leg: "cargo (PID 9)" if leg.name == "pr" else None
        with self.assertRaisesRegex(RuntimeError, "before the pr leg: cargo"):
            legs.run(self.plan, self.ask, [], lambda argv: 0, said)

    def test_the_cap_is_a_fraction_of_what_is_available_or_none(self):
        self.assertEqual(legs.available_bytes("MemTotal: 9 kB\nMemAvailable:    2048 kB\n"), 2048 * 1024)
        self.assertIsNone(legs.available_bytes("MemTotal: 9 kB\n"))
        prefix, said = legs.scope(3 * 1024**3, True)
        self.assertIn("MemoryMax=3221225472", prefix)
        self.assertIn("3.0 GiB", said)
        self.assertEqual(legs.scope(3 * 1024**3, False)[0], [])
        self.assertIn("run plain", legs.scope(None, True)[1])


if __name__ == "__main__":
    unittest.main()
