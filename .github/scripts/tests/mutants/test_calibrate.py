#!/usr/bin/env python3
"""The sweep sizes from what the last one measured (#1684).

Run 42367735766 was sized at a hand-set 4.3 min a mutant and its shards spread
from 3.7 to 6.7, so four stopped at their budget. These hold the calibration
from an earlier sweep's shard reports (finished, cut off, missing, malformed),
the slowest-shard choice, the width arithmetic at its edges (one shard, the
cap, a deadline even the cap misses) and the report's rate line.
"""

from __future__ import annotations

import datetime as dt
import tempfile
import unittest
from pathlib import Path

from tests import load

shards = load("mutants-shards.py")
SATURDAY = dt.datetime(2026, 11, 14, 4, 41, tzinfo=dt.timezone.utc)


class Reports(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.root = Path(self.dir.name)

    def shard(self, k: int, elapsed: str | None, measured: int) -> None:
        d = self.root / f"mutants-shard-{k}"
        d.mkdir()
        if elapsed is not None:
            (d / "elapsed-seconds").write_text(elapsed + "\n")
        (d / "caught.txt").write_text("".join(f"m{i}\n" for i in range(measured)))

    def test_the_slowest_shard_sizes_and_the_mean_times(self):
        self.shard(0, "6000", 25)        # 4 min a mutant, finished
        self.shard(1, "20400", 51)       # cut off at its budget: still a rate, 6.7
        rate = shards.calibrate(self.root, "run 1")
        self.assertAlmostEqual(rate.slowest, 20400 / 60 / 51)
        self.assertAlmostEqual(rate.mean, 26400 / 60 / 76)
        self.assertIn("2 shard reports of run 1", rate.source)

    def test_a_malformed_shard_is_skipped_not_trusted(self):
        self.shard(0, "6000", 25)
        self.shard(1, "0", 10)           # zero elapsed
        self.shard(2, None, 10)          # absent
        self.shard(3, "soon", 10)        # not a number
        self.shard(4, "-5", 10)          # negative
        self.shard(5, "900", 0)          # never passed its baseline
        rate = shards.calibrate(self.root)
        self.assertEqual(rate.slowest, 4.0)
        self.assertIn("1 shard reports", rate.source)

    def test_no_readable_report_falls_back(self):
        self.shard(0, "0", 3)
        for reports in (self.root, self.root / "missing", None):
            rate = shards.calibrate(reports)
            self.assertEqual(rate.slowest, shards.MINUTES_PER_MUTANT)
            self.assertIn("`MINUTES_PER_MUTANT`", rate.source)

    def test_the_report_line_warns_when_slower_than_sized(self):
        self.shard(0, "24000", 50)       # 8 min
        line = shards.report_rate(shards.calibrate(self.root, "this run"), 6.0)
        self.assertIn("8.0 on the slowest shard", line)
        self.assertIn("⚠️", line)
        self.assertNotIn("⚠️", shards.report_rate(shards.calibrate(self.root), 8.0))

    def test_nothing_measured_has_no_rate(self):
        self.assertIn("no rate", shards.report_rate(shards.calibrate(None), 6.7))


class Width(unittest.TestCase):
    def test_one_shard_is_one_runner(self):
        self.assertEqual(shards.parallel(1, 3.0, 40.0), (1, 3.0, True))

    def test_the_narrowest_width_that_ends_in_time(self):
        # 12 shards of 4 h by 20 h: 3 wide is 4 waves (16 h), 2 wide is 24 h.
        self.assertEqual(shards.parallel(12, 4.0, 20.0), (3, 16.0, True))

    def test_never_past_the_cap_and_late_is_said(self):
        width, hours, in_time = shards.parallel(200, 4.0, 40.0)
        self.assertEqual(width, shards.MAX_PARALLEL)
        self.assertEqual(hours, 80.0)
        self.assertFalse(in_time)

    def test_the_deadline_is_sunday_night(self):
        self.assertAlmostEqual(shards.hours_to_deadline(SATURDAY), 43 + 18 / 60)
        monday = dt.datetime(2026, 11, 16, 0, 30, tzinfo=dt.timezone.utc)
        self.assertAlmostEqual(shards.hours_to_deadline(monday), 6 * 24 + 23 + 29 / 60)

    def test_last_weeks_sweep_at_its_measured_rate(self):
        # Run 42367735766: ≈ 4018 mutants, slowest 6.67, mean 5.10 min.
        rate = shards.Rate(6.67, 5.10, "run 42367735766")
        outputs, summary = shards.plan(4018, rate, SATURDAY)
        self.assertEqual(outputs[1:], ["parallel=9", "minutes=6.67"])
        self.assertEqual(outputs[0].count(","), 112 - 1)
        self.assertNotIn("⚠️", summary)
        _, late = shards.plan(4018, rate, SATURDAY + dt.timedelta(hours=10))
        self.assertIn("⚠️", late)


if __name__ == "__main__":
    unittest.main()
