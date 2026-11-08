#!/usr/bin/env python3
"""One bad shard never sizes the sweep (PR #1903's review).

A shard that measured 1 mutant in 25 min (a degraded runner, a fatal error just
after its baseline) once read as 25 min a mutant: 419 shards at 4018 mutants,
past the 256 ceiling, so the plan went red and stayed red, since no new report
was written. These hold both guards: a shard under `MIN_MEASURED` is no rate,
and the slowest rate is capped at `OUTLIER_CAP` times the mean. And the
lookup's probe, `has-rate`, says no for a report that sizes nothing.
"""

from __future__ import annotations

import datetime as dt
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from tests import load

shards = load("mutants-shards.py")
SCRIPT = Path(__file__).resolve().parents[2] / "mutants-shards.py"
SATURDAY = dt.datetime(2026, 11, 14, 4, 41, tzinfo=dt.timezone.utc)


class Outliers(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.root = Path(self.dir.name)

    def shard(self, k: int, seconds: int, measured: int) -> None:
        d = self.root / f"mutants-shard-{k}"
        d.mkdir()
        (d / "elapsed-seconds").write_text(f"{seconds}\n")
        (d / "missed.txt").write_text("".join(f"m{i}\n" for i in range(measured)))

    def last_week(self) -> None:
        for k in range(23):
            self.shard(k, 15300, 50)     # 5.1 min a mutant

    def test_one_mutant_in_25_minutes_is_no_rate(self):
        self.last_week()
        self.shard(23, 1500, 1)
        rate = shards.calibrate(self.root, "run 1")
        self.assertAlmostEqual(rate.slowest, 5.1)
        self.assertIn("23 shard reports", rate.source)
        outputs, _ = shards.plan(4018, rate, SATURDAY)
        self.assertLess(outputs[0].count(",") + 1, shards.MATRIX_CEILING // 2)

    def test_the_minimum_is_inclusive(self):
        self.shard(0, 3000, shards.MIN_MEASURED)
        self.shard(1, 3000, shards.MIN_MEASURED - 1)
        self.assertIn("1 shard reports", shards.calibrate(self.root).source)

    def test_a_slow_shard_is_capped_at_a_multiple_of_the_mean(self):
        self.last_week()
        self.shard(23, 15000, 10)        # 25 min a mutant, enough mutants to count
        rate = shards.calibrate(self.root)
        self.assertAlmostEqual(rate.slowest, rate.mean * shards.OUTLIER_CAP)
        outputs, _ = shards.plan(4018, rate, SATURDAY)   # ≈ 133 shards, not 419
        self.assertLess(outputs[0].count(",") + 1, shards.MATRIX_CEILING * 0.6)

    def test_a_real_spread_is_not_capped(self):
        self.shard(0, 6000, 25)          # 4 min
        self.shard(1, 20400, 51)         # 6.7 min: 1.3 × the mean
        rate = shards.calibrate(self.root)
        self.assertAlmostEqual(rate.slowest, 20400 / 60 / 51)

    def test_the_fallback_is_marked_unmeasured(self):
        self.assertFalse(shards.calibrate(None).measured)
        self.shard(0, 3000, 20)
        self.assertTrue(shards.calibrate(self.root).measured)

    def has_rate(self, reports: Path) -> int:
        return subprocess.run([sys.executable, str(SCRIPT), "has-rate", "--reports", str(reports)],
                              check=False).returncode

    def test_the_lookup_probe_passes_over_a_report_without_a_rate(self):
        self.assertEqual(self.has_rate(self.root / "missing"), 1)
        self.shard(0, 1500, 1)
        self.assertEqual(self.has_rate(self.root), 1)
        self.shard(1, 3000, 20)
        self.assertEqual(self.has_rate(self.root), 0)


if __name__ == "__main__":
    unittest.main()
