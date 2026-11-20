#!/usr/bin/env python3
"""#2381: what `FORCE=1` runs past is still checked and listed: a build started
mid-run, and two builds that disagree on the texture cache."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

from bench import refuse

HERE = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("bench_ab_force", HERE / "bench-ab.py")
bench_ab = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench_ab)

BUSY = refuse.busy_refusal(["cargo (PID 9)"])


class BusyBeforeEachLeg(unittest.TestCase):
    def test_without_force_a_build_mid_run_stops_the_legs(self):
        self.assertEqual(bench_ab.Gate(False).leg("pr", BUSY), BUSY)

    def test_under_force_it_is_checked_and_listed(self):
        gate = bench_ab.Gate(True)
        self.assertIsNone(gate.leg("main-again", BUSY))
        self.assertEqual(len(gate.overridden), 1)
        self.assertIn("before the main-again leg", gate.overridden[0])
        self.assertIn("cargo (PID 9)", gate.overridden[0])

    def test_one_build_held_over_every_run_is_said_once(self):
        gate = bench_ab.Gate(True)
        for _ in range(3):
            self.assertIsNone(gate.leg("stress pr", BUSY))
        self.assertEqual(len(gate.overridden), 1)
        self.assertIsNone(gate.leg("stress main-again", BUSY))
        self.assertEqual(len(gate.overridden), 2, "another leg is its own line")

    def test_a_quiet_machine_is_noted_by_nobody(self):
        gate = bench_ab.Gate(True)
        self.assertIsNone(gate.leg("pr", None))
        self.assertEqual(gate.overridden, [])


CLEAN = ("clean", None)
PENDING = refuse.pending_answer("the PR", 1, "assets/a.png\n")


class Textures(unittest.TestCase):
    def test_both_clean_compare(self):
        self.assertEqual(refuse.textures(CLEAN, CLEAN), ([], True))

    def test_builds_that_disagree_are_told_apart_from_a_missing_import(self):
        said, comparable = refuse.textures(CLEAN, PENDING)
        self.assertFalse(comparable)
        self.assertEqual(len(said), 1)
        self.assertIn("disagree", said[0])
        self.assertIn("main's finds the project imported, the PR's does not", said[0])
        self.assertIn("FORCE=1", said[0])
        self.assertNotIn("make import", said[0], "an import cannot satisfy both: no loop")
        self.assertIn("assets/a.png", said[0])

    def test_a_project_never_imported_says_to_import(self):
        main = refuse.pending_answer("main", 1, "assets/a.png\n")
        said, comparable = refuse.textures(main, PENDING)
        self.assertTrue(comparable)
        self.assertEqual(len(said), 2)
        self.assertTrue(all("make import" in s for s in said))
        self.assertIn("main's build", said[0])

    def test_a_build_that_could_not_be_asked_is_no_disagreement(self):
        said, comparable = refuse.textures(("cannot", "no --pending-imports"), PENDING)
        self.assertTrue(comparable)
        self.assertEqual(said[0], "no --pending-imports")

    def test_disagreement_under_force_runs_on_listed(self):
        gate = bench_ab.Gate(True)
        self.assertFalse(bench_ab.check_textures(CLEAN, PENDING, gate))
        self.assertIn("disagree", gate.overridden[0])


if __name__ == "__main__":
    unittest.main()
