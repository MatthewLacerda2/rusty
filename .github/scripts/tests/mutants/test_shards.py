#!/usr/bin/env python3
"""The weekly sweep sizes its own matrix (#1684).

A fixed shard count was outgrown (#749, then run 41193398365 cut all 24 shards
off at their budget), and so was a fixed cost a mutant (run 42367735766), so
`mutants-plan` counts the mutants and `mutants-shards.py` turns the count into
the matrix. These hold the sizing (never a silent cap, never an empty matrix),
the plan's command line and the workflow's wiring: one list of globs for the
count and the shards, and the width the plan chose. Calibration and width:
`test_calibrate.py`.
"""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import unittest

from tests import SCRIPTS

SCRIPT = SCRIPTS / "mutants-shards.py"
WORKFLOW = SCRIPTS.parent / "workflows" / "mutants-sweep.yml"
_spec = importlib.util.spec_from_file_location("mutants_shards", SCRIPT)
shards = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(shards)


def plan(stdin: str) -> subprocess.CompletedProcess:
    # No `--reports`: the fallback sizes it. Saturday 04:41, the schedule's start.
    return subprocess.run([sys.executable, str(SCRIPT), "plan", "--now", "2026-11-14T04:41:00+00:00"],
                          input=stdin, capture_output=True, text=True, check=False)


class Sizing(unittest.TestCase):
    def test_the_count_that_cut_off_all_24_shards(self):
        # 3552 mutants × 4.3 min ÷ 240 min = 63.6: each shard near 4 h.
        self.assertEqual(shards.shard_count(3552, 4.3), 64)

    def test_one_mutant_is_one_shard(self):
        self.assertEqual(shards.shard_count(1), 1)

    def test_nothing_to_mutate_fails(self):
        with self.assertRaisesRegex(ValueError, "globs"):
            shards.shard_count(0)

    def test_past_the_matrix_ceiling_fails_rather_than_caps(self):
        most = int(shards.MATRIX_CEILING * shards.TARGET_HOURS * 60 / shards.MINUTES_PER_MUTANT)
        self.assertEqual(shards.shard_count(most), shards.MATRIX_CEILING)
        with self.assertRaisesRegex(ValueError, "ceiling"):
            shards.shard_count(most + 1)

    def test_listing_counts_non_blank_lines(self):
        self.assertEqual(shards.count_listed(["src/a.rs:1:1: replace x\n", "\n", "b\n"]), 2)


class Cli(unittest.TestCase):
    def test_plan_prints_github_outputs(self):
        done = plan("m1\nm2\nm3\n")
        self.assertEqual(done.returncode, 0, done.stderr)
        outputs = dict(line.split("=", 1) for line in done.stdout.splitlines())
        self.assertEqual(json.loads(outputs["shards"]), [0])
        self.assertEqual(outputs["parallel"], "1")
        self.assertEqual(float(outputs["minutes"]), shards.MINUTES_PER_MUTANT)
        self.assertIn("3 mutants", done.stderr)
        self.assertIn("`MINUTES_PER_MUTANT`", done.stderr, "the summary names its source")

    def test_plan_of_an_empty_listing_is_red(self):
        done = plan("")
        self.assertNotEqual(done.returncode, 0)
        self.assertEqual(done.stdout, "")


class Wiring(unittest.TestCase):
    def setUp(self):
        self.text = WORKFLOW.read_text()

    def test_the_matrix_is_the_plans(self):
        self.assertIn("shard: ${{ fromJSON(needs.mutants-plan.outputs.shards) }}", self.text)

    def test_a_skipped_sweep_still_gets_a_valid_matrix(self):
        # An empty matrix is an error, even on a job whose `if:` skips it.
        self.assertIn("shards: ${{ steps.size.outputs.shards || '[0]' }}", self.text)

    def test_the_width_is_the_plans(self):
        self.assertIn("max-parallel: ${{ fromJSON(needs.mutants-plan.outputs.parallel) }}", self.text)
        self.assertIn("parallel: ${{ steps.size.outputs.parallel || '1' }}", self.text)

    def test_the_report_compares_with_the_plans_rate(self):
        self.assertIn("--sized-at", self.text)
        self.assertIn("needs.mutants-plan.outputs.minutes", self.text)

    def test_count_and_shards_read_one_list_of_globs(self):
        self.assertEqual(self.text.count('for glob in $SWEEP_FILES; do files+=(--file "$glob"); done'), 2)
        self.assertNotIn("--file 'src/", self.text)


if __name__ == "__main__":
    unittest.main()
