#!/usr/bin/env python3
"""An on-request report never reads cleaner than the run was (#750).

A run stopped at its budget says how much it never reached; a timed-out
mutant is unmeasured, never "no survivors", and a run where most time out says
it is not a result; a broken run writes no report at all; survivors come with
their diffs.
"""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from tests import SCRIPTS

SCRIPT = SCRIPTS / "mutants-report.py"
_spec = importlib.util.spec_from_file_location("mutants_report", SCRIPT)
report = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(report)


def outcome(summary: str, name: str, diff: str | None = None) -> dict:
    out = {"scenario": {"Mutant": {"name": name, "file": name.split(":")[0]}}, "summary": summary}
    if diff:
        out["diff_path"] = diff
    return out


RUN = {"outcomes": [
    {"scenario": "Baseline", "summary": "Success"},
    outcome("CaughtMutant", "src/physics/a.rs:3:5: replace f -> u32 with 0"),
    outcome("MissedMutant", "src/physics/a.rs:9:5: replace g -> bool with true", "diff/g.diff"),
    outcome("MissedMutant", "src/physics/a.rs:1:5: replace h -> u32 with 1"),
    outcome("Unviable", "src/physics/a.rs:4:5: replace k -> Foo with Default::default()"),
]}


class Render(unittest.TestCase):
    def test_counts_mutants_not_the_baseline(self) -> None:
        text = report.render("Scoped.", RUN, 4, 2)
        self.assertIn("| 1 | 2 | 0 | 1 |", text)

    def test_survivors_are_a_sorted_worklist(self) -> None:
        text = report.render("Scoped.", RUN, 4, 2)
        self.assertLess(text.index("a.rs:1:5"), text.index("a.rs:9:5"))
        self.assertNotIn("replace f -> u32", text, "a caught mutant is not a survivor")

    def test_a_run_stopped_at_its_budget_names_the_gap(self) -> None:
        text = report.render("Scoped.", RUN, 10, 124)
        self.assertIn("4 of 10 planned mutants measured, 6 never reached", text)

    def test_nothing_mutated_is_not_called_clean(self) -> None:
        text = report.render("Scoped.", {}, 0, 0)
        self.assertIn("Nothing in scope was mutated", text)
        self.assertNotIn("No survivors", text)

    def test_a_caught_only_run_is_clean(self) -> None:
        run = {"outcomes": RUN["outcomes"][:2]}
        self.assertIn("No survivors. ✅", report.render("Scoped.", run, 1, 0))

    def test_an_all_timeout_run_is_not_a_result(self) -> None:
        run = {"outcomes": [outcome("Timeout", f"a.rs:{n}:1: replace x") for n in range(3)]
               + RUN["outcomes"][4:]}
        text = report.render("Scoped.", run, 4, 3)
        self.assertIn("3 of 3 viable mutants timed out", text)
        self.assertIn("not a result", text)
        self.assertNotIn("✅", text)

    def test_one_timeout_among_caught_is_named_unmeasured(self) -> None:
        run = {"outcomes": RUN["outcomes"][:2] * 2 + [outcome("Timeout", "a.rs:7:1: replace y")]}
        text = report.render("Scoped.", run, 3, 3)
        self.assertIn("1 of 3 viable mutants timed out: unmeasured", text)
        self.assertNotIn("not a result", text)
        self.assertNotIn("✅", text)


class Files(unittest.TestCase):
    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.root = Path(self.dir.name)
        (self.root / "mutants.out/diff").mkdir(parents=True)
        (self.root / "mutants.out/diff/g.diff").write_text("-    x\n+    true\n")
        (self.root / "mutants.out/outcomes.json").write_text(json.dumps(RUN))
        (self.root / "scope.txt").write_text("Scoped on request.\n")

    def cli(self, status: int) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(SCRIPT), str(self.root / "mutants.out"),
             str(self.root / "scope.txt"), "--in-scope", "4", "--exit", str(status),
             "--out", str(self.root / "report")],
            capture_output=True, text=True, check=False)

    def test_each_survivor_has_its_diff_or_says_it_has_none(self) -> None:
        self.assertEqual(self.cli(2).returncode, 0, "survivors are a result, not a failure")
        diffs = (self.root / "report/survivors.diff").read_text()
        self.assertIn("+    true", diffs)
        self.assertIn("kept no diff", diffs)

    def test_a_broken_run_writes_no_report(self) -> None:
        done = self.cli(1)
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("not a result", done.stderr)
        self.assertFalse((self.root / "report/report.md").exists())


if __name__ == "__main__":
    unittest.main()
